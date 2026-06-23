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
use crate::system::which;

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
                description: format!("Move the AppImage '{}' to Trash.", pkg.package_id),
                command_summary: format!("gio trash {}", pkg.package_id),
            }],
        ),
    }
}

/// Re-validate that a package still exists on the system before applying.
/// Returns an error message string when the plan is stale.
pub async fn revalidate(plan: &OperationPlan, scan: &[InstalledPackage]) -> Result<()> {
    let still_present = scan.iter().any(|p| {
        p.source == plan.source
            && p.package_id == plan.package_id
            && p.install_scope == plan.install_scope
    });
    if !still_present {
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

/// Execute the plan's uninstall command for the given source, capturing logs.
pub async fn apply(plan: &OperationPlan) -> OperationResult {
    match plan.source {
        PackageSource::Apt => apt_remove(&plan.package_id).await,
        PackageSource::Snap => snap_remove(&plan.package_id).await,
        PackageSource::Flatpak => flatpak_uninstall(&plan.package_id, plan.install_scope).await,
        PackageSource::AppImage => appimage_trash(&plan.package_id).await,
    }
}

/// Resolve a binary to an absolute path (pkexec and env cleanup prefer this).
fn abs(bin: &str) -> String {
    for dir in ["/usr/bin", "/bin", "/usr/local/bin", "/usr/sbin", "/sbin"] {
        let p = format!("{dir}/{bin}");
        if std::path::Path::new(&p).is_file() {
            return p;
        }
    }
    bin.to_string()
}

/// Run a command with an optional `pkexec env DEBIAN_FRONTEND=noninteractive`
/// prefix, capturing combined output and enforcing a timeout.
async fn run_elevated(
    program: &str,
    args: &[&str],
    auth: AuthMethod,
    timeout: Duration,
) -> OperationResult {
    let program_abs = abs(program);
    let mut argv: Vec<String> = Vec::new();
    let display_program: String;
    match auth {
        AuthMethod::Pkexec => {
            // `env` lets us carry a minimal non-interactive env through pkexec,
            // which otherwise strips the environment.
            argv.push("env".into());
            argv.push("DEBIAN_FRONTEND=noninteractive".into());
            argv.push(program_abs.clone());
            display_program = format!("pkexec env DEBIAN_FRONTEND=noninteractive {program_abs}");
        }
        AuthMethod::None => {
            display_program = program_abs.clone();
        }
    }
    for a in args {
        argv.push((*a).to_string());
    }
    let argv_refs: Vec<&str> = argv.iter().map(String::as_str).collect();

    let mut cmd = match auth {
        AuthMethod::Pkexec => {
            let mut c = tokio::process::Command::new("pkexec");
            c.args(&argv_refs);
            c
        }
        AuthMethod::None => {
            let mut c = tokio::process::Command::new(&program_abs);
            c.args(
                &argv[if matches!(auth, AuthMethod::Pkexec) {
                    3
                } else {
                    0
                }..],
            );
            c
        }
    };

    let started = std::time::Instant::now();
    let output = tokio::time::timeout(timeout, cmd.output()).await;

    let elapsed = started.elapsed();
    let logs_suffix = format!(
        "\n[scope] ran: {} {:?} ({}ms)",
        display_program,
        args,
        elapsed.as_millis()
    );

    match output {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).to_string();
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            let logs = format!("--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}{logs_suffix}");
            let success = out.status.success();
            let exit_code = out.status.code();
            let message = if success {
                "Uninstall completed successfully.".to_string()
            } else {
                let first_err = stderr
                    .lines()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or("command failed");
                format!("Uninstall failed (exit {:?}): {}", exit_code, first_err)
            };
            OperationResult {
                success,
                message,
                logs,
                exit_code,
            }
        }
        Ok(Err(e)) => OperationResult {
            success: false,
            message: format!("Failed to start command: {e}"),
            logs: format!("spawn error: {e}{logs_suffix}"),
            exit_code: None,
        },
        Err(_) => OperationResult {
            success: false,
            message: format!("Uninstall timed out after {timeout:?}."),
            logs: format!("timed out after {timeout:?}{logs_suffix}"),
            exit_code: None,
        },
    }
}

async fn apt_remove(pkg: &str) -> OperationResult {
    run_elevated(
        "apt",
        &["remove", "-y", pkg],
        AuthMethod::Pkexec,
        UNINSTALL_TIMEOUT,
    )
    .await
}

async fn snap_remove(pkg: &str) -> OperationResult {
    run_elevated(
        "snap",
        &["remove", pkg],
        AuthMethod::Pkexec,
        UNINSTALL_TIMEOUT,
    )
    .await
}

async fn flatpak_uninstall(app_id: &str, scope: Option<InstallScope>) -> OperationResult {
    let (auth, args): (AuthMethod, Vec<&str>) = match scope {
        Some(InstallScope::User) => (AuthMethod::None, vec!["uninstall", "-y", "--user", app_id]),
        Some(InstallScope::System) | None => (
            AuthMethod::Pkexec,
            vec!["uninstall", "-y", "--system", app_id],
        ),
    };
    run_elevated("flatpak", &args, auth, UNINSTALL_TIMEOUT).await
}

async fn appimage_trash(path: &str) -> OperationResult {
    // Prefer the FreeDesktop trash via `gio trash` (restorable). Fallback to
    // moving into ~/.local/share/Trash/files when gio is unavailable.
    if which("gio") {
        let res = run_elevated(
            "gio",
            &["trash", "-f", path],
            AuthMethod::None,
            Duration::from_secs(20),
        )
        .await;
        if res.success {
            return res;
        }
        // Fall through to manual move if gio failed.
    }
    manual_trash(path).await
}

async fn manual_trash(path: &str) -> OperationResult {
    let Some(home) = std::env::var_os("HOME") else {
        return OperationResult {
            success: false,
            message: "No HOME directory; cannot trash AppImage.".into(),
            logs: String::new(),
            exit_code: None,
        };
    };
    let trash_files = std::path::Path::new(&home).join(".local/share/Trash/files");
    if let Err(e) = tokio::fs::create_dir_all(&trash_files).await {
        return OperationResult {
            success: false,
            message: format!("Could not create trash dir: {e}"),
            logs: format!("mkdir failed: {e}"),
            exit_code: None,
        };
    }
    let src = std::path::Path::new(path);
    let filename = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "appimage".into());
    let dest = trash_files.join(format!("{}.{}", filename, now_ms_debris()));
    match tokio::fs::rename(src, &dest).await {
        Ok(_) => OperationResult {
            success: true,
            message: "AppImage moved to Trash.".into(),
            logs: format!("moved {path} -> {}", dest.display()),
            exit_code: Some(0),
        },
        Err(e) => OperationResult {
            success: false,
            message: format!("Could not move AppImage to Trash: {e}"),
            logs: format!("rename failed: {e}"),
            exit_code: None,
        },
    }
}

fn now_ms_debris() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
