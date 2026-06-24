//! Update preview + apply per package source.
//!
//! Mirrors the uninstall flow: preview builds an [`OperationPlan`], apply
//! revalidates that the package still exists and still has an update, then runs
//! the source-specific update command.

use std::time::Duration;

use anyhow::Result;

use crate::package::{InstallScope, InstalledPackage, PackageSource};
use crate::safety;
use crate::system::run_elevated;

use super::{new_plan_id, now_ms, AuthMethod, Operation, OperationPlan, OperationResult, PlanStep};

/// Max time an update command may run before we cancel it (5 min for downloads).
const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);

/// Build a preview plan for updating one package.
pub fn preview(pkg: &InstalledPackage) -> OperationPlan {
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    let (auth, steps) = build_steps(pkg, protection.protected);

    OperationPlan {
        plan_id: new_plan_id(),
        operation: Operation::Update,
        source: pkg.source,
        package_id: pkg.package_id.clone(),
        install_scope: pkg.install_scope,
        display_name: pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone()),
        current_version: pkg.version.clone(),
        target_version: pkg.update_version.clone().unwrap_or_else(|| "latest".into()),
        requires_auth: matches!(auth, AuthMethod::Pkexec),
        auth_method: auth,
        protected: protection.protected,
        protection_reason: protection.reason,
        steps,
        created_at_ms: now_ms(),
    }
}

fn build_steps(pkg: &InstalledPackage, protected: bool) -> (AuthMethod, Vec<PlanStep>) {
    if protected {
        return (
            AuthMethod::None,
            vec![PlanStep {
                description: "Blocked: this package is protected and cannot be updated.".into(),
                command_summary: "(no command — protected)".into(),
            }],
        );
    }

    let target = pkg.update_version.as_deref().unwrap_or("latest");

    match pkg.source {
        PackageSource::Apt => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Update APT package '{}' from {} to {}.", pkg.package_id, pkg.version, target),
                command_summary: format!("pkexec env DEBIAN_FRONTEND=noninteractive apt install -y {}", pkg.package_id),
            }],
        ),
        PackageSource::Snap => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Update Snap '{}' to {}.", pkg.package_id, target),
                command_summary: format!("pkexec snap refresh {}", pkg.package_id),
            }],
        ),
        PackageSource::Flatpak => {
            let (auth, scope_flag): (AuthMethod, &str) = match pkg.install_scope {
                Some(InstallScope::User) => (AuthMethod::None, "--user"),
                _ => (AuthMethod::Pkexec, "--system"),
            };
            let cmd_prefix = match auth {
                AuthMethod::Pkexec => "pkexec flatpak",
                AuthMethod::None => "flatpak",
            };
            (
                auth,
                vec![PlanStep {
                    description: format!("Update Flatpak '{}' to {}.", pkg.package_id, target),
                    command_summary: format!("{} update -y {} {}", cmd_prefix, scope_flag, pkg.package_id),
                }],
            )
        }
        PackageSource::AppImage => (
            AuthMethod::None,
            vec![PlanStep {
                description: format!("Replace AppImage '{}' with latest version.", pkg.name),
                command_summary: format!("Download and replace {}", pkg.package_id),
            }],
        ),
    }
}

/// Re-validate that a package still exists and still has an update available.
pub async fn revalidate(plan: &OperationPlan, scan: &[InstalledPackage]) -> Result<()> {
    let still_present = scan.iter().any(|p| {
        p.source == plan.source
            && p.package_id == plan.package_id
            && p.install_scope == plan.install_scope
    });
    if !still_present {
        anyhow::bail!(
            "This update plan is stale: '{}' is no longer installed.",
            plan.display_name
        );
    }
    // Check the package still has an update available.
    let pkg = scan.iter().find(|p| {
        p.source == plan.source
            && p.package_id == plan.package_id
            && p.install_scope == plan.install_scope
    });
    if let Some(pkg) = pkg {
        if !pkg.has_update {
            anyhow::bail!(
                "'{}' no longer has updates available. Rescan and try again.",
                plan.display_name
            );
        }
    }
    Ok(())
}

/// Execute the plan's update command for the given source, capturing logs.
pub async fn apply(plan: &OperationPlan) -> OperationResult {
    match plan.source {
        PackageSource::Apt => apt_update(&plan.package_id).await,
        PackageSource::Snap => snap_refresh(&plan.package_id).await,
        PackageSource::Flatpak => flatpak_update(&plan.package_id, plan.install_scope).await,
        PackageSource::AppImage => appimage_update(&plan.package_id).await,
    }
}

async fn apt_update(pkg: &str) -> OperationResult {
    run_elevated(
        "apt",
        &["install", "-y", pkg],
        AuthMethod::Pkexec,
        UPDATE_TIMEOUT,
    )
    .await
}

async fn snap_refresh(pkg: &str) -> OperationResult {
    run_elevated(
        "snap",
        &["refresh", pkg],
        AuthMethod::Pkexec,
        UPDATE_TIMEOUT,
    )
    .await
}

async fn flatpak_update(app_id: &str, scope: Option<InstallScope>) -> OperationResult {
    let (auth, args): (AuthMethod, Vec<&str>) = match scope {
        Some(InstallScope::User) => (AuthMethod::None, vec!["update", "-y", "--user", app_id]),
        Some(InstallScope::System) | None => (AuthMethod::Pkexec, vec!["update", "-y", "--system", app_id]),
    };
    run_elevated("flatpak", &args, auth, UPDATE_TIMEOUT).await
}

async fn appimage_update(path: &str) -> OperationResult {
    // AppImage auto-update is complex: requires AppImageUpdate tool or manual
    // download-and-replace. For v1 we report the capability as not-yet-implemented
    // so users know it is expected in a future release.
    let _ = path;
    OperationResult {
        success: false,
        message: "AppImage auto-update is not yet implemented. Download the latest version from the project website.".into(),
        logs: String::new(),
        exit_code: None,
    }
}
