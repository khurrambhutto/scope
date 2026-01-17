//! Flatpak scanner

use crate::package::{AppType, Package, PackageSource};
use crate::scanner::PackageScanner;
use anyhow::{Context, Result};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use tokio::process::Command;

pub struct FlatpakScanner;

impl FlatpakScanner {
    pub fn new() -> Self {
        Self
    }
}

impl PackageScanner for FlatpakScanner {
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { Path::new("/usr/bin/flatpak").exists() })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async {
            // Get list of installed flatpaks with details
            let output = Command::new("flatpak")
                .args([
                    "list",
                    "--app", // Only applications, not runtimes
                    "--columns=name,application,version,size,description",
                ])
                .output()
                .await
                .context("Failed to run flatpak list")?;

            if !output.status.success() {
                anyhow::bail!(
                    "flatpak list failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut packages = Vec::new();

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 4 {
                    let display_name = parts[0].to_string();
                    let app_id = parts[1].to_string();
                    let version = parts.get(2).unwrap_or(&"").to_string();
                    let size_str = parts.get(3).unwrap_or(&"0");
                    let description = parts.get(4).unwrap_or(&"").to_string();

                    // Parse size (can be like "1.2 GB", "500 MB", etc.)
                    let size_bytes = parse_size(size_str);

                    let mut package = Package::new(display_name, PackageSource::Flatpak);
                    package.version = version;
                    package.size_bytes = size_bytes;
                    package.description = description;
                    package.install_path = Some(app_id);
                    // Flatpaks are almost always GUI apps
                    package.app_type = AppType::GUI;

                    packages.push(package);
                }
            }

            Ok(packages)
        })
    }

    fn get_updates(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + Send + '_>> {
        Box::pin(async {
            let output = Command::new("flatpak")
                .args(["remote-ls", "--updates", "--columns=name,version"])
                .output()
                .await
                .context("Failed to check flatpak updates")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut updates = Vec::new();

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    updates.push((parts[0].to_string(), parts[1].to_string()));
                }
            }

            Ok(updates)
        })
    }

    fn uninstall(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let app_id = package.install_path.clone().unwrap_or(package.name.clone());
        Box::pin(async move {
            let status = Command::new("flatpak")
                .args(["uninstall", "-y", &app_id])
                .status()
                .await
                .context("Failed to run flatpak uninstall")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Flatpak uninstall failed")
            }
        })
    }

    fn update(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let app_id = package.install_path.clone().unwrap_or(package.name.clone());
        Box::pin(async move {
            let status = Command::new("flatpak")
                .args(["update", "-y", &app_id])
                .status()
                .await
                .context("Failed to run flatpak update")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Flatpak update failed")
            }
        })
    }
}

/// Parse human-readable size string to bytes
fn parse_size(size_str: &str) -> u64 {
    let size_str = size_str.trim();
    let parts: Vec<&str> = size_str.split_whitespace().collect();

    if parts.is_empty() {
        return 0;
    }

    let number: f64 = parts[0].replace(',', ".").parse().unwrap_or(0.0);
    let unit = parts.get(1).unwrap_or(&"B").to_uppercase();

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
