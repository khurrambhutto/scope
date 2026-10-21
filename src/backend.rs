//! Bridge between the GPUI shell and the vendored domain modules.
//!
//! The Tauri build reaches its backend through `src-tauri/src/commands/*`; this
//! crate replaces that thin Tauri layer with an equivalent orchestration layer
//! over `crate::domain::{scanner, operations, safety}`.

use std::future::Future;
use std::path::PathBuf;

use futures::channel::{mpsc, oneshot};

use crate::domain::operations::{
    uninstall, update, Operation, OperationPlan, OperationResult, OperationStage, OperationTarget,
    PlanStore,
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

/// Load source-aware informational details off the UI thread.
pub fn spawn_package_details(
    pkg: InstalledPackage,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    tx: mpsc::UnboundedSender<crate::domain::package_details::InspectionUpdate>,
) {
    std::thread::spawn(move || {
        let progress = tx.clone();
        let details = block_on(crate::domain::package_details::inspect_progressive(
            pkg,
            cancel,
            move |update| {
                let _ = progress.unbounded_send(update);
            },
        ));
        let _ = tx.unbounded_send(crate::domain::package_details::InspectionUpdate {
            details,
            complete: true,
        });
    });
}

/// Which destructive operation a dialog is previewing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Uninstall,
    Update,
    /// Install an opened package file; `pkg.package_id` is the file path.
    Install,
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
                OpKind::Install => install_preview(&pkg).await,
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

/// A list row standing in for an opened package file before it is installed.
pub fn opened_package(path: &std::path::Path) -> InstalledPackage {
    let mut pkg = InstalledPackage::new(
        if is_deb(path) {
            crate::domain::package::PackageSource::Apt
        } else {
            crate::domain::package::PackageSource::AppImage
        },
        path.to_string_lossy(),
    );
    pkg.name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "AppImage".into());
    if let Ok(size) = std::fs::metadata(path).map(|m| m.len()) {
        pkg.size_bytes = size;
    }
    pkg
}

fn is_deb(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("deb"))
}

async fn install_preview(pkg: &InstalledPackage) -> anyhow::Result<OperationPlan> {
    if pkg.source == crate::domain::package::PackageSource::Apt {
        return crate::domain::operations::deb_install::preview(std::path::Path::new(
            &pkg.package_id,
        ))
        .await;
    }
    let install =
        crate::domain::appimage::install::preview(std::path::Path::new(&pkg.package_id)).await?;
    Ok(OperationPlan {
        plan_id: crate::domain::operations::new_plan_id(),
        operation: Operation::Install,
        display_name: install.name.clone(),
        current_version: String::new(),
        target_version: install.version.clone().unwrap_or_default(),
        requires_auth: false,
        auth_method: crate::domain::operations::AuthMethod::None,
        protected: false,
        protection_reason: None,
        steps: install_steps(&install),
        created_at_ms: crate::domain::operations::now_ms(),
        apt_transaction_fingerprint: None,
        snap_details: None,
        app_data: Vec::new(),
        target: OperationTarget::AppImageInstall {
            plan: Box::new(install),
        },
    })
}

fn install_steps(
    install: &crate::domain::appimage::install::InstallPlan,
) -> Vec<crate::domain::operations::PlanStep> {
    use crate::domain::operations::PlanStep;
    let mut steps = vec![PlanStep {
        description: if install.move_source {
            "Move the AppImage into ~/AppImages.".into()
        } else {
            "Keep the AppImage in ~/AppImages.".into()
        },
        command_summary: install.destination.display().to_string(),
    }];
    steps.push(PlanStep {
        description: "Create its app-menu launcher.".into(),
        command_summary: install.launcher.display().to_string(),
    });
    if let Some(icon) = &install.icon {
        steps.push(PlanStep {
            description: "Save its icon.".into(),
            command_summary: icon.display().to_string(),
        });
    }
    steps
}

/// Revalidate and apply an issued plan, streaming stage/log messages.
///
/// Mirrors `apply_uninstall` / `apply_update`: take the plan from the store,
/// probe the live system, re-run the safety revalidation, then execute.
/// `remove_app_data` only selects whether the plan's own `app_data` list is
/// used; the paths always come from the stored plan.
pub fn spawn_apply(
    plans: PlanStore,
    plan_id: String,
    remove_app_data: bool,
    tx: mpsc::UnboundedSender<OpMsg>,
) {
    std::thread::spawn(move || {
        block_on(async move {
            let Some(mut plan) = plans.take(&plan_id) else {
                let _ = tx.unbounded_send(OpMsg::Done(failed(
                    "Stale or unknown plan. Please preview again.",
                )));
                return;
            };
            if !remove_app_data {
                plan.app_data.clear();
            }

            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Verifying));
            if let Some(expected) = plan
                .apt_transaction_fingerprint
                .as_deref()
                .filter(|_| !matches!(plan.target, OperationTarget::DebInstall { .. }))
            {
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
            let probed = match crate::domain::operations::probe::probe_target(&plan).await {
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
                // The probe already ran `install::revalidate` on the file.
                Operation::Install => match &plan.target {
                    OperationTarget::AppImageInstall { .. }
                    | OperationTarget::DebInstall { .. } => Ok(()),
                    _ => Err(anyhow::anyhow!(
                        "Only AppImage and Debian package files can be installed."
                    )),
                },
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

            let result = match (&plan.operation, &plan.target) {
                (Operation::Uninstall, _) => uninstall::apply(&plan, &on_line).await,
                (Operation::Update, _) => update::apply(&plan, &on_line).await,
                (Operation::Install, OperationTarget::DebInstall { .. }) => {
                    match crate::domain::operations::deb_install::apply(&plan, &on_line).await {
                        Ok(result) => result,
                        Err(error) => failed(format!("Install failed: {error}")),
                    }
                }
                (Operation::Install, OperationTarget::AppImageInstall { plan: install }) => {
                    match crate::domain::appimage::install::apply(install, &on_line).await {
                        Ok(message) => OperationResult {
                            success: true,
                            message,
                            logs: String::new(),
                            exit_code: Some(0),
                        },
                        Err(error) => failed(format!("Install failed: {error}")),
                    }
                }
                (Operation::Install, _) => {
                    failed("Only AppImage and Debian package files can be installed.")
                }
            };
            let _ = tx.unbounded_send(OpMsg::Done(result));
        });
    });
}

/// Check for a self-update off the UI thread.
pub fn spawn_updater_check(
    tx: oneshot::Sender<Result<Option<crate::domain::updater::UpdateCheck>, String>>,
) {
    std::thread::spawn(move || {
        let result = block_on(async {
            let kind = crate::domain::updater::detect();
            let current = env!("CARGO_PKG_VERSION").to_string();
            match crate::domain::updater::fetch_latest().await {
                Ok(release) => Ok(crate::domain::updater::check_update(
                    &current, &release, kind,
                )),
                Err(error) => Err(format!("Could not check for updates: {error}")),
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
    fn opened_files_route_debs_to_apt_and_appimages_to_appimage() {
        assert_eq!(
            opened_package(std::path::Path::new("/tmp/an app.DEB")).source,
            PackageSource::Apt
        );
        assert_eq!(
            opened_package(std::path::Path::new("/tmp/an app.AppImage")).source,
            PackageSource::AppImage
        );
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
        assert!(plan.snap_details.is_some());
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
