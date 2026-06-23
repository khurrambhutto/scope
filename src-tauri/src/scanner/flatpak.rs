//! Flatpak scanner.
//!
//! Strategy: `flatpak list --app` with explicit, tab-delimited columns. Only
//! applications are reported (runtimes are intentionally excluded). Flatpaks are
//! GUI-first; their `.desktop` ids equal the application id, which the desktop
//! enrichment step matches exactly.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use anyhow::{Context, Result};

use crate::package::{AppKind, InstalledPackage, PackageSource};
use crate::scanner::Scanner;
use crate::system::{capture_stdout, which, SCAN_TIMEOUT};

pub struct FlatpakScanner;

impl Scanner for FlatpakScanner {
    fn source(&self) -> PackageSource {
        PackageSource::Flatpak
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { which("flatpak") && Path::new("/var/lib/flatpak").exists() || Path::new(&home_flatpak()).exists() })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>> {
        Box::pin(scan())
    }
}

fn home_flatpak() -> String {
    std::env::var("HOME")
        .map(|h| format!("{h}/.local/share/flatpak"))
        .unwrap_or_default()
}

async fn scan() -> Result<Vec<InstalledPackage>> {
    // Tab-delimited column output (flatpak columns default to this separator
    // when redirected / non-tty).
    let columns = "application,name,version,origin,size,description";
    let output = capture_stdout("flatpak", &["list", "--app", &format!("--columns={columns}")], SCAN_TIMEOUT)
        .await
        .context("flatpak list")?;

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

        let mut pkg = InstalledPackage::new(PackageSource::Flatpak, app_id);
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