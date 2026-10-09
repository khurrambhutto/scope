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
    if pkg.source != PackageSource::Steam
        && (pkg.version.trim().is_empty() || pkg.version.eq_ignore_ascii_case("unknown"))
    {
        anyhow::bail!("Installed version is unknown. Rescan before updating.")
    }
    let target = OperationTarget::from_package(pkg)?;
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    let (auth, mut steps) = build_steps(pkg, &target, protection.protected);
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
        OperationTarget::AppImage { .. } => blocked("AppImage updates are not supported yet."),
        OperationTarget::Desktop { .. } => {
            blocked("This app is not managed by a supported package manager.")
        },
        OperationTarget::Steam { .. } => {
            blocked("Steam items are read-only in Scope. Manage updates in Steam.")
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
    if !probed.present {
        anyhow::bail!(
            "This update plan is stale: '{}' is no longer installed.",
            plan.display_name
        )
    }
    let protection = safety::check_package(plan.target.source(), plan.target.id());
    if protection.protected {
        anyhow::bail!(
            "Refusing to update protected package: {}",
            protection.reason.unwrap_or_else(|| "protected".into())
        )
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
            run_elevated(
                "snap",
                &["refresh", package_id],
                AuthMethod::Pkexec,
                UPDATE_TIMEOUT,
                on_line,
            )
            .await
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
        OperationTarget::AppImage { .. } | OperationTarget::Desktop { .. } => OperationResult {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::operations::probe::ProbedPackage;
    use crate::domain::package::PackageSource;

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
}
