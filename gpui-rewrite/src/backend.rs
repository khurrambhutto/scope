//! Bridge between the GPUI shell and the shared backend modules.
//!
//! The Tauri build reaches the backend through `src-tauri/src/commands/*`; this
//! crate replaces that thin Tauri layer with an equivalent orchestration layer
//! that reuses `scanner`, `operations`, and `safety` unchanged (they are pulled
//! in with `#[path]` from `src/main.rs`).

use std::future::Future;
use std::path::PathBuf;

use futures::channel::{mpsc, oneshot};

use crate::operations::{
    uninstall, update, Operation, OperationPlan, OperationResult, OperationStage, PlanStore,
};
use crate::package::InstalledPackage;
use crate::scanner::{scan_all, ScanAvailability};

/// One full scan of every package source, matching the Tauri `CachedScan` DTO.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Scan {
    pub packages: Vec<InstalledPackage>,
    pub availability: ScanAvailability,
    pub scanned_at_ms: u64,
}

/// Build a single-threaded Tokio runtime and drive one future to completion.
///
/// The shared backend uses `tokio::process`, which needs a Tokio reactor. We
/// run each scan/operation on a dedicated OS thread so the GPUI event loop is
/// never blocked.
fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build Tokio runtime")
        .block_on(future)
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
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))?;
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
        let _ = std::fs::write(path, bytes);
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
        let plan = match kind {
            OpKind::Uninstall => uninstall::preview(&pkg),
            OpKind::Update => {
                if !pkg.has_update {
                    let title = crate::theme::display_title(&pkg);
                    let _ = tx.send(Err(format!("'{title}' has no updates available.")));
                    return;
                }
                update::preview(&pkg)
            }
        };
        if !plan.protected {
            block_on(plans.issue(plan.clone()));
        }
        let _ = tx.send(Ok(plan));
    });
}

/// Revalidate and apply an issued plan, streaming stage/log messages.
///
/// Mirrors `apply_uninstall` / `apply_update`: take the plan from the store,
/// probe the live system, re-run the safety revalidation, then execute.
pub fn spawn_apply(plans: PlanStore, plan: OperationPlan, tx: mpsc::UnboundedSender<OpMsg>) {
    std::thread::spawn(move || {
        block_on(async move {
            let Some(plan) = plans.take(&plan.plan_id).await else {
                let _ = tx.unbounded_send(OpMsg::Done(failed(
                    "Stale or unknown plan. Please preview again.",
                )));
                return;
            };

            let _ = tx.unbounded_send(OpMsg::Stage(OperationStage::Verifying));
            let probed = match crate::operations::probe::probe_package(
                plan.source,
                &plan.package_id,
                plan.install_scope,
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
