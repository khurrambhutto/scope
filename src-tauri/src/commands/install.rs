//! Install-kind detection for the self-updater.
//!
//! The Tauri updater picks the artifact to download from the bundle type the
//! bundler patched into this binary: it looks up `{os}-{arch}-{bundle_type}`
//! in `latest.json` (e.g. `linux-x86_64-deb`) and installs with `dpkg -i`,
//! `rpm -U`, or an in-place AppImage rewrite. Exposing the same value here
//! lets the frontend offer one-click updates exactly where the updater can
//! install them, and fall back to the Releases page everywhere else (source
//! builds, AUR, Flatpak).

use std::ffi::OsString;

use tauri::utils::config::BundleType;
use tauri::utils::platform::bundle_type;

/// How this copy of Scope was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallKind {
    /// Running from an AppImage: the updater rewrites the file in place.
    AppImage,
    /// Installed as a Debian package: the updater runs `pkexec dpkg -i`.
    Deb,
    /// Installed as an RPM package: the updater runs `pkexec rpm -U`.
    Rpm,
    /// Unknown install (dev build, AUR, Flatpak, source install).
    /// Self-update cannot work here; new versions come from the Releases page.
    Unknown,
}

/// Pure classifier so the decision is unit-testable without touching the
/// real process environment (env mutation is process-global and racy).
fn classify(bundle: Option<BundleType>, appimage_var: Option<OsString>) -> InstallKind {
    match bundle {
        Some(BundleType::Deb) => InstallKind::Deb,
        Some(BundleType::Rpm) => InstallKind::Rpm,
        Some(BundleType::AppImage) => InstallKind::AppImage,
        // Bundles without a patched marker (dev builds, older bundlers):
        // the AppImage runtime always exports APPIMAGE, so trust it as a
        // fallback and fail closed to Unknown otherwise.
        _ if appimage_var.is_some() => InstallKind::AppImage,
        _ => InstallKind::Unknown,
    }
}

/// Report how this copy of Scope was installed.
#[tauri::command]
pub fn install_kind() -> InstallKind {
    classify(bundle_type(), std::env::var_os("APPIMAGE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn appimage_var() -> Option<OsString> {
        Some(OsString::from("/tmp/Scope_0.1.91_amd64.AppImage"))
    }

    #[test]
    fn deb_bundle_means_deb_install() {
        assert_eq!(classify(Some(BundleType::Deb), None), InstallKind::Deb);
    }

    #[test]
    fn rpm_bundle_means_rpm_install() {
        assert_eq!(classify(Some(BundleType::Rpm), None), InstallKind::Rpm);
    }

    #[test]
    fn appimage_bundle_means_appimage_install() {
        assert_eq!(
            classify(Some(BundleType::AppImage), None),
            InstallKind::AppImage
        );
    }

    #[test]
    fn missing_bundle_falls_back_to_appimage_env_var() {
        assert_eq!(classify(None, appimage_var()), InstallKind::AppImage);
    }

    #[test]
    fn missing_bundle_without_env_var_is_unknown() {
        assert_eq!(classify(None, None), InstallKind::Unknown);
    }

    #[test]
    fn foreign_bundle_types_are_unknown() {
        assert_eq!(classify(Some(BundleType::Msi), None), InstallKind::Unknown);
        assert_eq!(classify(Some(BundleType::Nsis), None), InstallKind::Unknown);
        assert_eq!(classify(Some(BundleType::App), None), InstallKind::Unknown);
        assert_eq!(classify(Some(BundleType::Dmg), None), InstallKind::Unknown);
    }

    #[test]
    fn bundle_wins_over_env_var() {
        // A deb install must stay "deb" even if APPIMAGE is set in the
        // environment — the updater resolves the artifact from the bundle.
        assert_eq!(classify(Some(BundleType::Deb), appimage_var()), InstallKind::Deb);
    }

    #[test]
    fn serializes_to_frontend_strings() {
        for (kind, expected) in [
            (InstallKind::AppImage, "appimage"),
            (InstallKind::Deb, "deb"),
            (InstallKind::Rpm, "rpm"),
            (InstallKind::Unknown, "unknown"),
        ] {
            assert_eq!(
                serde_json::to_value(kind).unwrap(),
                serde_json::Value::String(expected.into())
            );
        }
    }
}
