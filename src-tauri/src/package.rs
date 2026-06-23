//! Shared package/app models and source enums.
//!
//! These types are the single source of truth for what the backend reports to
//! the frontend. They are serialized directly into Tauri command results, so the
//! `src/shared/types/package.ts` TypeScript models must stay in sync.

use serde::{Deserialize, Serialize};

/// The package manager that installed an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    Apt,
    Snap,
    Flatpak,
    AppImage,
}

impl PackageSource {
    /// Short machine identifier used for keys and protocol routing.
    pub fn id(self) -> &'static str {
        match self {
            PackageSource::Apt => "apt",
            PackageSource::Snap => "snap",
            PackageSource::Flatpak => "flatpak",
            PackageSource::AppImage => "appimage",
        }
    }

    /// Human label shown in the UI.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            PackageSource::Apt => "APT",
            PackageSource::Snap => "Snap",
            PackageSource::Flatpak => "Flatpak",
            PackageSource::AppImage => "AppImage",
        }
    }
}

/// Coarse classification used for filtering/feedback only. Best-effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppKind {
    Gui,
    Cli,
    #[default]
    Unknown,
}

/// A unified view of one installed package/app regardless of its source.
///
/// GUI metadata (`display_name`, `icon`, `categories`, `terminal`) is an
/// enrichment layer filled in by the desktop-entry merge step. Non-GUI packages
/// keep `AppKind::Cli`/`Unknown` and still appear in the unified list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Backend-side stable key: `<source>:<package id>`.
    pub key: String,
    /// Source package manager.
    pub source: PackageSource,
    /// Package id as the package manager knows it (dpkg name, snap name,
    /// flatpak application id, or AppImage absolute path).
    pub package_id: String,
    /// Canonical name shown when no desktop display name is available.
    pub name: String,
    /// Optional human-friendly display name from a `.desktop` entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Optional one-line description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Installed version (empty string when unknown).
    pub version: String,
    /// On-disk installed size in bytes (0 when unknown).
    pub size_bytes: u64,
    /// Coarse app kind.
    pub app_kind: AppKind,
    /// Optional icon URL for the webview, e.g. `scope-icon://localhost/<path>`.
    /// Produced by resolving the `.desktop` entry's `Icon=` value through the
    /// XDG icon-theme lookup in `crate::icons`. `None` for non-GUI packages or
    /// when no icon file could be resolved (frontend falls back to initials).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Comma-joined category list from a `.desktop` entry, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<String>,
    /// Whether the item launches in a terminal (from `.desktop` entry).
    pub terminal: bool,
    /// True when an update is known to be available.
    pub has_update: bool,
}

impl InstalledPackage {
    pub fn new(source: PackageSource, package_id: impl Into<String>) -> Self {
        let package_id = package_id.into();
        let key = format!("{}:{}", source.id(), package_id);
        Self {
            key,
            source,
            package_id,
            name: String::new(),
            display_name: None,
            description: None,
            version: String::new(),
            size_bytes: 0,
            app_kind: AppKind::Unknown,
            icon: None,
            categories: None,
            terminal: false,
            has_update: false,
        }
    }
}

/// Status reported by `scan_status` so the UI can show per-source health.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatus {
    pub apt_available: bool,
    pub snap_available: bool,
    pub flatpak_available: bool,
    pub appimage_available: bool,
    pub appimage_dirs: Vec<String>,
}