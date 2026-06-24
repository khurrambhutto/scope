//! Tauri command handlers for uninstall/update preview + apply.
//!
//! These are thin typed wrappers. Every operation follows the same pattern:
//! preview looks up the package in the cached scan, issues a plan into the
//! [`PlanStore`], and apply only accepts a `plan_id` the backend itself issued,
//! revalidating against a fresh scan before executing.

use tauri::State;

use crate::commands::packages::ScanCache;
use crate::operations::uninstall::{apply as apply_remove, preview as preview_remove, revalidate as revalidate_remove};
use crate::operations::update::{
    apply as apply_update_op,
    preview as preview_update_op,
    revalidate as revalidate_update_op,
};
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

    let plan = preview_remove(&pkg);
    if plan.protected {
        return Ok(plan);
    }
    plans.issue(plan.clone()).await;
    Ok(plan)
}

/// Apply a previously-issued uninstall plan by id.
#[tauri::command]
pub async fn apply_uninstall(
    plans: State<'_, PlanStore>,
    plan_id: String,
) -> Result<OperationResult, String> {
    let plan = plans
        .take(&plan_id)
        .await
        .ok_or_else(|| "Stale or unknown uninstall plan. Please preview again.".to_string())?;

    let (pkgs, _) = crate::scanner::scan_all().await;
    revalidate_remove(&plan, &pkgs).await.map_err(|e| e.to_string())?;

    let result = apply_remove(&plan).await;
    Ok(result)
}

/// Build (and store) a preview plan for updating the package with the given
/// backend key.
#[tauri::command]
pub async fn preview_update(
    scan_cache: State<'_, ScanCache>,
    plans: State<'_, PlanStore>,
    package_key: String,
) -> Result<OperationPlan, String> {
    let pkg = find_package(&scan_cache, &package_key)
        .await
        .ok_or_else(|| format!("Package not found in current scan: {package_key}"))?;

    if !pkg.has_update {
        return Err(format!(
            "'{}' has no updates available.",
            pkg.display_name.unwrap_or(pkg.name)
        ));
    }

    let plan = preview_update_op(&pkg);
    if plan.protected {
        return Ok(plan);
    }
    plans.issue(plan.clone()).await;
    Ok(plan)
}

/// Apply a previously-issued update plan by id.
#[tauri::command]
pub async fn apply_update(
    plans: State<'_, PlanStore>,
    plan_id: String,
) -> Result<OperationResult, String> {
    let plan = plans
        .take(&plan_id)
        .await
        .ok_or_else(|| "Stale or unknown update plan. Please preview again.".to_string())?;

    let (pkgs, _) = crate::scanner::scan_all().await;
    revalidate_update_op(&plan, &pkgs).await.map_err(|e| e.to_string())?;

    let result = apply_update_op(&plan).await;
    Ok(result)
}

async fn find_package(
    scan_cache: &ScanCache,
    key: &str,
) -> Option<crate::package::InstalledPackage> {
    scan_cache.find(key).await
}
