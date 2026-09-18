//! Flatpak scanner.
//!
//! Strategy: `flatpak list --app` with explicit, tab-delimited columns. Only
//! applications are reported (runtimes are intentionally excluded). Flatpaks are
//! GUI-first; their `.desktop` ids equal the application id, which the desktop
//! enrichment step matches exactly.
//!
//! User and system installs are independent, so they are listed and checked
//! concurrently. `flatpak update --appstream` is global rather than per-scope, so
//! it runs once per scan and overlaps the listings.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};

use crate::package::{AppKind, InstallScope, InstalledPackage, PackageSource};
use crate::scanner::Scanner;
use crate::system::{capture_stdout, which, SCAN_TIMEOUT};

pub struct FlatpakScanner;

impl Scanner for FlatpakScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Flatpak
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("flatpak") })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>> {
        Box::pin(scan())
    }
}

async fn scan() -> Result<Vec<InstalledPackage>> {
    // AppStream metadata is shared by both scopes and may reach the network, so
    // refresh it once while the two scope listings run.
    let appstream = tokio::spawn(refresh_appstream());
    let (user, system) = tokio::join!(
        list_scope(InstallScope::User),
        list_scope(InstallScope::System)
    );
    let _ = appstream.await;

    let mut packages = merge_scopes(user, system)?;

    // Update checks read AppStream data, so they run after the refresh.
    let (user_updates, system_updates) = tokio::join!(
        available_updates(InstallScope::User),
        available_updates(InstallScope::System)
    );
    mark_updates(&mut packages, InstallScope::User, &user_updates);
    mark_updates(&mut packages, InstallScope::System, &system_updates);

    Ok(packages)
}

fn scope_flag(scope: InstallScope) -> &'static str {
    match scope {
        InstallScope::User => "--user",
        InstallScope::System => "--system",
    }
}

/// Refresh AppStream metadata. Best-effort: without it `remote-ls --updates`
/// still reports whatever the already-cached metadata knows about.
async fn refresh_appstream() {
    let _ = capture_stdout("flatpak", &["update", "--appstream"], SCAN_TIMEOUT).await;
}

/// Combine the two scope listings, tolerating one scope failing.
fn merge_scopes(
    user: Result<Vec<InstalledPackage>>,
    system: Result<Vec<InstalledPackage>>,
) -> Result<Vec<InstalledPackage>> {
    match (user, system) {
        (Ok(mut user_packages), Ok(mut system_packages)) => {
            user_packages.append(&mut system_packages);
            Ok(user_packages)
        }
        (Ok(packages), Err(_)) | (Err(_), Ok(packages)) => Ok(packages),
        (Err(user_err), Err(system_err)) => {
            Err(user_err.context(format!("flatpak system scan also failed: {system_err}")))
        }
    }
}

async fn list_scope(scope: InstallScope) -> Result<Vec<InstalledPackage>> {
    // Tab-delimited column output (flatpak columns default to this separator
    // when redirected / non-tty).
    let columns = "application,name,version,origin,size,description";
    let scope_arg = scope_flag(scope);
    let output = capture_stdout(
        "flatpak",
        &["list", scope_arg, "--app", &format!("--columns={columns}")],
        SCAN_TIMEOUT,
    )
    .await
    .context(format!("flatpak list {scope_arg}"))?;

    let mut packages = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let app_id = parts[0].to_string();
        let display_name = parts[1].to_string();
        let version = parts.get(2).copied().unwrap_or("").to_string();
        let origin = parts.get(3).copied().unwrap_or("");
        let size_str = parts.get(4).copied().unwrap_or("0");
        let description = parts.get(5).copied().unwrap_or("");

        let mut pkg = InstalledPackage::new_scoped(PackageSource::Flatpak, app_id, scope);
        pkg.name = display_name.clone();
        pkg.display_name = Some(display_name);
        pkg.version = version;
        pkg.size_bytes = parse_size(size_str);
        if !description.is_empty() {
            pkg.description = Some(description.to_string());
        }
        if !origin.is_empty() {
            // Tuck origin into description tail for the detail view; the dedicated
            // origin field would need a model change we leave for the detail phase.
            if let Some(d) = &mut pkg.description {
                d.push_str(&format!("  (remote: {origin})"));
            }
        }
        pkg.app_kind = AppKind::Gui;
        packages.push(pkg);
    }
    Ok(packages)
}

/// Available updates for one scope, keyed by application id.
async fn available_updates(scope: InstallScope) -> HashMap<String, Option<String>> {
    let mut updates = HashMap::new();
    let output = match capture_stdout(
        "flatpak",
        &[
            "remote-ls",
            "--updates",
            scope_flag(scope),
            "--columns=application,version",
        ],
        SCAN_TIMEOUT,
    )
    .await
    {
        Ok(o) => o,
        Err(_) => return updates,
    };

    // Tab-delimited: application_id\tversion
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(app_id) = parts.next() else { continue };
        let version = parts.next().filter(|v| !v.is_empty()).map(|s| s.to_string());
        updates.insert(app_id.to_string(), version);
    }
    updates
}

/// Flag packages from one scope that have an available update.
fn mark_updates(
    packages: &mut [InstalledPackage],
    scope: InstallScope,
    updates: &HashMap<String, Option<String>>,
) {
    if updates.is_empty() {
        return;
    }
    for pkg in packages
        .iter_mut()
        .filter(|p| p.install_scope == Some(scope))
    {
        if let Some(version) = updates.get(&pkg.package_id) {
            pkg.has_update = true;
            pkg.update_version = version.clone();
        }
    }
}

/// Parse human sizes like "384.1 MB", "1.2 GB" into bytes.
fn parse_size(size_str: &str) -> u64 {
    let parts: Vec<&str> = size_str.split_whitespace().collect();
    if parts.is_empty() {
        return 0;
    }
    let number: f64 = parts[0].replace(',', ".").parse().unwrap_or(0.0);
    let unit = parts.get(1).copied().unwrap_or("B").to_uppercase();
    let multiplier: u64 = match unit.as_str() {
        "B" => 1,
        "KB" | "K" => 1024,
        "MB" | "M" => 1024 * 1024,
        "GB" | "G" => 1024 * 1024 * 1024,
        "TB" | "T" => 1024 * 1024 * 1024 * 1024,
        _ => 1,
    };
    (number * multiplier as f64) as u64
}
