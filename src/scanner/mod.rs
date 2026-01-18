//! Scanner module - scans for installed packages from various sources

pub mod appimage;
pub mod apt;
pub mod flatpak;
pub mod snap;

use crate::package::{Package, PackageSource};
use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

/// Trait for package scanners
pub trait PackageScanner: Send + Sync {
    /// Check if this package manager is available on the system
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    /// Scan for installed packages
    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>>;

    /// Get packages that have updates available
    fn get_updates(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<(String, String)>>> + Send + '_>>;

    /// Uninstall a package
    fn uninstall(&self, package: &Package)
        -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Update a package
    fn update(&self, package: &Package) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get the source type for this scanner
    fn source_type(&self) -> PackageSource;
}

/// Message sent from scanners during progressive loading
#[derive(Debug)]
pub enum ScanMessage {
    /// A batch of packages was found
    Packages(Vec<Package>),
    /// Scanner started for a source
    Started(PackageSource),
    /// Scanner completed for a source
    Completed(PackageSource),
    /// All scanning done
    Done,
}

/// Scan all available package managers and send results through channel
pub fn scan_all_streaming() -> mpsc::Receiver<ScanMessage> {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move {
        use tokio::task::JoinSet;

        let scanners: Vec<Box<dyn PackageScanner>> = vec![
            Box::new(apt::AptScanner::new()),
            Box::new(snap::SnapScanner::new()),
            Box::new(flatpak::FlatpakScanner::new()),
            Box::new(appimage::AppImageScanner::new()),
        ];

        let mut join_set = JoinSet::new();

        for scanner in scanners {
            let tx = tx.clone();
            join_set.spawn(async move {
                let source = scanner.source_type();
                let _ = tx.send(ScanMessage::Started(source)).await;

                if scanner.is_available().await {
                    if let Ok(packages) = scanner.scan().await {
                        if !packages.is_empty() {
                            let _ = tx.send(ScanMessage::Packages(packages)).await;
                        }
                    }
                }

                let _ = tx.send(ScanMessage::Completed(source)).await;
            });
        }

        // Wait for all scanners to complete
        while join_set.join_next().await.is_some() {}

        let _ = tx.send(ScanMessage::Done).await;
    });

    rx
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
    use std::collections::HashMap;
    use tokio::task::JoinSet;

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
