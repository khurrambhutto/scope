//! Policy for packages shown in Scope's main list.
//!
//! The list is for user-facing applications and command-line tools that Scope
//! supports uninstalling. This is deliberately separate from the safety
//! deny-list: safety decides whether an operation may run, while this policy
//! decides whether an item belongs in the app-oriented list at all.

use crate::domain::package::{AppKind, InstalledPackage};
use crate::domain::safety;

/// True when `pkg` is a user-facing app/tool that Scope can offer to uninstall.
pub fn is_listable(pkg: &InstalledPackage) -> bool {
    matches!(pkg.app_kind, AppKind::Gui | AppKind::Cli)
        && !safety::check_package(pkg.source, &pkg.package_id).protected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::PackageSource;

    fn package(source: PackageSource, id: &str, kind: AppKind) -> InstalledPackage {
        let mut pkg = InstalledPackage::new(source, id);
        pkg.app_kind = kind;
        pkg
    }

    #[test]
    fn keeps_unprotected_gui_apps_and_cli_tools() {
        assert!(is_listable(&package(
            PackageSource::Apt,
            "firefox",
            AppKind::Gui
        )));
        assert!(is_listable(&package(
            PackageSource::Apt,
            "wl-clipboard",
            AppKind::Cli
        )));
    }

    #[test]
    fn hides_unclassified_packages_and_protected_packages() {
        assert!(!is_listable(&package(
            PackageSource::Apt,
            "steam-libs-amd64",
            AppKind::Unknown
        )));
        assert!(!is_listable(&package(
            PackageSource::Apt,
            "systemd",
            AppKind::Cli
        )));
    }

    #[test]
    fn hides_appimages_until_scope_can_uninstall_them() {
        assert!(!is_listable(&package(
            PackageSource::AppImage,
            "/opt/tool.AppImage",
            AppKind::Gui
        )));
    }
}
