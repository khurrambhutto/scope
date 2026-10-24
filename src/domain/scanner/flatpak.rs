//! Flatpak scanner.
//!
//! Strategy: `flatpak list --app` with explicit, tab-delimited columns. Only
//! applications are reported (runtimes are intentionally excluded). Flatpaks are
//! GUI-first; their `.desktop` ids equal the application id, which the desktop
//! enrichment step matches exactly.
//!
//! User and system installs are independent, so they are listed and checked
//! concurrently. `flatpak update --appstream` is global rather than per-scope, so
//! it runs once per scan after the local Flatpak inventory is ready.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};

use crate::domain::package::{AppKind, InstallScope, InstalledPackage, PackageSource};
use crate::domain::scanner::{ScanReport, Scanner};
use crate::domain::system::{capture_stdout, which, SCAN_TIMEOUT};

pub struct FlatpakScanner;

impl Scanner for FlatpakScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Flatpak
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("flatpak") })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>> {
        Box::pin(scan())
    }

    fn check_updates<'a>(
        &'a self,
        report: &'a mut ScanReport,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(check_updates(report))
    }
}

async fn scan() -> Result<ScanReport> {
    let (user, system) = tokio::join!(
        list_scope(InstallScope::User),
        list_scope(InstallScope::System)
    );
    let (packages, warning) = merge_scopes(user, system)?;
    Ok(ScanReport { packages, warning })
}

async fn check_updates(report: &mut ScanReport) {
    if report.packages.is_empty() {
        return;
    }
    refresh_appstream().await;
    let (user_updates, system_updates) = tokio::join!(
        available_updates_for_report(report, InstallScope::User),
        available_updates_for_report(report, InstallScope::System)
    );
    apply_scope_updates(report, InstallScope::User, user_updates);
    apply_scope_updates(report, InstallScope::System, system_updates);
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
///
/// A single-scope failure is not fatal, but it is no longer silent: the
/// surviving packages come back alongside a warning that `scan_all` surfaces to
/// the UI, so a half-empty Flatpak list is explained instead of invisible.
fn merge_scopes(
    user: Result<Vec<InstalledPackage>>,
    system: Result<Vec<InstalledPackage>>,
) -> Result<(Vec<InstalledPackage>, Option<String>)> {
    match (user, system) {
        (Ok(mut user_packages), Ok(mut system_packages)) => {
            user_packages.append(&mut system_packages);
            Ok((user_packages, None))
        }
        (Ok(packages), Err(err)) => Ok((
            packages,
            Some(format!("flatpak system scan failed: {err:#}")),
        )),
        (Err(err), Ok(packages)) => {
            Ok((packages, Some(format!("flatpak user scan failed: {err:#}"))))
        }
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
        let mut fields = line.split('\t');
        let (Some(app_id), Some(display_name), Some(version)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let origin = fields.next().unwrap_or("");
        let size_str = fields.next().unwrap_or("0");
        let description = fields.next().unwrap_or("");

        let mut pkg = InstalledPackage::new_scoped(PackageSource::Flatpak, app_id, scope);
        pkg.name = display_name.to_string();
        pkg.display_name = Some(pkg.name.clone());
        pkg.version = version.to_string();
        pkg.size_bytes = parse_size(size_str);
        if !description.is_empty() {
            pkg.description = Some(description.to_string());
        }
        if !origin.is_empty() {
            // Tuck origin into description tail for the detail view; the dedicated
            // origin field would need a model change we leave for the detail phase.
            if let Some(d) = &mut pkg.description {
                use std::fmt::Write as _;
                let _ = write!(d, "  (remote: {origin})");
            }
        }
        pkg.app_kind = AppKind::Gui;
        packages.push(pkg);
    }
    Ok(packages)
}

/// Available updates for one scope, keyed by application id.
async fn available_updates(scope: InstallScope) -> Result<HashMap<String, Option<String>>> {
    let output = capture_stdout(
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
    .context(format!("flatpak update check {}", scope_flag(scope)))?;

    // Tab-delimited: application_id\tversion
    let mut updates = HashMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(app_id) = parts.next() else { continue };
        let version = parts
            .next()
            .filter(|v| !v.is_empty())
            .map(|s| s.to_string());
        updates.insert(app_id.to_string(), version);
    }
    Ok(updates)
}

async fn available_updates_for_report(
    report: &ScanReport,
    scope: InstallScope,
) -> Result<HashMap<String, Option<String>>> {
    if report
        .packages
        .iter()
        .any(|package| package.install_scope == Some(scope))
    {
        available_updates(scope).await
    } else {
        Ok(HashMap::new())
    }
}

fn apply_scope_updates(
    report: &mut ScanReport,
    scope: InstallScope,
    result: Result<HashMap<String, Option<String>>>,
) {
    match result {
        Ok(updates) => {
            for package in report
                .packages
                .iter_mut()
                .filter(|package| package.install_scope == Some(scope))
            {
                super::update_status::checked(package);
            }
            mark_updates(&mut report.packages, scope, &updates);
        }
        Err(error) => {
            let message = format!(
                "Flatpak {} update check failed: {error:#}",
                scope_flag(scope)
            );
            for package in report
                .packages
                .iter_mut()
                .filter(|package| package.install_scope == Some(scope))
            {
                super::update_status::unavailable(package, &mut report.warning, &message);
            }
        }
    }
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
    let mut parts = size_str.split_whitespace();
    let Some(number) = parts.next() else {
        return 0;
    };
    let number: f64 = number.replace(',', ".").parse().unwrap_or(0.0);
    let unit = parts.next().unwrap_or("B").to_uppercase();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_package(scope: InstallScope) -> InstalledPackage {
        let mut package =
            InstalledPackage::new_scoped(PackageSource::Flatpak, "org.example.App", scope);
        package.update_note = Some("Checking for updates...".into());
        package
    }

    #[test]
    fn one_scope_update_failure_keeps_the_other_scope_result() {
        let mut report = ScanReport {
            packages: vec![
                pending_package(InstallScope::User),
                pending_package(InstallScope::System),
            ],
            warning: Some("Flatpak system scan failed".into()),
        };
        let mut system_updates = HashMap::new();
        system_updates.insert("org.example.App".into(), Some("2.0".into()));

        apply_scope_updates(
            &mut report,
            InstallScope::User,
            Err(anyhow::anyhow!("remote query failed")),
        );
        apply_scope_updates(&mut report, InstallScope::System, Ok(system_updates));

        assert_eq!(
            report.packages[0].update_note.as_deref(),
            Some("Flatpak --user update check failed: remote query failed")
        );
        assert_eq!(report.packages[1].update_version.as_deref(), Some("2.0"));
        assert!(report.packages[1].has_update);
        assert_eq!(report.packages[1].update_note, None);
        assert_eq!(
            report.warning.as_deref(),
            Some("Flatpak system scan failed; Flatpak --user update check failed: remote query failed")
        );
    }
}
