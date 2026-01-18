//! Snap package scanner

use crate::package::{AppType, Package, PackageSource};
use crate::scanner::PackageScanner;
use anyhow::{Context, Result};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use tokio::process::Command;

pub struct SnapScanner;

impl SnapScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get the size of a snap package from its directory
    async fn get_snap_size(name: &str) -> u64 {
        let snap_path = format!("/snap/{}/current", name);
        if let Ok(output) = Command::new("du").args(["-sb", &snap_path]).output().await {
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if let Some(size_str) = stdout.split_whitespace().next() {
                    return size_str.parse().unwrap_or(0);
                }
            }
        }
        0
    }

    /// Detect if a snap is a GUI application
    fn detect_app_type(name: &str) -> AppType {
        // Check for desktop file
        let desktop_paths = [format!(
            "/var/lib/snapd/desktop/applications/{}_*.desktop",
            name
        )];

        for pattern in &desktop_paths {
            if let Ok(entries) = glob::glob(pattern) {
                if entries.count() > 0 {
                    return AppType::GUI;
                }
            }
        }

        // Common GUI snaps
        let gui_snaps = [
            "firefox",
            "chromium",
            "vlc",
            "spotify",
            "slack",
            "discord",
            "code",
            "sublime-text",
            "gimp",
            "inkscape",
            "blender",
        ];

        if gui_snaps.iter().any(|s| name.contains(s)) {
            return AppType::GUI;
        }

        AppType::Unknown
    }
}

impl PackageScanner for SnapScanner {
    fn source_type(&self) -> PackageSource {
        PackageSource::Snap
    }

    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { Path::new("/usr/bin/snap").exists() })
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async {
            let output = Command::new("snap")
                .args(["list"])
                .output()
                .await
                .context("Failed to run snap list")?;

            if !output.status.success() {
                anyhow::bail!(
                    "snap list failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut packages = Vec::new();

            // Skip header line
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let name = parts[0].to_string();
                    let version = parts[1].to_string();

                    // Skip core snaps
                    if name == "snapd" || name.starts_with("core") || name.starts_with("bare") {
                        continue;
                    }

                    let mut package = Package::new(name.clone(), PackageSource::Snap);
                    package.version = version;
                    package.size_bytes = Self::get_snap_size(&name).await;
                    package.app_type = Self::detect_app_type(&name);

                    // Get description from snap info
                    if let Ok(info_output) =
                        Command::new("snap").args(["info", &name]).output().await
                    {
                        let info = String::from_utf8_lossy(&info_output.stdout);
                        for line in info.lines() {
                            if line.starts_with("summary:") {
                                package.description =
                                    line.trim_start_matches("summary:").trim().to_string();
                                break;
                            }
                        }
                    }

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
            let output = Command::new("snap")
                .args(["refresh", "--list"])
                .output()
                .await
                .context("Failed to check snap updates")?;

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut updates = Vec::new();

            // Skip header line
            for line in stdout.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    updates.push((parts[0].to_string(), parts[1].to_string()));
                }
            }

            Ok(updates)
        })
    }

    fn uninstall(
        &self,
        package: &Package,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let name = package.name.clone();
        Box::pin(async move {
            let status = Command::new("pkexec")
                .args(["snap", "remove", &name])
                .status()
                .await
                .context("Failed to run snap remove")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Snap uninstall failed")
            }
        })
    }

    fn update(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let name = package.name.clone();
        Box::pin(async move {
            let status = Command::new("pkexec")
                .args(["snap", "refresh", &name])
                .status()
                .await
                .context("Failed to run snap refresh")?;

            if status.success() {
                Ok(())
            } else {
                anyhow::bail!("Snap update failed")
            }
        })
    }
}
