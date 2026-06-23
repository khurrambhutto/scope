//! Package list commands: scanning, status reporting, and search.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::package::{InstalledPackage, ScanStatus};
use crate::scanner::{scan_all, ScanAvailability};

/// Cache of the latest full scan, shared across commands.
#[derive(Default)]
pub struct ScanCache {
    inner: Arc<tokio::sync::Mutex<Option<CachedScan>>>,
}

impl ScanCache {
    /// Look up a single package by its backend key in the cached scan.
    pub async fn find(&self, key: &str) -> Option<crate::package::InstalledPackage> {
        let guard = self.inner.lock().await;
        let cached = guard.as_ref()?;
        cached.packages.iter().find(|p| p.key == key).cloned()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedScan {
    pub packages: Vec<InstalledPackage>,
    pub availability: ScanAvailability,
    pub scanned_at_ms: u64,
}

/// Run a full scan across APT, Snap, Flatpak, and AppImage and cache it.
///
/// This is the only command that touches the package managers. Results are
/// cached so `search_packages` can filter without re-scanning.
#[tauri::command]
pub async fn scan_packages(state: State<'_, ScanCache>) -> Result<CachedScan, String> {
    let (packages, availability) = scan_all().await;
    let scanned_at_ms = now_ms();
    let cached = CachedScan {
        packages: packages.clone(),
        availability: availability.clone(),
        scanned_at_ms,
    };
    let mut guard = state.inner.lock().await;
    *guard = Some(cached.clone());
    Ok(cached)
}

/// Return the most recent cached scan without rescanning.
#[tauri::command]
pub async fn get_cached_scan(state: State<'_, ScanCache>) -> Result<Option<CachedScan>, String> {
    Ok(state.inner.lock().await.clone())
}

/// Per-source availability summary (cheap probes; no real scans).
#[tauri::command]
pub async fn scan_status() -> Result<ScanStatus, String> {
    use crate::system::which;
    let appimage_dirs = crate::scanner::appimage::search_directories();
    Ok(ScanStatus {
        apt_available: which("dpkg-query") && which("apt-mark"),
        snap_available: which("snap"),
        flatpak_available: which("flatpak"),
        appimage_available: true,
        appimage_dirs,
    })
}

/// Server-side search filter applied to the cached scan. Filters by source, app
/// kind, and a case-insensitive query matched against name + display name +
/// description + package id + categories.
#[tauri::command]
pub async fn search_packages(
    state: State<'_, ScanCache>,
    query: Option<String>,
    source: Option<String>,
    app_kind: Option<String>,
) -> Result<Vec<InstalledPackage>, String> {
    let guard = state.inner.lock().await;
    let Some(cached) = guard.as_ref() else {
        return Ok(Vec::new());
    };

    let q = query.map(|s| s.trim().to_lowercase());
    let source_filter = source.and_then(|s| match s.to_lowercase().as_str() {
        "apt" => Some(crate::package::PackageSource::Apt),
        "snap" => Some(crate::package::PackageSource::Snap),
        "flatpak" => Some(crate::package::PackageSource::Flatpak),
        "appimage" => Some(crate::package::PackageSource::AppImage),
        _ => None,
    });
    let kind_filter = app_kind.and_then(|s| match s.to_lowercase().as_str() {
        "gui" => Some(crate::package::AppKind::Gui),
        "cli" => Some(crate::package::AppKind::Cli),
        "unknown" => Some(crate::package::AppKind::Unknown),
        _ => None,
    });

    let needle = |p: &InstalledPackage| -> String {
        format!(
            "{} {} {} {} {} {} {}",
            p.name,
            p.display_name.as_deref().unwrap_or(""),
            p.description.as_deref().unwrap_or(""),
            p.package_id,
            p.install_scope.map(|s| s.id()).unwrap_or(""),
            p.categories.as_deref().unwrap_or(""),
            p.version
        )
        .to_lowercase()
    };

    let results = cached
        .packages
        .iter()
        .filter(|p| match (source_filter, kind_filter) {
            (Some(s), Some(k)) => p.source == s && p.app_kind == k,
            (Some(s), None) => p.source == s,
            (None, Some(k)) => p.app_kind == k,
            (None, None) => true,
        })
        .filter(|p| match &q {
            Some(needle_q) if !needle_q.is_empty() => needle(p).contains(needle_q),
            _ => true,
        })
        .cloned()
        .collect();
    Ok(results)
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
