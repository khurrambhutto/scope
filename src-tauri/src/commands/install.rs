//! Install-kind detection for the self-updater.
//!
//! The Tauri updater serves an AppImage from `latest.json`. That can only be
//! installed over an AppImage install. System-package installs (.deb/.rpm,
//! which `dpkg`/`rpm` own) fail with the updater's `InvalidUpdaterFormat`
//! error, so the frontend asks for the install kind first and only offers
//! one-click update to AppImage installs.

use std::ffi::OsString;

/// How this copy of Scope was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InstallKind {
    /// Running from an AppImage: the `APPIMAGE` env var is set by the runtime.
    #[serde(rename = "appimage")]
    AppImage,
    /// Installed through a system package manager (.deb/.rpm/…).
    /// Self-update cannot work here; new versions come from the Releases page.
    #[serde(rename = "system")]
    SystemPackage,
}

/// Pure classifier so the decision is unit-testable without touching the
/// real process environment (env mutation is process-global and racy).
fn classify_install(appimage_var: Option<OsString>) -> InstallKind {
    if appimage_var.is_some() {
        InstallKind::AppImage
    } else {
        InstallKind::SystemPackage
    }
}

/// Report how this copy of Scope was installed.
#[tauri::command]
pub fn install_kind() -> InstallKind {
    classify_install(std::env::var_os("APPIMAGE"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appimage_var_means_appimage_install() {
        assert_eq!(
            classify_install(Some(OsString::from("/tmp/Scope_0.1.9_amd64.AppImage"))),
            InstallKind::AppImage
        );
    }

    #[test]
    fn missing_var_means_system_package() {
        assert_eq!(classify_install(None), InstallKind::SystemPackage);
    }

    #[test]
    fn serializes_to_frontend_strings() {
        assert_eq!(
            serde_json::to_value(InstallKind::AppImage).unwrap(),
            serde_json::Value::String("appimage".into())
        );
        assert_eq!(
            serde_json::to_value(InstallKind::SystemPackage).unwrap(),
            serde_json::Value::String("system".into())
        );
    }
}
