//! Update preview + apply per package source.
//!
//! Mirrors the uninstall flow: preview builds an [`OperationPlan`], apply
//! revalidates that the package still exists and still has an update, then runs
//! the source-specific update command.

use std::time::Duration;

use anyhow::Result;

use crate::domain::package::{InstallScope, InstalledPackage, PackageSource};
use crate::domain::safety;
use crate::domain::system::run_elevated;

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
                description: "Blocked: AppImage updates are not supported yet.".into(),
                command_summary: "(no command — not supported yet)".into(),
            }],
        ),
        PackageSource::Desktop => (
            AuthMethod::None,
            vec![PlanStep {
                description: "Blocked: this app is not managed by a supported package manager.".into(),
                command_summary: "(no command — unmanaged desktop app)".into(),
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

/// Execute the plan's update command for the given source, streaming output.
///
/// `on_line` receives each output line as it is produced so the UI can show
/// live progress during the possibly-lengthy download/install.
pub async fn apply(
    plan: &OperationPlan,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    match plan.source {
        PackageSource::Apt => apt_update(&plan.package_id, on_line).await,
        PackageSource::Snap => snap_refresh(&plan.package_id, on_line).await,
        PackageSource::Flatpak => flatpak_update(&plan.package_id, plan.install_scope, on_line).await,
        PackageSource::AppImage => OperationResult {
            success: false,
            message: "AppImage auto-update is not yet implemented. Download the latest version from the project website.".into(),
            logs: String::new(),
            exit_code: None,
        },
        PackageSource::Desktop => OperationResult {
            success: false,
            message: "This desktop app is not managed by a supported package manager.".into(),
            logs: String::new(),
            exit_code: None,
        },
    }
}

async fn apt_update(pkg: &str, on_line: &(dyn Fn(&str) + Send + Sync)) -> OperationResult {
    run_elevated(
        "apt",
        &["install", "-y", pkg],
        AuthMethod::Pkexec,
        UPDATE_TIMEOUT,
        on_line,
    )
    .await
}

async fn snap_refresh(pkg: &str, on_line: &(dyn Fn(&str) + Send + Sync)) -> OperationResult {
    run_elevated(
        "snap",
        &["refresh", pkg],
        AuthMethod::Pkexec,
        UPDATE_TIMEOUT,
        on_line,
    )
    .await
}

async fn flatpak_update(
    app_id: &str,
    scope: Option<InstallScope>,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    let (auth, args): (AuthMethod, Vec<&str>) = match scope {
        Some(InstallScope::User) => (AuthMethod::None, vec!["update", "-y", "--user", app_id]),
        Some(InstallScope::System) | None => (AuthMethod::Pkexec, vec!["update", "-y", "--system", app_id]),
    };
    run_elevated("flatpak", &args, auth, UPDATE_TIMEOUT, on_line).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::operations::probe::ProbedPackage;

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

    fn pkg(source: PackageSource, package_id: &str) -> InstalledPackage {
        let mut pkg = InstalledPackage::new(source, package_id);
        pkg.name = package_id.into();
        pkg.update_version = Some("2.0".into());
        pkg
    }

    fn present(version: &str, has_update: Option<bool>) -> ProbedPackage {
        ProbedPackage {
            present: true,
            version: Some(version.into()),
            has_update,
        }
    }

    #[test]
    fn revalidate_rejects_a_package_that_is_no_longer_installed() {
        let probed = ProbedPackage {
            present: false,
            version: None,
            has_update: None,
        };
        let err = revalidate(&plan("gimp"), &probed).unwrap_err();
        assert!(
            err.to_string().contains("no longer installed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn revalidate_rejects_a_protected_package() {
        let err = revalidate(&plan("libc6"), &present("1.0", None)).unwrap_err();
        assert!(
            err.to_string().contains("protected"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn revalidate_rejects_when_the_update_disappeared() {
        let err = revalidate(&plan("gimp"), &present("1.0", Some(false))).unwrap_err();
        assert!(
            err.to_string().contains("no longer has updates"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn revalidate_rejects_when_the_version_changed_since_preview() {
        let err = revalidate(&plan("gimp"), &present("2.0", None)).unwrap_err();
        assert!(
            err.to_string().contains("changed since the preview"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn revalidate_accepts_an_unchanged_package_with_an_update_pending() {
        assert!(revalidate(&plan("gimp"), &present("1.0", Some(true))).is_ok());
    }

    #[test]
    fn revalidate_accepts_when_update_state_is_unknown_but_version_is_unchanged() {
        assert!(revalidate(&plan("gimp"), &present("1.0", None)).is_ok());
    }

    #[test]
    fn revalidate_accepts_when_the_installed_version_is_unknown() {
        let probed = ProbedPackage {
            present: true,
            version: None,
            has_update: None,
        };
        assert!(revalidate(&plan("gimp"), &probed).is_ok());
    }

    #[test]
    fn revalidate_accepts_when_the_preview_version_was_unknown() {
        let mut p = plan("gimp");
        p.current_version = String::new();
        assert!(revalidate(&p, &present("9.9", None)).is_ok());
    }

    /// AppImage update plans can never execute: revalidation refuses them.
    #[test]
    fn revalidate_refuses_an_appimage() {
        let mut p = plan("/opt/Foo-1.0.AppImage");
        p.source = PackageSource::AppImage;
        assert!(revalidate(&p, &present("1.0", Some(true))).is_err());
    }

    #[test]
    fn preview_apt_uses_the_documented_pkexec_command() {
        let p = preview(&pkg(PackageSource::Apt, "gimp"));
        assert_eq!(
            p.steps[0].command_summary,
            "pkexec env DEBIAN_FRONTEND=noninteractive apt install -y gimp"
        );
    }

    #[test]
    fn preview_snap_uses_the_documented_pkexec_command() {
        let p = preview(&pkg(PackageSource::Snap, "code"));
        assert_eq!(p.steps[0].command_summary, "pkexec snap refresh code");
    }

    #[test]
    fn preview_flatpak_user_uses_the_user_scope_flag_without_auth() {
        let mut p = pkg(PackageSource::Flatpak, "org.gimp.GIMP");
        p.install_scope = Some(InstallScope::User);
        assert_eq!(
            preview(&p).steps[0].command_summary,
            "flatpak update -y --user org.gimp.GIMP"
        );
    }

    #[test]
    fn preview_flatpak_system_routes_through_pkexec() {
        let p = preview(&pkg(PackageSource::Flatpak, "org.gimp.GIMP"));
        assert_eq!(p.auth_method, AuthMethod::Pkexec);
    }
}
