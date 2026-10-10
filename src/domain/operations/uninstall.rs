//! Uninstall preview, fail-closed revalidation, and apply commands.

use std::time::Duration;

use anyhow::Result;

use crate::domain::package::{InstallScope, InstalledPackage};
use crate::domain::safety;
use crate::domain::system::run_elevated;

use super::{
    apt_transaction, new_plan_id, now_ms, AuthMethod, Operation, OperationPlan, OperationResult,
    OperationTarget, PlanStep,
};

const UNINSTALL_TIMEOUT: Duration = Duration::from_secs(180);

pub async fn preview(pkg: &InstalledPackage) -> Result<OperationPlan> {
    let target = OperationTarget::from_package(pkg)?;
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    let (auth, mut steps) = build_steps(&target, protection.protected);
    let fingerprint = if matches!(target, OperationTarget::Apt { .. }) && !protection.protected {
        let transaction = apt_transaction::simulate(Operation::Uninstall, target.id()).await?;
        steps.push(PlanStep {
            description: format!(
                "APT simulation removes {} package(s), including dependencies.",
                transaction.removals.len()
            ),
            command_summary: "apt-get -s remove (verified again before apply)".into(),
        });
        Some(transaction.fingerprint)
    } else {
        None
    };
    let snap_details = if matches!(target, OperationTarget::Snap { .. }) && !protection.protected {
        Some(Box::new(
            crate::domain::snap_details::inspect(
                target.id(),
                std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            )
            .await,
        ))
    } else {
        None
    };
    Ok(OperationPlan {
        plan_id: new_plan_id(),
        operation: Operation::Uninstall,
        target,
        display_name: pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone()),
        current_version: pkg.version.clone(),
        target_version: String::new(),
        requires_auth: matches!(auth, AuthMethod::Pkexec),
        auth_method: auth,
        protected: protection.protected,
        protection_reason: protection.reason,
        steps,
        created_at_ms: now_ms(),
        apt_transaction_fingerprint: fingerprint,
        snap_details,
    })
}

fn build_steps(target: &OperationTarget, protected: bool) -> (AuthMethod, Vec<PlanStep>) {
    if protected {
        return blocked("This package is protected and cannot be removed.");
    }
    match target {
        OperationTarget::Apt { package_id } => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Remove the APT package '{package_id}' via apt."),
                command_summary: format!(
                    "pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y {package_id}"
                ),
            }],
        ),
        OperationTarget::Snap { package_id } => (
            AuthMethod::Pkexec,
            vec![PlanStep {
                description: format!("Remove the Snap '{package_id}' via snap remove."),
                command_summary: format!("pkexec snap remove {package_id}"),
            }],
        ),
        OperationTarget::Flatpak { app_id, scope } => {
            let (auth, flag, prefix, label) = match scope {
                InstallScope::User => (AuthMethod::None, "--user", "flatpak", "user"),
                InstallScope::System => {
                    (AuthMethod::Pkexec, "--system", "pkexec flatpak", "system")
                }
            };
            (
                auth,
                vec![PlanStep {
                    description: format!("Uninstall Flatpak '{app_id}' from the {label} scope."),
                    command_summary: format!("{prefix} uninstall -y {flag} {app_id}"),
                }],
            )
        }
        OperationTarget::AppImage { .. } => blocked("AppImage uninstall is not supported yet."),
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
            command_summary: "(no command — protected)".into(),
        }],
    )
}

pub fn revalidate(plan: &OperationPlan, probed: &super::probe::ProbedPackage) -> Result<()> {
    if !probed.present {
        anyhow::bail!(
            "This uninstall plan is stale: '{}' is no longer installed. Rescan and try again.",
            plan.display_name
        )
    }
    let protection = safety::check_package(plan.target.source(), plan.target.id());
    if protection.protected {
        anyhow::bail!(
            "Refusing to remove protected package: {}",
            protection.reason.unwrap_or_else(|| "protected".into())
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
                &["remove", "-y", package_id],
                AuthMethod::Pkexec,
                UNINSTALL_TIMEOUT,
                on_line,
            )
            .await
        }
        OperationTarget::Snap { package_id } => {
            run_elevated(
                "snap",
                &["remove", package_id],
                AuthMethod::Pkexec,
                UNINSTALL_TIMEOUT,
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
                &["uninstall", "-y", flag, app_id],
                auth,
                UNINSTALL_TIMEOUT,
                on_line,
            )
            .await
        }
        OperationTarget::AppImage { .. } | OperationTarget::Desktop { .. } => OperationResult {
            success: false,
            message: "This app cannot be uninstalled through Scope.".into(),
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

    fn plan(target: OperationTarget) -> OperationPlan {
        OperationPlan {
            plan_id: "plan".into(),
            operation: Operation::Uninstall,
            target,
            display_name: "Test".into(),
            current_version: "1.0".into(),
            target_version: String::new(),
            requires_auth: true,
            auth_method: AuthMethod::Pkexec,
            protected: false,
            protection_reason: None,
            steps: vec![],
            created_at_ms: 0,
            apt_transaction_fingerprint: None,
            snap_details: None,
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
    fn snap_removal_preserves_normal_snapshot_behavior() {
        let target = OperationTarget::Snap {
            package_id: "code".into(),
        };
        let (auth, steps) = build_steps(&target, false);
        assert_eq!(auth, AuthMethod::Pkexec);
        assert_eq!(steps[0].command_summary, "pkexec snap remove code");
    }

    #[test]
    fn revalidate_rejects_a_protected_package() {
        let target = OperationTarget::Apt {
            package_id: "systemd".into(),
        };
        assert!(revalidate(&plan(target), &present()).is_err());
    }

    #[test]
    fn flatpak_target_requires_an_explicit_scope() {
        let package = InstalledPackage::new(PackageSource::Flatpak, "org.gimp.GIMP");
        assert!(OperationTarget::from_package(&package).is_err());
    }

    #[test]
    fn flatpak_user_command_uses_user_scope_without_auth() {
        let target = OperationTarget::Flatpak {
            app_id: "org.gimp.GIMP".into(),
            scope: InstallScope::User,
        };
        let (auth, steps) = build_steps(&target, false);
        assert_eq!(auth, AuthMethod::None);
        assert_eq!(
            steps[0].command_summary,
            "flatpak uninstall -y --user org.gimp.GIMP"
        );
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
        let plan = plan(OperationTarget::Steam {
            app_id: "570".into(),
        });

        assert!(revalidate(&plan, &present()).is_err());
        let result = futures::executor::block_on(apply(&plan, &|_| {}));
        assert!(!result.success);
    }
}
