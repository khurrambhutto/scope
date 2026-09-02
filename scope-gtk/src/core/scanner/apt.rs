//! APT/dpkg scanner.
//!
//! Strategy: list *manually* installed packages via `apt-mark showmanual`, then
//! fetch rich metadata for exactly those names with one `dpkg-query` call.
//! Reporting only manual installs keeps the unified list focused on apps the
//! user actually chose, instead of thousands of pulled-in dependencies.

use std::collections::{HashMap, HashSet};
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
    // `dpkg-query` (metadata) and `apt list --upgradable` (update marks) are
    // independent — run them concurrently instead of sequentially. The update
    // fetch is best-effort: a failure leaves packages unmarked, as before.
    let (dpkg_result, upgradable_result) = tokio::join!(
        capture_stdout("dpkg-query", &argv, Duration::from_secs(20)),
        capture_stdout("apt", &["list", "--upgradable"], Duration::from_secs(30)),
    );
    let output = dpkg_result.context("query dpkg metadata for manual packages")?;

    let mut packages = Vec::new();
    // dpkg-query emits one row per architecture for multiarch packages (e.g.
    // libc6:amd64 and libc6:i386). Keep the first row per package name so
    // every package key stays unique.
    let mut seen: HashSet<String> = HashSet::new();
    // Read the classifier directories once; per-package classification then
    // runs from memory instead of ~14 filesystem stats per package.
    let class_index = ClassIndex::load();
    for line in output.lines() {
        let parts: Vec<&str> = line.split(SEP).collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[0].to_string();
        if !seen.insert(name.clone()) {
            continue;
        }
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
        pkg.app_kind = class_index.classify(&pkg.name);
        packages.push(pkg);
    }
    if let Ok(upgradable) = upgradable_result {
        mark_updates(&mut packages, &upgradable);
    }
    Ok(packages)
}

/// Directories probed for GUI/CLI classification. Kept as constants so the
/// one-time snapshot and the in-memory lookup cannot drift apart.
const DESKTOP_DIRS: [&str; 2] = [
    "/usr/share/applications",
    "/usr/local/share/applications",
];
const BIN_DIRS: [&str; 5] = ["/usr/bin", "/bin", "/usr/sbin", "/sbin", "/usr/local/bin"];

/// One-time snapshot of the classifier directories, read once per scan.
///
/// The old per-package `classify()`/`has_binary()` issued ~14 filesystem stats
/// per package (2 desktop dirs × 2 name variants + 5 bin dirs × 2 variants).
/// This struct pays one `read_dir` per directory plus one stat per entry, then
/// answers every package from memory with identical lookup semantics.
struct ClassIndex {
    desktops: HashSet<String>,
    bins: HashSet<String>,
}

impl ClassIndex {
    fn load() -> Self {
        let mut desktops = HashSet::new();
        for dir in DESKTOP_DIRS {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let full = format!("{dir}/{}", entry.file_name().to_string_lossy());
                    // Mirror the old `Path::exists()` check (true for files,
                    // dirs, and live symlinks; false for missing/broken links).
                    if std::path::Path::new(&full).exists() {
                        desktops.insert(full);
                    }
                }
            }
        }
        let mut bins = HashSet::new();
        for dir in BIN_DIRS {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let full = format!("{dir}/{}", entry.file_name().to_string_lossy());
                    // Mirror the old `Path::is_file()` check (regular files and
                    // symlinks resolving to files only).
                    if std::path::Path::new(&full).is_file() {
                        bins.insert(full);
                    }
                }
            }
        }
        Self { desktops, bins }
    }

    /// Best-effort GUI/CLI classification from the snapshot, without spawning
    /// a per-package subprocess or touching the filesystem.
    fn classify(&self, name: &str) -> AppKind {
        for variant in [name.to_lowercase(), name.replace('-', "_")] {
            for dir in DESKTOP_DIRS {
                if self.desktops.contains(&format!("{dir}/{variant}.desktop")) {
                    return AppKind::Gui;
                }
            }
        }
        if self.has_binary(name) {
            AppKind::Cli
        } else {
            AppKind::Unknown
        }
    }

    fn has_binary(&self, name: &str) -> bool {
        for variant in [name.to_string(), name.replace('-', "_")] {
            for bin_dir in BIN_DIRS {
                if self.bins.contains(&format!("{bin_dir}/{variant}")) {
                    return true;
                }
            }
        }
        false
    }
}

/// Mark packages that have available updates by parsing one
/// `apt list --upgradable` output. Pure function over already-fetched output
/// (the fetch runs concurrently with `dpkg-query` in `scan`); unparseable
/// output simply marks nothing.
fn mark_updates(packages: &mut Vec<InstalledPackage>, output: &str) {
    // Line format: "package/suite candidate_version arch [upgradable from: old_version]"
    // Or:          "package/arch candidate_version arch [held]"
    let re = match regex::Regex::new(
        r"^(\S+)/(\S+)\s+(\S+)\s+\S+\s+\[upgradable from: (\S+)\]",
    ) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Index package ids once so per-line matching is O(1) instead of a
    // linear scan over all packages for every upgradable line. Keys are
    // owned so the map doesn't borrow `packages` (which we mutate below).
    let by_id: HashMap<String, usize> = packages
        .iter()
        .enumerate()
        .map(|(i, p)| (p.package_id.clone(), i))
        .collect();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line == "Listing..." || line.starts_with("WARNING:") {
            continue;
        }
        if let Some(caps) = re.captures(line) {
            let candidate = caps[3].to_string();
            if let Some(&i) = by_id.get(caps.get(1).map_or("", |m| m.as_str())) {
                packages[i].has_update = true;
                packages[i].update_version = Some(candidate);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(id: &str) -> InstalledPackage {
        let mut p = InstalledPackage::new(PackageSource::Apt, id);
        p.name = id.to_string();
        p
    }

    #[test]
    fn mark_updates_flags_only_listed_packages() {
        let mut packages = vec![pkg("firefox"), pkg("curl")];
        let output = "Listing...\n\
            firefox/noble-updates 1:128.0 amd64 [upgradable from: 1:127.0]\n\
            other-pkg/noble-updates 2.0 amd64 [upgradable from: 1.0]\n";
        mark_updates(&mut packages, output);
        assert!(packages[0].has_update);
        assert_eq!(packages[0].update_version.as_deref(), Some("1:128.0"));
        assert!(!packages[1].has_update);
        assert!(packages[1].update_version.is_none());
    }

    #[test]
    fn mark_updates_ignores_garbage() {
        let mut packages = vec![pkg("curl")];
        mark_updates(&mut packages, "Listing...\nWARNING: apt does not have a stable CLI interface.\n\n");
        assert!(!packages[0].has_update);
    }
}
