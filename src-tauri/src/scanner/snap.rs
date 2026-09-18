//! Snap scanner.
//!
//! Strategy: parse `snap list` for installed snaps and skip base/runtime snaps
//! (core, bare, snapd, gtk-*, gnome-*).
//!
//! Two platform realities shape this scanner:
//!
//! 1. Every `snap` subcommand talks to `snapd` over the system bus, and a busy,
//!    restarting, or unreachable daemon makes them hang instead of fail. Each
//!    invocation is bounded by the much shorter `SNAP_TIMEOUT`, so a wedged
//!    daemon surfaces as a per-source error rather than stalling the whole scan.
//! 2. `snap list` cannot report sizes, and measuring the mounted
//!    `/snap/<name>/current` tree means walking every file in the snap (one
//!    `du` process per snap in the naive form). Sizes come from a single listing
//!    of `/var/lib/snapd/snaps/<name>_<revision>.snap` — the compressed image
//!    snapd actually stores and mounts — and only snaps with no image on disk
//!    fall back to one batched `du` pass.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::scanner::Scanner;
use crate::system::{capture_stdout, which};

pub struct SnapScanner;

/// Bound for every `snap` invocation. Snapd answers over the system bus, so a
/// request can block far longer than reading local files ever would.
const SNAP_TIMEOUT: Duration = Duration::from_secs(8);

/// Cap on the fallback `du` pass for snaps with no image file on disk.
const MOUNTED_SIZE_TIMEOUT: Duration = Duration::from_secs(5);

/// Directory holding the revision images snapd mounts at `/snap/<name>/current`.
const SNAPS_DIR: &str = "/var/lib/snapd/snaps";

impl Scanner for SnapScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Snap
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("snap") && Path::new("/var/lib/snapd").exists() })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>> {
        Box::pin(scan())
    }
}

/// One parsed row of `snap list`.
struct SnapRow {
    name: String,
    version: String,
    revision: String,
}

async fn scan() -> Result<Vec<InstalledPackage>> {
    // Whitespace columns: Name Version Rev Tracking Publisher Notes
    let output = capture_stdout("snap", &["list"], SNAP_TIMEOUT)
        .await
        .context("snap list")?;

    let mut rows = Vec::new();
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].to_string();
        if is_runtime(&name) {
            continue;
        }
        rows.push(SnapRow {
            name,
            version: parts[1].to_string(),
            revision: parts[2].to_string(),
        });
    }

    let sizes = snap_sizes(&rows).await;

    let mut packages = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut pkg = InstalledPackage::new(PackageSource::Snap, row.name.clone());
        pkg.name = row.name.clone();
        pkg.version = row.version.clone();
        pkg.size_bytes = sizes.get(&row.name).copied().unwrap_or(0);
        pkg.app_kind = if has_snap_command(&row.name) {
            AppKind::Cli
        } else {
            AppKind::Unknown
        };
        packages.push(pkg);
    }

    check_updates(&mut packages).await;
    Ok(packages)
}

/// On-disk size of every installed snap, keyed by snap name.
///
/// The fast path is a single `read_dir` of snapd's image directory: the
/// `<name>_<revision>.snap` image is what snapd stores and mounts, so its length
/// is the space the snap actually occupies. Snaps without a matching image fall
/// back to one batched `du` over their mounted trees.
async fn snap_sizes(rows: &[SnapRow]) -> HashMap<String, u64> {
    let images = read_snap_images();
    let mut sizes = HashMap::with_capacity(rows.len());
    let mut unmeasured = Vec::new();

    for row in rows {
        match images.get(&image_stem(&row.name, &row.revision)) {
            Some(bytes) => {
                sizes.insert(row.name.clone(), *bytes);
            }
            None => unmeasured.push(row.name.clone()),
        }
    }

    if !unmeasured.is_empty() {
        sizes.extend(measure_mounted(&unmeasured).await);
    }
    sizes
}

/// Snapd stores each revision as `<name>_<revision>.snap`.
fn image_stem(name: &str, revision: &str) -> String {
    format!("{name}_{revision}")
}

/// Index the `.snap` images snapd has on disk, keyed by image stem.
fn read_snap_images() -> HashMap<String, u64> {
    let mut images = HashMap::new();
    let Ok(entries) = std::fs::read_dir(SNAPS_DIR) else {
        return images;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("snap") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if let Ok(metadata) = entry.metadata() {
            images.insert(stem.to_string(), metadata.len());
        }
    }
    images
}

