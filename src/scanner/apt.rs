//! APT/dpkg scanner for Debian-based systems

use crate::package::{AppType, Package, PackageSource};
use crate::scanner::PackageScanner;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use tokio::process::Command;

pub struct AptScanner;

impl AptScanner {
    pub fn new() -> Self {
        Self
    }

    /// Detect if a package is a GUI application
    async fn detect_app_type(package_name: &str) -> AppType {
        // Check for .desktop file
        let desktop_paths = [
            format!("/usr/share/applications/{}.desktop", package_name),
            format!(
                "/usr/share/applications/{}.desktop",
                package_name.to_lowercase()
            ),
        ];

        for path in &desktop_paths {
            if Path::new(path).exists() {
                return AppType::GUI;
            }
        }

        // Check dependencies for GUI libraries
        if let Ok(output) = Command::new("dpkg-query")
            .args(["-W", "-f=${Depends}", package_name])
            .output()
            .await
        {
            let deps = String::from_utf8_lossy(&output.stdout).to_lowercase();
            if deps.contains("libgtk")
                || deps.contains("libqt")
                || deps.contains("libx11")
                || deps.contains("wayland")
                || deps.contains("libgl")
            {
                return AppType::GUI;
            }
        }

        // Check if it's a known CLI tool pattern
        let cli_patterns = [
            "lib", "dev", "doc", "data", "common", "core", "base", "utils",
        ];
        for pattern in cli_patterns {
            if package_name.starts_with(pattern) || package_name.ends_with(pattern) {
                return AppType::CLI;
            }
        }

        AppType::Unknown
    }
}

impl PackageScanner for AptScanner {
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async {
            Command::new("dpkg-query")
                .arg("--version")
                .output()
                .await
                .is_ok()
        })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async {
            // Get list of installed packages with details
            // Format: package\tversion\tinstalled-size(KB)\tdescription
            let output = Command::new("dpkg-query")
                .args([
                    "-W",
                    "-f=${Package}\t${Version}\t${Installed-Size}\t${binary:Summary}\n",
                ])
                .output()
                .await
                .context("Failed to run dpkg-query")?;

            if !output.status.success() {
                anyhow::bail!(
                    "dpkg-query failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut packages = Vec::new();

            // Get list of manually installed packages to filter out dependencies
            let manual_output = Command::new("apt-mark")
                .arg("showmanual")
                .output()
                .await
                .ok();

            let manual_packages: HashSet<String> = manual_output
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 4 {
                    let name = parts[0].to_string();

                    // Skip packages that are just dependencies (not manually installed)
                    // But include them if manual_packages is empty (fallback)
                    if !manual_packages.is_empty() && !manual_packages.contains(&name) {
                        continue;
                    }

                    let version = parts[1].to_string();
                    let size_kb: u64 = parts[2].parse().unwrap_or(0);
                    let description = parts[3..].join(" ");

                    let mut package = Package::new(name.clone(), PackageSource::Apt);
                    package.version = version;
                    package.size_bytes = size_kb * 1024; // Convert KB to bytes
                    package.description = description;
                    package.app_type = Self::detect_app_type(&name).await;

                    packages.push(package);
                }
            }

            Ok(packages)
        })
    }

    fn get_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + Send + '_>>
    {
        Box::pin(async {
            // First update package lists
            let _ = Command::new("apt")
                .args(["update", "-qq"])
                .output()
                .await;

            // Get upgradable packages
            let output = Command::new("apt")
                .args(["list", "--upgradable"])
                .output()
                .await
                .context("Failed to check for updates")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut updates = Vec::new();

            for line in stdout.lines().skip(1) {
                // Skip "Listing..." header
                // Format: package/source version arch [upgradable from: old_version]
                if let Some(name) = line.split('/').next() {
                    if let Some(version_part) = line.split_whitespace().nth(1) {
                        updates.push((name.to_string(), version_part.to_string()));
                    }
                }
            }

            Ok(updates)
        })
    }

    fn uninstall(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let name = package.name.clone();
        Box::pin(async move {
            let status = Command::new("pkexec")
                .args(["apt", "remove", "-y", &name])
                .status()
                .await
                .context("Failed to run uninstall command")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Uninstall failed with exit code: {:?}", status.code())
            }
        })
    }

    fn update(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let name = package.name.clone();
        Box::pin(async move {
            let status = Command::new("pkexec")
                .args(["apt", "install", "-y", "--only-upgrade", &name])
                .status()
                .await
                .context("Failed to run update command")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Update failed with exit code: {:?}", status.code())
            }
        })
    }
}
