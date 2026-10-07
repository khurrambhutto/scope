//! Bridge between the GPUI shell and the vendored domain modules.
//!
//! The Tauri build reaches its backend through `src-tauri/src/commands/*`; this
//! crate replaces that thin Tauri layer with an equivalent orchestration layer
//! over `crate::domain::{scanner, operations, safety}`.

use std::future::Future;
use std::path::PathBuf;

use futures::channel::{mpsc, oneshot};

use crate::domain::operations::{
    uninstall, update, Operation, OperationPlan, OperationResult, OperationStage, PlanStore,
};
use crate::domain::package::InstalledPackage;
use crate::domain::scanner::{scan_all, ScanAvailability};

/// One full scan of every package source, matching the Tauri `CachedScan` DTO.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Scan {
    pub packages: Vec<InstalledPackage>,
    pub availability: ScanAvailability,
    pub scanned_at_ms: u64,
}

/// One current-thread Tokio runtime shared by every scan/preview/apply.
///
/// The shared backend uses `tokio::process`, which needs a Tokio reactor.
/// Jobs still run on dedicated OS threads so the GPUI event loop is never
/// blocked; tokio supports calling `block_on` on a current-thread runtime
/// concurrently from several threads (the first caller owns the IO/timer
/// drivers, later callers hook in and steal the driver when it finishes),
/// so a per-call runtime would only add construction cost.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build Tokio runtime")
    })
}

/// Drive one future to completion on the shared runtime.
fn block_on<F: Future>(future: F) -> F::Output {
    runtime().block_on(future)
}

