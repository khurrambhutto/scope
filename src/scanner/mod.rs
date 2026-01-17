//! Scanner module - scans for installed packages from various sources

pub mod apt;
pub mod snap;
pub mod flatpak;
pub mod appimage;

use crate::package::Package;
use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

/// Trait for package scanners
pub trait PackageScanner: Send + Sync {
    /// Check if this package manager is available on the system
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
    
    /// Scan for installed packages
    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>>;
    
    /// Get packages that have updates available
    fn get_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + Send + '_>>;
    
    /// Uninstall a package
    fn uninstall(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
    
    /// Update a package
    fn update(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// Scan all available package managers in parallel
pub async fn scan_all() -> Result<Vec<Package>> {
    use tokio::task::JoinSet;
    
    let scanners: Vec<Box<dyn PackageScanner>> = vec![
        Box::new(apt::AptScanner::new()),
        Box::new(snap::SnapScanner::new()),
        Box::new(flatpak::FlatpakScanner::new()),
        Box::new(appimage::AppImageScanner::new()),
    ];
    
    let mut join_set = JoinSet::new();
    
    for scanner in scanners {
        join_set.spawn(async move {
            if scanner.is_available().await {
                scanner.scan().await.unwrap_or_default()
            } else {
                Vec::new()
            }
        });
    }
    
    let mut all_packages = Vec::new();
    
    while let Some(result) = join_set.join_next().await {
        if let Ok(packages) = result {
            all_packages.extend(packages);
        }
    }
    
    Ok(all_packages)
}

/// Check for updates across all package managers
pub async fn check_all_updates(packages: &mut [Package]) -> Result<()> {
    use tokio::task::JoinSet;
    use std::collections::HashMap;
    
    let scanners: Vec<Box<dyn PackageScanner>> = vec![
        Box::new(apt::AptScanner::new()),
        Box::new(snap::SnapScanner::new()),
        Box::new(flatpak::FlatpakScanner::new()),
    ];
    
    let mut join_set = JoinSet::new();
    
    for scanner in scanners {
        join_set.spawn(async move {
            if scanner.is_available().await {
                scanner.get_updates().await.unwrap_or_default()
            } else {
                Vec::new()
            }
        });
    }
    
    let mut updates_map: HashMap<String, String> = HashMap::new();
    
    while let Some(result) = join_set.join_next().await {
        if let Ok(updates) = result {
            for (name, version) in updates {
                updates_map.insert(name, version);
            }
        }
    }
    
    // Mark packages with updates
    for package in packages.iter_mut() {
        if let Some(new_version) = updates_map.get(&package.name) {
            package.has_update = Some(true);
            package.update_version = Some(new_version.clone());
        } else {
            package.has_update = Some(false);
        }
    }
    
    Ok(())
}

/// Get the appropriate scanner for a package source
pub fn get_scanner(source: crate::package::PackageSource) -> Box<dyn PackageScanner> {
    use crate::package::PackageSource;
    match source {
        PackageSource::Apt | PackageSource::DebFile => Box::new(apt::AptScanner::new()),
        PackageSource::Snap => Box::new(snap::SnapScanner::new()),
        PackageSource::Flatpak => Box::new(flatpak::FlatpakScanner::new()),
        PackageSource::AppImage => Box::new(appimage::AppImageScanner::new()),
    }
}
