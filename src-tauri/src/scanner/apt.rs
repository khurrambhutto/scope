//! APT/dpkg scanner.
//!
//! Strategy: list *manually* installed packages via `apt-mark showmanual`, then
//! fetch rich metadata for exactly those names with one `dpkg-query` call.
//! Reporting only manual installs keeps the unified list focused on apps the
//! user actually chose, instead of thousands of pulled-in dependencies.

use std::collections::HashSet;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::scanner::Scanner;
use crate::system::{capture_stdout, which, SCAN_TIMEOUT};

pub struct AptScanner;

impl Scanner for AptScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Apt
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("dpkg-query") && which("apt-mark") })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>> {
        Box::pin(scan())
    }
}

// Field separator unlikely to appear in package metadata.
const SEP: &str = "\x1f";

async fn scan() -> Result<Vec<InstalledPackage>> {
    let manual = capture_stdout("apt-mark", &["showmanual"], SCAN_TIMEOUT)
        .await
        .context("read manually-installed packages")?;
    let manual: HashSet<String> = manual
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if manual.is_empty() {
        return Ok(Vec::new());
    }

    // One dpkg-query over all manual names. Installed-Size is in KiB.
    // ${binary:Summary} truncates the description to one line; perfect for the UI.
    let format = format!(
        "${{Package}}{SEP}${{Version}}{SEP}${{Installed-Size}}{SEP}${{binary:Summary}}{SEP}\\n"
    );
    let mut args: Vec<String> = vec!["-W".into(), format!("-f={format}")];
    args.extend(manual.into_iter());

    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = capture_stdout("dpkg-query", &argv, Duration::from_secs(20))
        .await
        .context("query dpkg metadata for manual packages")?;

    let mut packages = Vec::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split(SEP).collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[0].to_string();
        let version = parts[1].to_string();
        let kib: u64 = parts[2].parse().unwrap_or(0);
        let summary = parts.get(3).copied().unwrap_or("").to_string();

        let mut pkg = InstalledPackage::new(PackageSource::Apt, name.clone());
        pkg.name = name;
        pkg.version = version;
        pkg.size_bytes = kib * 1024;
        if !summary.is_empty() {
            pkg.description = Some(summary);
        }
        pkg.app_kind = classify(&pkg.name);
        packages.push(pkg);
    }
    check_updates(&mut packages).await;
    Ok(packages)
}

/// Best-effort GUI/CLI classification using filesystem presence, without
/// spawning a per-package subprocess (the old impl ran dpkg-query per package
/// and was slow).
fn classify(name: &str) -> AppKind {
    for dir in ["/usr/share/applications", "/usr/local/share/applications"] {
        for variant in [name.to_lowercase(), name.replace('-', "_")] {
            let path = format!("{dir}/{variant}.desktop");
            if std::path::Path::new(&path).exists() {
                return AppKind::Gui;
            }
        }
    }
    if has_binary(name) {
        AppKind::Cli
    } else {
        AppKind::Unknown
    }
}

/// Run `apt list --upgradable` and mark packages that have available updates.
async fn check_updates(packages: &mut Vec<InstalledPackage>) {
    let output = match capture_stdout("apt", &["list", "--upgradable"], Duration::from_secs(30)).await
    {
        Ok(o) => o,
        Err(_) => return,
    };

    // Line format: "package/suite candidate_version arch [upgradable from: old_version]"
    // Or:          "package/arch candidate_version arch [held]"
    let re = match regex::Regex::new(
        r"^(\S+)/(\S+)\s+(\S+)\s+\S+\s+\[upgradable from: (\S+)\]",
    ) {
        Ok(r) => r,
        Err(_) => return,
    };

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line == "Listing..." || line.starts_with("WARNING:") {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            let name = caps[1].to_string();
            let candidate = caps[3].to_string();
            if let Some(pkg) = packages.iter_mut().find(|p| p.package_id == name) {
                pkg.has_update = true;
                pkg.update_version = Some(candidate);
            }
        }
    }
}

fn has_binary(name: &str) -> bool {
    for variant in [name.to_string(), name.replace('-', "_")] {
        for bin_dir in ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"] {
            if std::path::Path::new(&format!("{bin_dir}/{variant}")).is_file() {
                return true;
            }
        }
    }
    false
}
