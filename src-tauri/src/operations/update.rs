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

/// Re-validate that the package still exists, still passes the safety check,
/// and still matches the state the plan was built from. `probed` comes from
/// [`super::probe::probe_package`] — one cheap query instead of a full rescan.
pub fn revalidate(plan: &OperationPlan, probed: &super::probe::ProbedPackage) -> Result<()> {
    if !probed.present {
        anyhow::bail!(
            "This update plan is stale: '{}' is no longer installed.",
            plan.display_name
        );
    }
    // Safety re-check (mirrors uninstall) in case deny-list state changed.
    let protection = safety::check_package(plan.source, &plan.package_id);
    if protection.protected {
        anyhow::bail!(
            "Refusing to update protected package: {}",
            protection.reason.unwrap_or_else(|| "protected".into())
        );
    }
    if probed.has_update == Some(false) {
        anyhow::bail!(
            "'{}' no longer has updates available. Rescan and try again.",
            plan.display_name
        );
    }
    if let Some(version) = &probed.version {
        if !plan.current_version.is_empty() && *version != plan.current_version {
            anyhow::bail!(
                "This update plan is stale: '{}' changed since the preview (expected version {}, found {}). Rescan and try again.",
                plan.display_name,
                plan.current_version,
                version
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::probe::ProbedPackage;

    fn plan(package_id: &str) -> OperationPlan {
        OperationPlan {
            plan_id: "plan-test-2".into(),
            operation: Operation::Update,
            source: PackageSource::Apt,
            package_id: package_id.into(),
            install_scope: None,
            display_name: "Test".into(),
            current_version: "1.0".into(),
            target_version: "2.0".into(),
            requires_auth: true,
            auth_method: AuthMethod::Pkexec,
            protected: false,
            protection_reason: None,
            steps: vec![],
            created_at_ms: 0,
        }
    }

    fn present(version: &str, has_update: Option<bool>) -> ProbedPackage {
        ProbedPackage {
            present: true,
            version: Some(version.into()),
            has_update,
        }
    }

    #[test]
    fn rejects_missing_package() {
        let p = plan("gimp");
        let probed = ProbedPackage {
            present: false,
            version: None,
            has_update: None,
        };
        assert!(revalidate(&p, &probed).is_err());
    }

    #[test]
    fn rejects_protected_package() {
        let p = plan("libc6");
        assert!(revalidate(&p, &present("1.0", None)).is_err());
    }

    #[test]
    fn rejects_when_update_disappeared() {
        let p = plan("gimp");
        assert!(revalidate(&p, &present("1.0", Some(false))).is_err());
    }

    #[test]
    fn rejects_when_version_changed_since_preview() {
        let p = plan("gimp");
        assert!(revalidate(&p, &present("2.0", None)).is_err());
    }

    #[test]
    fn accepts_unchanged_package_with_update_still_pending() {
        let p = plan("gimp");
        assert!(revalidate(&p, &present("1.0", Some(true))).is_ok());
    }

    #[test]
    fn accepts_when_update_state_unknown_but_version_unchanged() {
        let p = plan("gimp");
        assert!(revalidate(&p, &present("1.0", None)).is_ok());
    }

    #[test]
    fn accepts_when_version_unknown_but_present() {
        let p = plan("gimp");
        let probed = ProbedPackage {
            present: true,
            version: None,
            has_update: None,
        };
        assert!(revalidate(&p, &probed).is_ok());
    }

    #[test]
    fn accepts_when_preview_version_was_unknown() {
        let mut p = plan("gimp");
        p.current_version = String::new();
        assert!(revalidate(&p, &present("9.9", None)).is_ok());
    }
}
