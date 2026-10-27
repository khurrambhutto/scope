//! Update preview, fail-closed revalidation, and apply commands.

use std::time::Duration;

use anyhow::Result;

use crate::domain::package::{InstallScope, InstalledPackage, PackageSource};
use crate::domain::safety;
use crate::domain::system::run_elevated;

use super::{
    apt_transaction, new_plan_id, now_ms, AuthMethod, Operation, OperationPlan, OperationResult,
    OperationTarget, PlanStep,
};

const UPDATE_TIMEOUT: Duration = Duration::from_secs(300);

pub async fn preview(pkg: &InstalledPackage) -> Result<OperationPlan> {
    if pkg.source == PackageSource::JetBrainsToolbox {
        anyhow::bail!("JetBrains Toolbox manages IDE updates and uninstall.")
    }
    if !matches!(pkg.source, PackageSource::Steam | PackageSource::AppImage)
        && (pkg.version.trim().is_empty() || pkg.version.eq_ignore_ascii_case("unknown"))
    {
        anyhow::bail!("Installed version is unknown. Rescan before updating.")
    }
    let target = OperationTarget::from_package(pkg)?;
    if let OperationTarget::Tool {
        package_id,
        installation,
    } = &target
    {
        crate::domain::tool_managers::operations::verify(package_id, installation, &pkg.version)
            .await?;
    }
    let protection = safety::check_target(&target);
    let (auth, mut steps) = if let OperationTarget::Tool {
        package_id,
        installation,
    } = &target
    {
        (
            AuthMethod::None,
            crate::domain::tool_managers::operations::steps(
                package_id,
                installation,
                Operation::Update,
                pkg.update_version.as_deref().unwrap_or("latest"),
            )?,
        )
    } else if let (
        OperationTarget::AppImage {
            installation: Some(installation),
            ..
        },
        false,
    ) = (&target, protection.protected)
    {
        crate::domain::appimage::operations::verify(installation)?;
        (
            AuthMethod::None,
            crate::domain::appimage::operations::steps(installation, Operation::Update)?,
        )
    } else {
        build_steps(pkg, &target, protection.protected)
    };
    let fingerprint = if matches!(target, OperationTarget::Apt { .. }) && !protection.protected {
        let transaction = apt_transaction::simulate(Operation::Update, target.id()).await?;
        steps.push(PlanStep {
            description: format!(
                "APT simulation changes {} package(s) and removes {}.",
                transaction.installs.len() + transaction.upgrades.len(),
                transaction.removals.len()
            ),
            command_summary: "apt-get -s install (verified again before apply)".into(),
        });
        Some(transaction.fingerprint)
    } else {
        None
    };
    Ok(OperationPlan {
        plan_id: new_plan_id(),
        operation: Operation::Update,
        target,
        display_name: pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone()),
        current_version: pkg.version.clone(),
        target_version: pkg
            .update_version
            .clone()
            .unwrap_or_else(|| "latest".into()),
        requires_auth: matches!(auth, AuthMethod::Pkexec),
        auth_method: auth,
        protected: protection.protected,
        protection_reason: protection.reason,
        steps,
        created_at_ms: now_ms(),
        apt_transaction_fingerprint: fingerprint,
        snap_details: None,
        app_data: Vec::new(),
    })
}

