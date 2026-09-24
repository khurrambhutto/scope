//! Tauri command handlers for uninstall/update preview + apply.
//!
//! These are thin typed wrappers. Every operation follows the same pattern:
//! preview looks up the package in the cached scan, issues a plan into the
//! [`PlanStore`], and apply only accepts a `plan_id` the backend itself issued,
//! revalidating the live system with a targeted single-package probe before
//! executing. Progress stages are streamed over [`OPERATION_STATUS_EVENT`] so
//! the UI can stay honest during the wait before the Polkit password dialog.

use tauri::{AppHandle, Emitter, State};

use crate::commands::packages::ScanCache;
use crate::operations::probe::probe_package;
use crate::operations::uninstall::{apply as apply_remove, preview as preview_remove, revalidate as revalidate_remove};
use crate::operations::update::{
    apply as apply_update_op,
    preview as preview_update_op,
    revalidate as revalidate_update_op,
};
use crate::operations::{
    OperationPlan, OperationResult, OperationStage, OperationStatus, PlanStore,
    OPERATION_STATUS_EVENT,
};

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
///
/// Revalidates with a single-package probe (fast — keeps the delay before the
/// Polkit dialog near-instant) and then executes the plan's command.
#[tauri::command]
pub async fn apply_uninstall(
    app: AppHandle,
    plans: State<'_, PlanStore>,
    plan_id: String,
) -> Result<OperationResult, String> {
    let plan = plans
        .take(&plan_id)
        .await
        .ok_or_else(|| "Stale or unknown uninstall plan. Please preview again.".to_string())?;

    emit_stage(&app, &plan.plan_id, OperationStage::Verifying);
    let probed = probe_package(plan.source, &plan.package_id, plan.install_scope)
        .await
        .map_err(|e| format!("Could not verify package state before continuing: {e}"))?;
    revalidate_remove(&plan, &probed).map_err(|e| e.to_string())?;
    emit_stage(&app, &plan.plan_id, OperationStage::Executing);

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
///
/// Revalidates with a single-package probe and then executes the plan's
/// update command.
#[tauri::command]
pub async fn apply_update(
    app: AppHandle,
    plans: State<'_, PlanStore>,
    plan_id: String,
) -> Result<OperationResult, String> {
    let plan = plans
        .take(&plan_id)
        .await
        .ok_or_else(|| "Stale or unknown update plan. Please preview again.".to_string())?;

    emit_stage(&app, &plan.plan_id, OperationStage::Verifying);
    let probed = probe_package(plan.source, &plan.package_id, plan.install_scope)
        .await
        .map_err(|e| format!("Could not verify package state before continuing: {e}"))?;
    revalidate_update_op(&plan, &probed).map_err(|e| e.to_string())?;
    emit_stage(&app, &plan.plan_id, OperationStage::Executing);

    let result = apply_update_op(&plan).await;
    Ok(result)
}

fn emit_stage(app: &AppHandle, plan_id: &str, stage: OperationStage) {
    let _ = app.emit(
        OPERATION_STATUS_EVENT,
        OperationStatus {
            plan_id: plan_id.to_string(),
            stage,
        },
    );
}

async fn find_package(
    scan_cache: &ScanCache,
    key: &str,
) -> Option<crate::package::InstalledPackage> {
    scan_cache.find(key).await
}
