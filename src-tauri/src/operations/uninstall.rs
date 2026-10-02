//! Uninstall preview + apply per package source.
//!
//! Preview builds an [`OperationPlan`] from the live cached scan.
//! Apply revalidates that the package still exists and still passes the safety
//! check, then runs the source-specific command with proper auth and timeouts,
//! capturing logs for the UI.

use std::time::Duration;

use anyhow::Result;

use crate::package::{InstallScope, InstalledPackage, PackageSource};
use crate::safety;
use crate::system::run_elevated;

use super::{new_plan_id, now_ms, AuthMethod, Operation, OperationPlan, OperationResult, PlanStep};

/// Max time an uninstall command may run before we cancel it.
const UNINSTALL_TIMEOUT: Duration = Duration::from_secs(180);

/// Build a preview plan for removing one package identified by its backend key.
/// The package must come from the supplied scan so the frontend can never
/// nominate an arbitrary id we haven't seen.
pub fn preview(pkg: &InstalledPackage) -> OperationPlan {
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    let (auth, steps) = build_steps(pkg, protection.protected);

    OperationPlan {
        plan_id: new_plan_id(),
        operation: Operation::Uninstall,
        source: pkg.source,
        package_id: pkg.package_id.clone(),
        install_scope: pkg.install_scope,
        display_name: pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone()),
        current_version: pkg.version.clone(),
        target_version: String::new(),
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
                description: "Blocked: this package is protected and cannot be removed.".into(),
                command_summary: "(no command — protected)".into(),
            }],
        );
    }

    match pkg.source {
        PackageSource::Apt => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Remove the APT package '{}' via apt.", pkg.package_id),
                command_summary: format!(
                    "pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y {}",
                    pkg.package_id
                ),
            }],
        ),
        PackageSource::Snap => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Remove the Snap '{}' via snap remove.", pkg.package_id),
                command_summary: format!("pkexec snap remove {}", pkg.package_id),
            }],
        ),
        PackageSource::Flatpak => {
            let (auth, cmd) = match pkg.install_scope {
                Some(InstallScope::User) => (
                    AuthMethod::None,
                    format!("flatpak uninstall -y --user {}", pkg.package_id),
                ),
                Some(InstallScope::System) | None => (
                    AuthMethod::Pkexec,
                    format!("pkexec flatpak uninstall -y --system {}", pkg.package_id),
                ),
            };
            let where_label = match pkg.install_scope {
                Some(InstallScope::User) => "user",
                Some(InstallScope::System) => "system",
                None => "system (best-effort)",
            };
            (
                auth,
                vec![PlanStep {
                    description: format!(
                        "Uninstall the Flatpak '{}' ({} installation).",
                        pkg.package_id, where_label
                    ),
                    command_summary: cmd,
                }],
            )
        }
        PackageSource::AppImage => (
            AuthMethod::None,
            vec![PlanStep {
                description: "Blocked: AppImage uninstall is not supported yet.".into(),
                command_summary: "(no command — not supported yet)".into(),
            }],
        ),
    }
}

/// Re-validate that the package still exists and still passes the safety check
/// before applying. `probed` comes from [`super::probe::probe_package`], which
/// queries only this package instead of re-scanning the whole system — so the
/// confirmation-to-password-prompt gap stays near-instant.
pub fn revalidate(plan: &OperationPlan, probed: &super::probe::ProbedPackage) -> Result<()> {
    if !probed.present {
        anyhow::bail!(
            "This uninstall plan is stale: '{}' is no longer installed. Rescan and try again.",
            plan.display_name
        );
    }
    // Re-run the safety check in case state changed since preview.
    let protection = safety::check_package(plan.source, &plan.package_id);
    if protection.protected {
        anyhow::bail!(
            "Refusing to remove protected package: {}",
            protection.reason.unwrap_or_else(|| "protected".into())
        );
    }
    Ok(())
}