fn build_steps(
    pkg: &InstalledPackage,
    operation_target: &OperationTarget,
    protected: bool,
) -> (AuthMethod, Vec<PlanStep>) {
    if protected {
        return (
            AuthMethod::None,
            vec![PlanStep {
                description: "Blocked: this package is protected and cannot be updated.".into(),
                command_summary: "(no command — protected)".into(),
            }],
        );
    }
    let version = pkg.update_version.as_deref().unwrap_or("latest");
    match operation_target {
        OperationTarget::Tool { .. } => blocked("Tool installation requires verified metadata."),
        OperationTarget::Apt { package_id } => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!(
                    "Update APT package '{package_id}' from {} to {version}.",
                    pkg.version
                ),
                command_summary: format!(
                    "pkexec env DEBIAN_FRONTEND=noninteractive apt install -y {package_id}"
                ),
            }],
        ),
        OperationTarget::Snap { package_id } => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Update Snap '{package_id}' to {version}."),
                command_summary: format!("pkexec snap refresh {package_id}"),
            }],
        ),
        OperationTarget::Flatpak { app_id, scope } => {
            let (auth, flag, prefix) = match scope {
                InstallScope::User => (AuthMethod::None, "--user", "flatpak"),
                InstallScope::System => (AuthMethod::Pkexec, "--system", "pkexec flatpak"),
            };
            (
                auth,
                vec![PlanStep {
                    description: format!("Update Flatpak '{app_id}' to {version}."),
                    command_summary: format!("{prefix} update -y {flag} {app_id}"),
                }],
            )
        }
        OperationTarget::AppImage { .. }
        | OperationTarget::AppImageInstall { .. }
        | OperationTarget::DebInstall { .. } => {
            blocked("AppImage integration metadata is missing. Rescan first.")
        }
        OperationTarget::Desktop { .. } => {
            blocked("This app is not managed by a supported package manager.")
        }
        OperationTarget::Steam { .. } => {
            blocked("Steam items are read-only in Scope. Manage updates in Steam.")
        }
        OperationTarget::JetBrainsToolbox { .. } => {
            blocked("JetBrains Toolbox manages IDE updates and uninstall.")
        }
    }
}

fn blocked(message: &str) -> (AuthMethod, Vec<PlanStep>) {
    (
        AuthMethod::None,
        vec![PlanStep {
            description: format!("Blocked: {message}"),
            command_summary: "(no command — not supported)".into(),
        }],
    )
}

