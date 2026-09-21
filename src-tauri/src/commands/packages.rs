//! Package list commands: scanning, status reporting, and search.

use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::package::InstalledPackage;
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
/// cached in memory for the other commands and persisted so the next launch can
/// paint the previous list while a fresh scan runs.
#[tauri::command]
pub async fn scan_packages(
    app: AppHandle,
    state: State<'_, ScanCache>,
) -> Result<CachedScan, String> {
    let (packages, availability) = scan_all().await;
    let cached = CachedScan {
        packages,
        availability,
        scanned_at_ms: now_ms(),
    };
    *state.inner.lock().await = Some(cached.clone());

    // Serialize once, then persist off the async runtime. The response does not
    // wait on disk I/O.
    if let Ok(json) = serde_json::to_vec(&cached) {
        let app = app.clone();
        let _ = tokio::task::spawn_blocking(move || write_disk_cache(&app, json)).await;
    }
    Ok(cached)
}

/// Return the most recent cached scan without rescanning.
///
/// Falls back to the copy persisted by the previous session, so a cold start
/// shows the previous list immediately instead of an empty screen.
#[tauri::command]
pub async fn get_cached_scan(
    app: AppHandle,
    state: State<'_, ScanCache>,
) -> Result<Option<CachedScan>, String> {
    if let Some(cached) = state.inner.lock().await.clone() {
        return Ok(Some(cached));
    }

    let Some(cached) = tokio::task::spawn_blocking(move || read_disk_cache(&app))
        .await
        .unwrap_or(None)
    else {
        return Ok(None);
    };
    *state.inner.lock().await = Some(cached.clone());
    Ok(Some(cached))
}

/// Name of the persisted scan inside the app data directory.
const SCAN_CACHE_FILE: &str = "scan-cache.json";

fn cache_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(SCAN_CACHE_FILE))
}

fn read_disk_cache(app: &AppHandle) -> Option<CachedScan> {
    let bytes = std::fs::read(cache_file(app)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_disk_cache(app: &AppHandle, json: Vec<u8>) {
    let Some(path) = cache_file(app) else { return };
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    let _ = std::fs::write(path, json);
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