/// Execute the plan's uninstall command for the given source, streaming output.
///
/// `on_line` receives each output line as it is produced so the UI can show
/// live progress. AppImages never reach here: preview marks them protected and
/// apply-time revalidation refuses them, so only package-manager sources run.
pub async fn apply(
    plan: &OperationPlan,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    match plan.source {
        PackageSource::Apt => apt_remove(&plan.package_id, on_line).await,
        PackageSource::Snap => snap_remove(&plan.package_id, on_line).await,
        PackageSource::Flatpak => flatpak_uninstall(&plan.package_id, plan.install_scope, on_line).await,
        PackageSource::AppImage => {
            let _ = &plan.package_id;
            OperationResult {
                success: false,
                message: "AppImage uninstall is not supported yet.".into(),
                logs: String::new(),
                exit_code: None,
            }
        }
    }
}

async fn apt_remove(pkg: &str, on_line: &(dyn Fn(&str) + Send + Sync)) -> OperationResult {
    run_elevated(
        "apt",
        &["remove", "-y", pkg],
        AuthMethod::Pkexec,
        UNINSTALL_TIMEOUT,
        on_line,
    )
    .await
}

async fn snap_remove(pkg: &str, on_line: &(dyn Fn(&str) + Send + Sync)) -> OperationResult {
    run_elevated(
        "snap",
        &["remove", pkg],
        AuthMethod::Pkexec,
        UNINSTALL_TIMEOUT,
        on_line,
    )
    .await
}

async fn flatpak_uninstall(
    app_id: &str,
    scope: Option<InstallScope>,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let (auth, args): (AuthMethod, Vec<&str>) = match scope {
        Some(InstallScope::User) => (AuthMethod::None, vec!["uninstall", "-y", "--user", app_id]),
        Some(InstallScope::System) | None => (
            AuthMethod::Pkexec,
            vec!["uninstall", "-y", "--system", app_id],
        ),
    };
    run_elevated("flatpak", &args, auth, UNINSTALL_TIMEOUT, on_line).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::probe::ProbedPackage;

    fn plan(source: PackageSource, package_id: &str) -> OperationPlan {
        OperationPlan {
            plan_id: "plan-test-1".into(),
            operation: Operation::Uninstall,
            source,
            package_id: package_id.into(),
            install_scope: None,
            display_name: "Test".into(),
            current_version: "1.0".into(),
            target_version: String::new(),
            requires_auth: true,
            auth_method: AuthMethod::Pkexec,
            protected: false,
            protection_reason: None,
            steps: vec![],
            created_at_ms: 0,
        }
    }

    fn present() -> ProbedPackage {
        ProbedPackage {
            present: true,
            version: Some("1.0".into()),
            has_update: None,
        }
    }

    #[test]
    fn rejects_missing_package() {
        let p = plan(PackageSource::Apt, "gimp");
        let probed = ProbedPackage {
            present: false,
            version: None,
            has_update: None,
        };
        assert!(revalidate(&p, &probed).is_err());
    }

    #[test]
    fn rejects_protected_package_even_if_plan_says_otherwise() {
        let p = plan(PackageSource::Apt, "systemd");
        assert!(revalidate(&p, &present()).is_err());
    }

    #[test]
    fn rejects_protected_snap_runtime() {
        let p = plan(PackageSource::Snap, "core22");
        assert!(revalidate(&p, &present()).is_err());
    }

    #[test]
    fn accepts_present_allowed_package() {
        let p = plan(PackageSource::Apt, "gimp");
        assert!(revalidate(&p, &present()).is_ok());
    }

    /// AppImages are listed but not removable: preview must mark them
    /// protected and revalidation must refuse them even if a stale plan
    /// somehow reaches apply.
    #[test]
    fn appimage_preview_is_protected_and_revalidate_refuses() {
        let mut pkg = InstalledPackage::new(PackageSource::AppImage, "/opt/Foo-1.0.AppImage");
        pkg.name = "Foo".into();
        let p = preview(&pkg);
        assert!(p.protected);
        assert!(p.protection_reason.as_deref().unwrap().contains("yet"));
        assert!(revalidate(&p, &present()).is_err());
    }
}
