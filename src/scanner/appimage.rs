//! AppImage scanner - scans for AppImage files in common locations

use crate::package::{AppType, Package, PackageSource};
use crate::scanner::PackageScanner;
use anyhow::{Context, Result};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use tokio::fs;
use walkdir::WalkDir;

pub struct AppImageScanner;

impl AppImageScanner {
    pub fn new() -> Self {
        Self
    }

    /// Get common directories where AppImages might be stored
    fn get_search_directories() -> Vec<PathBuf> {
        let mut dirs = vec![
            PathBuf::from("/opt"),
            PathBuf::from("/usr/local/bin"),
        ];

        // Add user-specific directories
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(&home).join("Applications"));
            dirs.push(PathBuf::from(&home).join("apps"));
            dirs.push(PathBuf::from(&home).join(".local/bin"));
            dirs.push(PathBuf::from(&home).join("AppImages"));
            dirs.push(PathBuf::from(&home).join("Downloads")); // Common location
        }

        dirs
    }

    /// Check if a file is an AppImage by checking magic bytes or extension
    async fn is_appimage(path: &Path) -> bool {
        // Check extension first
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "appimage" {
                return true;
            }
        }

        // Check magic bytes: AppImages start with ELF header and contain "AI" magic
        if let Ok(mut file) = fs::File::open(path).await {
            use tokio::io::AsyncReadExt;
            let mut buffer = [0u8; 16];
            if file.read_exact(&mut buffer).await.is_ok() {
                // ELF magic number
                if &buffer[0..4] == b"\x7fELF" {
                    // Could be an AppImage - check if executable
                    if let Ok(metadata) = fs::metadata(path).await {
                        use std::os::unix::fs::PermissionsExt;
                        return metadata.permissions().mode() & 0o111 != 0;
                    }
                }
            }
        }

        false
    }

    /// Extract version from AppImage filename if possible
    fn extract_version(filename: &str) -> String {
        // Common patterns: App-1.2.3.AppImage, App_v1.2.3_x86_64.AppImage
        let name = filename.trim_end_matches(".AppImage").trim_end_matches(".appimage");

        // Try to find version pattern
        let patterns = [
            r"-(\d+\.\d+\.?\d*)[-_]?",
            r"_v?(\d+\.\d+\.?\d*)[-_]?",
            r"[_-](\d+\.\d+\.?\d*)$",
        ];

        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(name) {
                    if let Some(version) = caps.get(1) {
                        return version.as_str().to_string();
                    }
                }
            }
        }

        "unknown".to_string()
    }

    /// Extract app name from filename
    fn extract_name(filename: &str) -> String {
        let name = filename
            .trim_end_matches(".AppImage")
            .trim_end_matches(".appimage");

        // Remove version and architecture suffixes
        let patterns = [
            r"[-_]v?\d+\.\d+.*$",
            r"[-_]x86_64.*$",
            r"[-_]amd64.*$",
            r"[-_]linux.*$",
        ];

        let mut clean_name = name.to_string();
        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                clean_name = re.replace(&clean_name, "").to_string();
            }
        }

        clean_name
    }
}

impl PackageScanner for AppImageScanner {
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        Box::pin(async { true }) // Always available - just scans filesystem
    }

    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async {
            let mut packages = Vec::new();
            let search_dirs = Self::get_search_directories();

            for dir in search_dirs {
                if !dir.exists() {
                    continue;
                }

                // Use walkdir for recursive search, but limit depth
                for entry in WalkDir::new(&dir)
                    .max_depth(3)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    let path = entry.path();
                    if path.is_file() && Self::is_appimage(path).await {
                        let filename = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        let name = Self::extract_name(&filename);
                        let version = Self::extract_version(&filename);

                        let mut package = Package::new(name, PackageSource::AppImage);
                        package.version = version;
                        package.install_path = Some(path.to_string_lossy().to_string());

                        // Get file size
                        if let Ok(metadata) = fs::metadata(path).await {
                            package.size_bytes = metadata.len();
                        }

                        package.description = format!("AppImage at {}", path.display());
                        package.app_type = AppType::GUI; // AppImages are typically GUI apps

                        packages.push(package);
                    }
                }
            }

            Ok(packages)
        })
    }

    fn get_updates(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + Send + '_>> {
        Box::pin(async {
            // AppImages don't have a central update mechanism
            // Some support AppImageUpdate, but we'll skip that for now
            Ok(Vec::new())
        })
    }

    fn uninstall(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let path = package.install_path.clone();
        Box::pin(async move {
            if let Some(path) = path {
                fs::remove_file(&path)
                    .await
                    .context("Failed to delete AppImage file")?;

                // Also try to remove associated .desktop file if it exists
                if let Ok(home) = std::env::var("HOME") {
                    let desktop_dir = PathBuf::from(&home).join(".local/share/applications");
                    if let Ok(mut entries) = fs::read_dir(&desktop_dir).await {
                        use tokio::io::AsyncReadExt;
                        while let Ok(Some(entry)) = entries.next_entry().await {
                            let entry_path = entry.path();
                            if entry_path.extension().map(|e| e == "desktop").unwrap_or(false) {
                                if let Ok(mut file) = fs::File::open(&entry_path).await {
                                    let mut contents = String::new();
                                    if file.read_to_string(&mut contents).await.is_ok() {
                                        if contents.contains(&path) {
                                            let _ = fs::remove_file(&entry_path).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Ok(())
            } else {
                anyhow::bail!("No path specified for AppImage")
            }
        })
    }

    fn update(&self, _package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async { anyhow::bail!("AppImage updates are not supported") })
    }
}