/// Run a full scan on a background thread.
pub fn scan_blocking() -> Scan {
    block_on(async {
        let (packages, availability) = scan_all().await;
        Scan {
            packages,
            availability,
            scanned_at_ms: now_ms(),
        }
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Path of the persisted scan inside the XDG data directory.
fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(base.join("scope").join("scan-cache.json"))
}

/// Load the previous session's scan, if any, so a cold start paints instantly.
pub fn read_cache() -> Option<Scan> {
    let bytes = std::fs::read(cache_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persist a scan for the next launch.
pub fn write_cache(scan: &Scan) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(scan) {
        if let Some(parent) = path.parent() {
            if let Ok(mut temp) = tempfile::Builder::new()
                .prefix(".scan-cache-")
                .tempfile_in(parent)
            {
                use std::io::Write as _;
                if temp.write_all(&bytes).is_ok() && temp.as_file().sync_all().is_ok() {
                    let _ = temp.persist(path);
                }
            }
        }
    }
}

/// Which destructive operation a dialog is previewing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Uninstall,
    Update,
}

/// Messages streamed from a running apply task back to the UI.
pub enum OpMsg {
    Stage(OperationStage),
    Log(String),
    Done(OperationResult),
}

fn failed(message: impl Into<String>) -> OperationResult {
    OperationResult {
        success: false,
        message: message.into(),
        logs: String::new(),
        exit_code: None,
    }
}

/// Build (and register) an operation plan off the UI thread, then hand it back.
///
/// Mirrors `preview_uninstall` / `preview_update` in the Tauri command layer:
/// protected packages return a plan without being registered, and update plans
/// are rejected up front when no update is available.
pub fn spawn_preview(
    plans: PlanStore,
    kind: OpKind,
    pkg: InstalledPackage,
    tx: oneshot::Sender<Result<OperationPlan, String>>,
) {
    std::thread::spawn(move || {
        let plan = match block_on(async {
            match kind {
                OpKind::Uninstall => uninstall::preview(&pkg).await,
                OpKind::Update => {
                    if !pkg.has_update {
                        anyhow::bail!(
                            "'{}' has no updates available.",
                            crate::theme::display_title(&pkg)
                        );
                    }
                    update::preview(&pkg).await
                }
            }
        }) {
            Ok(plan) => plan,
            Err(error) => {
                let _ = tx.send(Err(error.to_string()));
                return;
            }
        };
        if !plan.protected {
            plans.issue(plan.clone());
        }
        let _ = tx.send(Ok(plan));
    });
}

/// Revalidate and apply an issued plan, streaming stage/log messages.
///
/// Mirrors `apply_uninstall` / `apply_update`: take the plan from the store,
/// probe the live system, re-run the safety revalidation, then execute.
pub fn spawn_apply(plans: PlanStore, plan_id: String, tx: mpsc::UnboundedSender<OpMsg>) {
    std::thread::spawn(move || {
        block_on(async move {
            let Some(plan) = plans.take(&plan_id) else {
                let _ = tx.unbounded_send(OpMsg::Done(failed(
                    "Stale or unknown plan. Please preview again.",
                )));
                return;
            };

            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Verifying));
            if let Some(expected) = plan.apt_transaction_fingerprint.as_deref() {
                let transaction = match crate::domain::operations::apt_transaction::simulate(
                    plan.operation,
                    plan.target.id(),
                )
                .await
                {
                    Ok(transaction) => transaction,
                    Err(error) => {
                        let _ = tx.unbounded_send(OpMsg::Done(failed(format!(
                            "Could not re-simulate the APT transaction: {error}"
                        ))));
                        return;
                    }
                };
                if transaction.fingerprint != expected {
                    let _ = tx.unbounded_send(OpMsg::Done(failed(
                        "The APT transaction changed since preview. Preview it again.",
                    )));
                    return;
                }
            }
            let probed = match crate::domain::operations::probe::probe_package(
                plan.target.source(),
                plan.target.id(),
                plan.target.install_scope(),
            )
            .await
            {
                Ok(probed) => probed,
                Err(e) => {
                    let _ = tx.unbounded_send(OpMsg::Done(failed(format!(
                        "Could not verify package state before continuing: {e}"
                    ))));
                    return;
                }
            };

            let revalidation = match plan.operation {
                Operation::Uninstall => uninstall::revalidate(&plan, &probed),
                Operation::Update => update::revalidate(&plan, &probed),
            };
            if let Err(e) = revalidation {
                let _ = tx.unbounded_send(OpMsg::Done(failed(e.to_string())));
                return;
            }

            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Executing));
            let line_tx = tx.clone();
            let on_line = move |line: &str| {
                let _ = line_tx.unbounded_send(OpMsg::Log(line.to_string()));
            };

            let result = match plan.operation {
                Operation::Uninstall => uninstall::apply(&plan, &on_line).await,
                Operation::Update => update::apply(&plan, &on_line).await,
            };
            let _ = tx.unbounded_send(OpMsg::Done(result));
        });
    });
}

/// Check for a self-update off the UI thread.
pub fn spawn_updater_check(tx: oneshot::Sender<Option<crate::domain::updater::UpdateCheck>>) {
    std::thread::spawn(move || {
        let result = block_on(async {
            let kind = crate::domain::updater::detect();
            let current = env!("CARGO_PKG_VERSION").to_string();
            match crate::domain::updater::fetch_latest().await {
                Ok(release) => crate::domain::updater::check_update(&current, &release, kind),
                Err(_) => None,
            }
        });
        let _ = tx.send(result);
    });
}

/// Download and install a self-update, streaming progress to the UI.
/// Reuses [`OpMsg`] so the banner needs no new message pump.
pub fn spawn_updater_install(
    check: crate::domain::updater::UpdateCheck,
    tx: mpsc::UnboundedSender<OpMsg>,
) {
    std::thread::spawn(move || {
        block_on(async move {
            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Verifying));
            let _ = tx.unbounded_send(OpMsg::Log(format!(
                "Downloading Scope {} ...",
                check.latest
            )));
            let line_tx = tx.clone();
            let on_line = move |line: &str| {
                let _ = line_tx.unbounded_send(OpMsg::Log(line.to_string()));
            };
            let artifact = match crate::domain::updater::download_verified(&check, &on_line).await {
                Ok(artifact) => artifact,
                Err(e) => {
                    let _ = tx.unbounded_send(OpMsg::Done(failed(format!(
                        "Update verification failed: {e}"
                    ))));
                    return;
                }
            };
            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Executing));
            let appimage_target = std::env::var_os("APPIMAGE").map(std::path::PathBuf::from);
            let result = crate::domain::updater::install(
                check.kind,
                &artifact,
                appimage_target.as_deref(),
                &on_line,
            )
            .await;
            let _ = tx.unbounded_send(OpMsg::Done(result));
        });
    });
}