pub fn revalidate(plan: &OperationPlan, probed: &super::probe::ProbedPackage) -> Result<()> {
    if matches!(plan.target, OperationTarget::JetBrainsToolbox { .. }) {
        anyhow::bail!("JetBrains Toolbox manages IDE updates and uninstall.")
    }
    if !probed.present {
        anyhow::bail!(
            "This update plan is stale: '{}' is no longer installed.",
            plan.display_name
        )
    }
    let protection = safety::check_target(&plan.target);
    if protection.protected {
        anyhow::bail!(
            "Refusing to update protected package: {}",
            protection.reason.unwrap_or_else(|| "protected".into())
        )
    }
    if let OperationTarget::AppImage {
        installation: Some(installation),
        ..
    } = &plan.target
    {
        // AppImage identity is the verified file (size and mtime), checked by
        // the probe; launcher version labels are informational only.
        if installation.update.is_none() {
            anyhow::bail!("'{}' no longer has a verified update.", plan.display_name)
        }
        return Ok(());
    }
    if probed.has_update == Some(false) {
        anyhow::bail!("'{}' no longer has an update.", plan.display_name)
    }
    let version = probed
        .version
        .as_deref()
        .filter(|version| !version.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Could not verify the installed version."))?;
    if plan.current_version.trim().is_empty()
        || plan.current_version.eq_ignore_ascii_case("unknown")
        || version != plan.current_version
    {
        anyhow::bail!(
            "This update plan is stale: '{}' changed since the preview.",
            plan.display_name
        )
    }
    Ok(())
}

pub async fn apply(
    plan: &OperationPlan,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> OperationResult {
    match &plan.target {
        OperationTarget::Tool {
            package_id,
            installation,
        } => {
            crate::domain::tool_managers::operations::apply(plan, package_id, installation, on_line)
                .await
        }
        OperationTarget::Apt { package_id } => {
            run_elevated(
                "apt",
                &["install", "-y", package_id],
                AuthMethod::Pkexec,
                UPDATE_TIMEOUT,
                on_line,
            )
            .await
        }
        OperationTarget::Snap { package_id } => {
            let result = run_elevated(
                "snap",
                &["refresh", package_id],
                AuthMethod::Pkexec,
                UPDATE_TIMEOUT,
                on_line,
            )
            .await;
            snap_update_result(result, package_id, &plan.display_name)
        }
        OperationTarget::Flatpak { app_id, scope } => {
            let (auth, flag) = match scope {
                InstallScope::User => (AuthMethod::None, "--user"),
                InstallScope::System => (AuthMethod::Pkexec, "--system"),
            };
            run_elevated(
                "flatpak",
                &["update", "-y", flag, app_id],
                auth,
                UPDATE_TIMEOUT,
                on_line,
            )
            .await
        }
        OperationTarget::AppImage {
            installation: Some(installation),
            ..
        } => crate::domain::appimage::operations::update(installation, on_line).await,
        OperationTarget::AppImage { .. }
        | OperationTarget::AppImageInstall { .. }
        | OperationTarget::DebInstall { .. }
        | OperationTarget::Desktop { .. } => OperationResult {
            success: false,
            message: "This app cannot be updated through Scope.".into(),
            logs: String::new(),
            exit_code: None,
        },
        OperationTarget::Steam { .. } => OperationResult {
            success: false,
            message: "Steam items are read-only in Scope. Manage updates in Steam.".into(),
            logs: String::new(),
            exit_code: None,
        },
        OperationTarget::JetBrainsToolbox { .. } => OperationResult {
            success: false,
            message: "JetBrains Toolbox manages IDE updates and uninstall.".into(),
            logs: String::new(),
            exit_code: None,
        },
    }
}

fn snap_update_result(
    mut result: OperationResult,
    package_id: &str,
    display_name: &str,
) -> OperationResult {
    let running_apps = format!("snap \"{package_id}\" has running apps");
    if !result.success
        && (result.message.contains(&running_apps) || result.logs.contains(&running_apps))
    {
        result.message = format!(
            "{display_name} is still running. Quit it completely, then try updating again."
        );
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::operations::probe::ProbedPackage;
    use crate::domain::package::PackageSource;

    #[test]
    fn snap_running_app_error_explains_how_to_retry() {
        let result = OperationResult {
            success: false,
            message: "Operation failed (exit Some(1)): error: cannot refresh \"bitwarden\": snap \"bitwarden\" has running apps".into(),
            logs: "error: cannot refresh \"bitwarden\": snap \"bitwarden\" has running apps".into(),
            exit_code: Some(1),
        };
        let original_logs = result.logs.clone();
        let result = snap_update_result(result, "bitwarden", "Bitwarden");

        assert_eq!(
            result.message,
            "Bitwarden is still running. Quit it completely, then try updating again."
        );
        assert!(!result.success);
        assert_eq!(result.exit_code, Some(1));
        assert_eq!(result.logs, original_logs);
    }

    #[test]
    fn snap_running_app_error_is_found_in_multiline_logs() {
        let result = OperationResult {
            success: false,
            message: "Operation failed (exit Some(1)): error: cannot perform the following tasks:".into(),
            logs: "error: cannot perform the following tasks:\n- Ensure prerequisites (snap \"bitwarden\" has running apps (bitwarden), pids: 1234)".into(),
            exit_code: Some(1),
        };

        assert_eq!(
            snap_update_result(result, "bitwarden", "Bitwarden").message,
            "Bitwarden is still running. Quit it completely, then try updating again."
        );
    }

    #[test]
    fn snap_other_failures_keep_the_original_message() {
        for error in [
            "error: cannot communicate with server",
            "error: snap \"another-app\" has running apps",
        ] {
            let result = OperationResult {
                success: false,
                message: error.into(),
                logs: error.into(),
                exit_code: Some(1),
            };

            assert_eq!(
                snap_update_result(result, "bitwarden", "Bitwarden").message,
                error
            );
        }
    }

    #[test]
    fn snap_success_keeps_the_original_message() {
        let result = OperationResult {
            success: true,
            message: "Operation completed successfully.".into(),
            logs: "snap \"bitwarden\" has running apps".into(),
            exit_code: Some(0),
        };

        assert_eq!(
            snap_update_result(result, "bitwarden", "Bitwarden").message,
            "Operation completed successfully."
        );
    }

    fn plan(version: &str) -> OperationPlan {
        OperationPlan {
            plan_id: "plan".into(),
            operation: Operation::Update,
            target: OperationTarget::Apt {
                package_id: "gimp".into(),
            },
            display_name: "GIMP".into(),
            current_version: version.into(),
            target_version: "2.0".into(),
            requires_auth: true,
            auth_method: AuthMethod::Pkexec,
            protected: false,
            protection_reason: None,
            steps: vec![],
            created_at_ms: 0,
            apt_transaction_fingerprint: Some("fingerprint".into()),
            snap_details: None,
            app_data: Vec::new(),
        }
    }

    fn present(version: Option<&str>, update: Option<bool>) -> ProbedPackage {
        ProbedPackage {
            present: true,
            version: version.map(str::to_string),
            has_update: update,
        }
    }

    #[test]
    fn revalidate_accepts_only_known_unchanged_version_and_confirmed_update() {
        assert!(revalidate(&plan("1.0"), &present(Some("1.0"), Some(true))).is_ok());
    }

    #[test]
    fn revalidate_rejects_unknown_versions() {
        assert!(revalidate(&plan(""), &present(Some("1.0"), Some(true))).is_err());
        assert!(revalidate(&plan("1.0"), &present(None, Some(true))).is_err());
    }

    #[test]
    fn revalidate_accepts_unknown_update_state_when_version_is_known() {
        assert!(revalidate(&plan("1.0"), &present(Some("1.0"), None)).is_ok());
    }

    #[test]
    fn flatpak_preview_requires_scope() {
        let mut package = InstalledPackage::new(PackageSource::Flatpak, "org.gimp.GIMP");
        package.name = "GIMP".into();
        package.version = "1.0".into();
        assert!(futures::executor::block_on(preview(&package)).is_err());
    }

    #[test]
    fn appimage_preview_allows_unknown_version() {
        let mut package = InstalledPackage::new(PackageSource::AppImage, "/tmp/tool.AppImage");
        package.name = "Tool".into();
        package.version = "unknown".into();

        let plan = futures::executor::block_on(preview(&package)).unwrap();

        assert_eq!(plan.current_version, "unknown");
        assert!(matches!(plan.target, OperationTarget::AppImage { .. }));
    }

    #[test]
    fn steam_preview_is_protected() {
        let package = InstalledPackage::new(PackageSource::Steam, "570");
        let plan = futures::executor::block_on(preview(&package)).unwrap();

        assert!(plan.protected);
        assert!(matches!(plan.target, OperationTarget::Steam { .. }));
    }

    #[test]
    fn steam_revalidation_and_apply_fail_closed() {
        let mut plan = plan("1");
        plan.target = OperationTarget::Steam {
            app_id: "570".into(),
        };

        assert!(revalidate(&plan, &present(Some("1"), Some(true))).is_err());
        let result = futures::executor::block_on(apply(&plan, &|_| {}));
        assert!(!result.success);
    }
    #[test]
    fn toolbox_preview_rejects_even_when_has_update_is_forged() {
        let mut package = InstalledPackage::new(
            PackageSource::JetBrainsToolbox,
            "/home/user/.local/share/JetBrains/Toolbox/apps/pycharm",
        );
        package.version = "2026.2 (build)".into();
        package.has_update = true;
        package.update_version = Some("forged".into());

        assert!(futures::executor::block_on(preview(&package)).is_err());
    }

    #[test]
    fn toolbox_forged_target_revalidation_and_apply_fail_closed() {
        let mut plan = plan("2026.2 (build)");
        plan.target = OperationTarget::JetBrainsToolbox {
            installation_root: "/home/user/.local/share/JetBrains/Toolbox/apps/pycharm".into(),
        };

        assert!(revalidate(&plan, &present(Some("2026.2 (build)"), Some(true))).is_err());
        let result = futures::executor::block_on(apply(&plan, &|_| {}));
        assert!(!result.success);
    }
}
