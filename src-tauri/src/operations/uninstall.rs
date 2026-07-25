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
use crate::system::{run_elevated, which};

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
                description: format!("Move the AppImage '{}' to Trash.", pkg.package_id),
                command_summary: format!("gio trash {}", pkg.package_id),
            }],
        ),
        PackageSource::Manual => {
            let (primary, desktop) = crate::scanner::split_manual_id(&pkg.package_id);
            let targets = manual_removal_targets(primary, desktop);
            let needs_pkexec = targets.iter().any(|t| t.starts_with("/opt/"));
            let auth = if needs_pkexec {
                AuthMethod::Pkexec
            } else {
                AuthMethod::None
            };
            let steps = targets
                .into_iter()
                .map(|t| {
                    let is_desktop = t.ends_with(".desktop");
                    PlanStep {
                        description: if is_desktop {
                            format!("Remove desktop entry '{t}'.")
                        } else {
                            format!("Move '{t}' to Trash.")
                        },
                        command_summary: if t.starts_with("/opt/") {
                            format!("pkexec gio trash {t}")
                        } else {
                            format!("gio trash {t}")
                        },
                    }
                })
                .collect();
            (auth, steps)
        }
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
        PackageSource::Manual => manual_trash_paths(&plan.package_id).await,
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
    trash_one(path, AuthMethod::None).await
}

async fn manual_trash_paths(package_id: &str) -> OperationResult {
    let (primary, desktop) = crate::scanner::split_manual_id(package_id);
    let targets = manual_removal_targets(primary, desktop);
    if targets.is_empty() {
        return OperationResult {
            success: false,
            message: "Nothing to remove for this manual install.".into(),
            logs: String::new(),
            exit_code: None,
        };
    }

    let mut logs = String::new();
    let mut removed = 0usize;
    for target in &targets {
        let auth = if target.starts_with("/opt/") {
            AuthMethod::Pkexec
        } else {
            AuthMethod::None
        };
        // Re-check each path at apply time.
        let check = safety::check_package(
            PackageSource::Manual,
            // Encode as a single primary so path guard runs on this target.
            target,
        );
        if check.protected {
            if !logs.is_empty() {
                logs.push('\n');
            }
            logs.push_str(&format!(
                "skip protected path {target}: {}",
                check.reason.as_deref().unwrap_or("protected")
            ));
            continue;
        }
        let res = trash_one(target, auth).await;
        if !logs.is_empty() {
            logs.push('\n');
        }
        logs.push_str(&res.logs);
        if res.success {
            removed += 1;
        } else if removed == 0 {
            // First target failed hard — abort.
            return OperationResult {
                success: false,
                message: res.message,
                logs,
                exit_code: res.exit_code,
            };
        } else {
            logs.push_str(&format!("\nwarning: failed to trash {target}: {}", res.message));
        }
    }

    if removed == 0 {
        return OperationResult {
            success: false,
            message: "Could not remove any files for this manual install.".into(),
            logs,
            exit_code: None,
        };
    }

    OperationResult {
        success: true,
        message: format!("Manual install moved to Trash ({removed} item(s))."),
        logs,
        exit_code: Some(0),
    }
}

/// Build the list of paths to trash for a manual install.
///
/// Prefer the install root (e.g. `Foo.app`, `~/.local/opt/Foo-x64`, `/opt/foo`)
/// over a nested binary, and also drop matching `~/.local/bin` symlinks.
fn manual_removal_targets(primary: &str, desktop: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let primary_path = std::path::Path::new(primary);

    // If primary is only a .desktop (binary never resolved), just trash that.
    if primary.ends_with(".desktop") {
        out.push(primary.to_string());
        return out;
    }

    let root = install_root_for(primary_path).unwrap_or_else(|| primary.to_string());
    out.push(root.clone());

    // PATH shim: ~/.local/bin/<name> → this binary (or into the install root).
    if let Some(home) = std::env::var_os("HOME") {
        let bin_dir = std::path::Path::new(&home).join(".local/bin");
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_symlink() {
                    continue;
                }
                let Ok(target) = std::fs::read_link(&p) else {
                    continue;
                };
                let resolved = if target.is_absolute() {
                    target
                } else {
                    bin_dir.join(target)
                };
                let resolved_s = resolved.to_string_lossy();
                if resolved_s == primary
                    || resolved_s.starts_with(&format!("{root}/"))
                    || resolved_s == root
                {
                    out.push(p.to_string_lossy().into_owned());
                }
            }
        }
    }

    if let Some(d) = desktop {
        if !out.iter().any(|x| x == d) {
            out.push(d.to_string());
        }
    }

    // De-dupe while preserving order.
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.clone()));
    out
}

/// If `binary` sits inside a known install bundle, return that bundle root.
fn install_root_for(binary: &std::path::Path) -> Option<String> {
    let s = binary.to_string_lossy();

    // macOS-style / Zed: .../Something.app/bin/zed → Something.app
    if let Some(idx) = s.find(".app/") {
        let root = &s[..=idx + 3]; // include ".app"
        if std::path::Path::new(root).is_dir() {
            return Some(root.to_string());
        }
    }

    // ~/.local/opt/<bundle>/binary → <bundle>
    if let Some(home) = std::env::var_os("HOME") {
        let opt = std::path::Path::new(&home).join(".local/opt");
        if let Ok(opt_s) = opt.canonicalize() {
            if let Ok(bin_can) = binary.canonicalize() {
                if bin_can.starts_with(&opt_s) {
                    // first component under .local/opt
                    if let Ok(rel) = bin_can.strip_prefix(&opt_s) {
                        if let Some(first) = rel.components().next() {
                            let root = opt_s.join(first);
                            if root.is_dir() {
                                return Some(root.to_string_lossy().into_owned());
                            }
                        }
                    }
                }
            }
        }
    }

    // /opt/<bundle>/... → /opt/<bundle>
    if s.starts_with("/opt/") {
        let rest = &s["/opt/".len()..];
        if let Some(bundle) = rest.split('/').next() {
            if !bundle.is_empty() {
                let root = format!("/opt/{bundle}");
                if std::path::Path::new(&root).is_dir() {
                    return Some(root);
                }
            }
        }
    }

    // ~/Downloads/<bundle>/binary → <bundle> (common for JetBrains tarballs)
    if let Some(home) = std::env::var_os("HOME") {
        let downloads = std::path::Path::new(&home).join("Downloads");
        if let (Ok(dl), Ok(bin_can)) = (downloads.canonicalize(), binary.canonicalize()) {
            if bin_can.starts_with(&dl) {
                if let Ok(rel) = bin_can.strip_prefix(&dl) {
                    if let Some(first) = rel.components().next() {
                        let root = dl.join(first);
                        // Only treat as bundle if it looks like a directory install
                        // (has bin/ or is multi-file), not a lone downloaded file.
                        if root.is_dir() && (root.join("bin").is_dir() || root.join("lib").is_dir())
                        {
                            return Some(root.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
    }

    None
}

async fn trash_one(path: &str, auth: AuthMethod) -> OperationResult {
    // Prefer the FreeDesktop trash via `gio trash` (restorable). Fallback to
    // moving into ~/.local/share/Trash/files when gio is unavailable.
    if which("gio") {
        let res = run_elevated(
            "gio",
            &["trash", "-f", path],
            auth,
            Duration::from_secs(20),
        )
        .await;
        if res.success {
            return res;
        }
        // Fall through to manual move if gio failed (and no elevation needed).
        if matches!(auth, AuthMethod::Pkexec) {
            return res;
        }
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