/// Turn the backend's `scope-icon://localhost/<percent-encoded-path>` URL back
/// into the on-disk path GPUI can load directly. The URL is produced by
/// `icons::icon_url`, so decoding it is lossless.
pub fn icon_path(pkg: &InstalledPackage) -> Option<PathBuf> {
    let url = pkg.icon.as_deref()?;
    let encoded = url
        .strip_prefix("scope-icon://localhost")
        .or_else(|| url.strip_prefix("scope-icon://"))?;
    Some(PathBuf::from(percent_decode(encoded)))
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::PackageSource;

    #[test]
    fn percent_decode_expands_valid_sequences() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("%E2%9C%93"), "✓");
        assert_eq!(percent_decode("plain"), "plain");
    }

    #[test]
    fn percent_decode_keeps_invalid_sequences_verbatim() {
        assert_eq!(percent_decode("%zz"), "%zz");
        assert_eq!(percent_decode("trailing%4"), "trailing%4");
        assert_eq!(percent_decode("%"), "%");
        assert_eq!(percent_decode(""), "");
    }

    #[test]
    fn icon_path_accepts_only_scope_urls() {
        let mut pkg = InstalledPackage::new(PackageSource::Apt, "x");

        pkg.icon = Some("scope-icon://localhost/opt/my%20app.png".to_string());
        assert_eq!(icon_path(&pkg), Some(PathBuf::from("/opt/my app.png")));

        pkg.icon = Some("scope-icon://themes/hicolor/icon.svg".to_string());
        assert_eq!(
            icon_path(&pkg),
            Some(PathBuf::from("themes/hicolor/icon.svg"))
        );

        pkg.icon = Some("https://example.com/icon.png".to_string());
        assert_eq!(icon_path(&pkg), None);

        pkg.icon = None;
        assert_eq!(icon_path(&pkg), None);
    }

    fn apt_package(id: &str) -> InstalledPackage {
        let mut pkg = InstalledPackage::new(PackageSource::Apt, id);
        pkg.name = id.to_string();
        pkg
    }

    #[test]
    fn spawn_preview_rejects_an_update_without_an_update_available() {
        let (tx, rx) = oneshot::channel();
        let mut pkg = apt_package("gimp");
        pkg.has_update = false;
        spawn_preview(PlanStore::default(), OpKind::Update, pkg, tx);
        assert!(block_on(rx).unwrap().is_err());
    }

    #[test]
    fn spawn_preview_registers_a_non_protected_plan() {
        let store = PlanStore::default();
        let (tx, rx) = oneshot::channel();
        let mut pkg = apt_package("code");
        pkg.source = PackageSource::Snap;
        spawn_preview(store.clone(), OpKind::Uninstall, pkg, tx);
        let plan = block_on(rx).unwrap().unwrap();
        assert!(store.take(&plan.plan_id).is_some());
    }

    #[test]
    fn spawn_preview_does_not_register_a_protected_plan() {
        let store = PlanStore::default();
        let (tx, rx) = oneshot::channel();
        spawn_preview(store.clone(), OpKind::Uninstall, apt_package("systemd"), tx);
        let plan = block_on(rx).unwrap().unwrap();
        assert!(plan.protected);
        assert!(store.take(&plan.plan_id).is_none());
    }
}
