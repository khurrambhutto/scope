//! Snap scanner.
//!
//! Strategy: parse `snap list` for installed snaps and skip base/runtime snaps
//! (core, bare, snapd). On-disk size is measured from `/snap/<name>/current`
//! because `snap list` does not report sizes for user-classic snaps reliably.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use anyhow::{Context, Result};

use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::scanner::Scanner;
use crate::system::{capture_stdout, which, SCAN_TIMEOUT};

pub struct SnapScanner;

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

async fn scan() -> Result<Vec<InstalledPackage>> {
    // Whitespace columns: Name Version Rev Tracking Publisher Notes
    let output = capture_stdout("snap", &["list"], SCAN_TIMEOUT)
        .await
        .context("snap list")?;

    let mut packages = Vec::new();
    for line in output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 4 {
            continue;
        }
        let name = parts[0].to_string();
        if is_runtime(&name) {
            continue;
        }
        let version = parts[1].to_string();
        let notes = parts.get(5).copied().unwrap_or("");

        let mut pkg = InstalledPackage::new(PackageSource::Snap, name.clone());
        pkg.name = name;
        pkg.version = version;
        pkg.size_bytes = snap_size(&pkg.package_id).await;
        pkg.app_kind = if has_snap_command(&pkg.package_id) {
            AppKind::Cli
        } else {
            AppKind::Unknown
        };
        if notes.contains("classic") {
            // Keep classic snaps; command/desktop metadata still drives classification.
        }
        packages.push(pkg);
    }
    Ok(packages)
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

/// Bytes used by a snap under /snap/<name>/current. -L follows the symlink.
async fn snap_size(name: &str) -> u64 {
    let path = format!("/snap/{name}/current");
    if !Path::new(&path).exists() {
        return 0;
    }
    let args: [&str; 2] = ["-sbL", &path];
    let Ok(out) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio::process::Command::new("du").args(args).output(),
    )
    .await
    else {
        return 0;
    };
    let Ok(out) = out else { return 0 };
    let stdout = String::from_utf8_lossy(&out.stdout);
    stdout
        .split_whitespace()
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}
