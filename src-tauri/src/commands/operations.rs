//! Tauri command handlers for uninstall preview + apply.
//!
//! These are thin typed wrappers. `preview_uninstall` looks the package up in
//! the cached scan, validates it, and issues a plan into the [`PlanStore`].
//! `apply_uninstall` only accepts a `plan_id` the backend itself issued,
//! revalidates against a fresh scan, then runs the source-specific command.

use tauri::State;

use crate::commands::packages::ScanCache;
use crate::operations::uninstall::{apply, preview, revalidate};
use crate::operations::{OperationPlan, OperationResult, PlanStore};

/// Build (and store) a preview plan for uninstalling the package with the given
/// backend key. Returns the plan for the UI to confirm, or an error if the
/// package cannot be found / is not uninstallable.
#[tauri::command]
pub async fn preview_uninstall(
    scan_cache: State<'_, ScanCache>,
    plans: State<'_, PlanStore>,
    package_key: String,
) -> Result<OperationPlan, String> {
    let pkg = find_package(&scan_cache, &package_key)
        .await
        .ok_or_else(|| format!("Package not found in current scan: {package_key}"))?;

    let plan = preview(&pkg);
    if plan.protected {
        // Still return the plan so the UI can show the protection reason, but
        // do not issue it for apply — protected plans cannot be executed.
        return Ok(plan);
    }
    plans.issue(plan.clone()).await;
    Ok(plan)
}

/// Apply a previously-issued uninstall plan by id. Revalidates against a fresh
/// full scan before executing, and rejects stale/missing/expired plans.
#[tauri::command]
pub async fn apply_uninstall(
    plans: State<'_, PlanStore>,
    plan_id: String,
) -> Result<OperationResult, String> {
    let plan = plans
        .take(&plan_id)
        .await
        .ok_or_else(|| "Stale or unknown uninstall plan. Please preview again.".to_string())?;

    // Backend revalidation: rescan the single source the plan targets so we
    // know the package is still present and still passes safety checks. We do
    // a full scan to keep the code path simple and authoritative.
    let (pkgs, _) = crate::scanner::scan_all().await;
    revalidate(&plan, &pkgs).await.map_err(|e| e.to_string())?;

    let result = apply(&plan).await;
    Ok(result)
}

async fn find_package(
    scan_cache: &ScanCache,
    key: &str,
) -> Option<crate::package::InstalledPackage> {
    scan_cache.find(key).await
}