/// Fallback for snaps with no image file: one `du -sbL` over every mounted
/// `/snap/<name>/current`. `du` exits non-zero when it meets an unreadable file
/// (some snaps ship broken symlinks) while still printing totals for the paths it
/// could read, so stdout is parsed regardless of exit status.
async fn measure_mounted(names: &[String]) -> HashMap<String, u64> {
    let mut targets = Vec::new();
    for name in names {
        let path = Path::new("/snap").join(name).join("current");
        if path.exists() {
            targets.push(path);
        }
    }
    if targets.is_empty() {
        return HashMap::new();
    }

    let mut command = tokio::process::Command::new("du");
    command.arg("-sbL").args(&targets);
    let Ok(Ok(out)) = tokio::time::timeout(MOUNTED_SIZE_TIMEOUT, command.output()).await else {
        return HashMap::new();
    };

    parse_du_totals(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `du` totals into a snap-name to bytes map.
///
/// `du` prints `<bytes>\t<path>` for each requested path. Only lines carrying
/// both a parsable size and a `/snap/<name>/current` path are kept, which makes
/// this tolerant of partial output and of `du` diagnostics that leak onto stdout.
/// Snap names cannot contain spaces, so whitespace splitting is unambiguous here.
fn parse_du_totals(stdout: &str) -> HashMap<String, u64> {
    let mut sizes = HashMap::new();
    for line in stdout.lines() {
        let mut parts = line.split_whitespace();
        let (Some(bytes), Some(path)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(bytes) = bytes.parse::<u64>() else {
            continue;
        };
        if let Some(name) = path
            .strip_prefix("/snap/")
            .and_then(|rest| rest.strip_suffix("/current"))
        {
            sizes.insert(name.to_string(), bytes);
        }
    }
    sizes
}

/// Run `snap refresh --list` and mark snaps that have available updates.
/// Note: the list shows current version, not the target, so we set
/// `has_update = true` without a specific target version for v1.
async fn check_updates(packages: &mut Vec<InstalledPackage>) {
    let output = match capture_stdout("snap", &["refresh", "--list"], SNAP_TIMEOUT).await {
        Ok(o) => o,
        Err(_) => return,
    };

    for line in output.lines().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].to_string();
        if let Some(pkg) = packages.iter_mut().find(|p| p.package_id == name) {
            pkg.has_update = true;
            // The Version column in refresh --list is the current version, not
            // the target. We mark the update as available without a target version.
        }
    }
}

fn is_runtime(name: &str) -> bool {
    name == "snapd"
        || name == "bare"
        || name.starts_with("core")
        || name.starts_with("gtk-")
        || name.starts_with("gnome-")
        || name.ends_with("-gtk3")
}

fn has_snap_command(name: &str) -> bool {
    Path::new(&format!("/snap/bin/{name}")).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_stem_matches_snapd_layout() {
        assert_eq!(image_stem("code", "263"), "code_263");
        assert_eq!(image_stem("desktop-security-center", "12"), "desktop-security-center_12");
    }

    /// `du` can exit non-zero (unreadable files inside a snap) while still
    /// printing totals for the paths it could read, so the parser must accept
    /// clean totals mixed with diagnostics.
    #[test]
    fn du_totals_keep_snap_paths_and_skip_noise() {
        let stdout = "1234437173\t/snap/vlc/current\n4096\t/snap/code/current\n\
                      du: cannot access '/snap/vlc/current/share/x.svg': No such file or directory\n";
        let sizes = parse_du_totals(stdout);
        assert_eq!(sizes.get("vlc"), Some(&1_234_437_173));
        assert_eq!(sizes.get("code"), Some(&4096));
        assert_eq!(sizes.len(), 2);
    }

    #[test]
    fn du_totals_ignore_unrelated_paths_and_bad_sizes() {
        let sizes = parse_du_totals("not-a-size\t/snap/foo/current\n12\t/elsewhere/foo\n\n");
        assert!(sizes.is_empty());
    }

    /// The scrubber must keep real apps while hiding runtimes and base snaps.
    #[test]
    fn runtime_scrub_covers_snap_internals() {
        for runtime in ["snapd", "bare", "core22", "gtk-common-themes", "gnome-46-2404"] {
            assert!(is_runtime(runtime), "{runtime} should be treated as a runtime");
        }
        for app in ["firefox", "code", "vlc", "onlyoffice-desktopeditors"] {
            assert!(!is_runtime(app), "{app} should be treated as an app");
        }
    }
}
