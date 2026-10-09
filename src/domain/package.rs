//! Shared package/app models and source enums.
//!
//! These types are the single source of truth for what the backend reports to
//! the frontend. They are serialized directly into Tauri command results, so the
//! `src/shared/types/package.ts` TypeScript models must stay in sync.

use serde::{Deserialize, Serialize};

/// The package manager or user-local source that owns an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PackageSource {
    Apt,
    Snap,
    Flatpak,
    AppImage,
    /// A locally discovered Steam application. Steam content is read-only in Scope.
    Steam,
    /// A user-visible desktop launcher not owned by a supported package manager.
    Desktop,
}

impl PackageSource {
    /// Short machine identifier used for keys and protocol routing.
    pub fn id(self) -> &'static str {
        match self {
            PackageSource::Apt => "apt",
            PackageSource::Snap => "snap",
            PackageSource::Flatpak => "flatpak",
            PackageSource::AppImage => "appimage",
            PackageSource::Steam => "steam",
            PackageSource::Desktop => "desktop",
        }
    }
}

/// Coarse classification used for filtering/feedback only. Best-effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppKind {
    Gui,
    Cli,
    Game,
    #[default]
    Unknown,
}

/// The locally recorded state of a Steam update, if Steam supplied valid
/// manifest flags during the scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteamUpdateStatus {
    /// Steam did not record usable state flags for this installed application.
    Unknown,
    /// Steam recorded state flags but did not mark an update as required.
    NoUpdateRecorded,
    /// Steam recorded an update requirement that is neither paused nor active.
    Pending,
    /// Steam recorded an update requirement that is paused.
    Paused,
    /// Steam recorded an update requirement with active transfer or processing.
    InProgress,
}

/// Where a package is installed when the package manager has multiple scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallScope {
    User,
    System,
}

impl InstallScope {
    pub fn id(self) -> &'static str {
        match self {
            InstallScope::User => "user",
            InstallScope::System => "system",
        }
    }
}

/// A unified view of one installed package/app regardless of its source.
///
/// Desktop metadata (`display_name`, `icon`, `categories`, `terminal`) is an
/// enrichment layer filled in by the desktop-entry merge step. Packages with
/// no reliable GUI, CLI, or game classification remain `Unknown` and are
/// omitted from the app-facing main list by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPackage {
    /// Backend-side stable key: `<source>:<package id>`.
    pub key: String,
    /// Package manager or local desktop-launcher source.
    pub source: PackageSource,
    /// Package id as the package manager knows it (dpkg name, snap name,
    /// Flatpak application id, AppImage absolute path, Steam AppID, or
    /// desktop-entry path.
    pub package_id: String,
    /// Install scope for package managers that can install the same id in more
    /// than one place, such as Flatpak user/system installations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_scope: Option<InstallScope>,
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
    /// XDG icon-theme lookup in `crate::domain::icons`. `None` for non-GUI packages or
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
    /// The version string of the available update, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_version: Option<String>,
    /// Locally recorded Steam update state, when this package came from Steam.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_update_status: Option<SteamUpdateStatus>,
}

impl InstalledPackage {
    pub fn new(source: PackageSource, package_id: impl Into<String>) -> Self {
        let package_id = package_id.into();
        let key = format!("{}:{}", source.id(), package_id);
        Self {
            key,
            source,
            package_id,
            install_scope: None,
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
            update_version: None,
            steam_update_status: None,
        }
    }

    pub fn new_scoped(
        source: PackageSource,
        package_id: impl Into<String>,
        scope: InstallScope,
    ) -> Self {
        let package_id = package_id.into();
        let key = format!("{}:{}:{}", source.id(), scope.id(), package_id);
        Self {
            key,
            source,
            package_id,
            install_scope: Some(scope),
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
            update_version: None,
            steam_update_status: None,
        }
    }

    /// Open the Steam library details page for this package when it has a
    /// canonical, positive 32-bit Steam AppID.
    pub fn steam_library_url(&self) -> Option<String> {
        if self.source != PackageSource::Steam {
            return None;
        }
        let bytes = self.package_id.as_bytes();
        if bytes.is_empty()
            || bytes.len() > 10
            || (bytes.len() > 1 && bytes[0] == b'0')
            || !bytes.iter().all(u8::is_ascii_digit)
        {
            return None;
        }
        let app_id = self.package_id.parse::<u32>().ok().filter(|id| *id != 0)?;
        Some(format!("steam://nav/games/details/{app_id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_prefixes_the_key_with_the_source() {
        assert_eq!(
            InstalledPackage::new(PackageSource::Apt, "gimp").key,
            "apt:gimp"
        );
    }

    #[test]
    fn new_records_no_install_scope() {
        assert_eq!(
            InstalledPackage::new(PackageSource::Snap, "code").install_scope,
            None
        );
    }

    #[test]
    fn constructors_leave_steam_update_status_unset() {
        assert_eq!(
            InstalledPackage::new(PackageSource::Steam, "42").steam_update_status,
            None
        );
        assert_eq!(
            InstalledPackage::new_scoped(
                PackageSource::Flatpak,
                "org.gimp.GIMP",
                InstallScope::User
            )
            .steam_update_status,
            None
        );
    }

    #[test]
    fn new_scoped_embeds_the_scope_in_the_key() {
        assert_eq!(
            InstalledPackage::new_scoped(
                PackageSource::Flatpak,
                "org.gimp.GIMP",
                InstallScope::User
            )
            .key,
            "flatpak:user:org.gimp.GIMP"
        );
    }

    #[test]
    fn new_scoped_distinguishes_user_and_system_installs() {
        let user = InstalledPackage::new_scoped(
            PackageSource::Flatpak,
            "org.gimp.GIMP",
            InstallScope::User,
        );
        let system = InstalledPackage::new_scoped(
            PackageSource::Flatpak,
            "org.gimp.GIMP",
            InstallScope::System,
        );
        assert_ne!(user.key, system.key);
    }

    #[test]
    fn new_scoped_records_the_install_scope() {
        assert_eq!(
            InstalledPackage::new_scoped(
                PackageSource::Flatpak,
                "org.gimp.GIMP",
                InstallScope::System
            )
            .install_scope,
            Some(InstallScope::System)
        );
    }

    #[test]
    fn cached_package_without_steam_update_status_deserializes() {
        let old_cache = serde_json::to_string(&InstalledPackage::new(PackageSource::Steam, "42"))
            .unwrap();
        assert!(!old_cache.contains("steam_update_status"));

        let package: InstalledPackage = serde_json::from_str(&old_cache).unwrap();
        assert_eq!(package.steam_update_status, None);
    }

    #[test]
    fn steam_update_status_serializes_with_snake_case_names() {
        assert_eq!(
            serde_json::to_string(&SteamUpdateStatus::NoUpdateRecorded).unwrap(),
            "\"no_update_recorded\""
        );
    }

    #[test]
    fn steam_library_url_requires_a_canonical_positive_app_id() {
        let valid = InstalledPackage::new(PackageSource::Steam, "1062520");
        assert_eq!(
            valid.steam_library_url().as_deref(),
            Some("steam://nav/games/details/1062520")
        );

        for invalid in ["0", "001", "+42", "42 ", "4294967296", "not-an-app-id"] {
            let pkg = InstalledPackage::new(PackageSource::Steam, invalid);
            assert_eq!(pkg.steam_library_url(), None, "{invalid}");
        }
        assert_eq!(
            InstalledPackage::new(PackageSource::Apt, "42").steam_library_url(),
            None
        );
    }
}
