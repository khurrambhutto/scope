//! Policy for packages shown in Scope's main list.
//!
//! The list is for user-facing applications and command-line tools. This is
//! deliberately separate from the safety deny-list: safety decides whether an
//! operation may run, while this policy decides whether an item belongs in the
//! app-oriented list at all.

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};
use crate::domain::safety;

/// True when `pkg` is a user-facing app/tool that belongs in Scope's main list.
pub fn is_listable(pkg: &InstalledPackage) -> bool {
    if !matches!(pkg.app_kind, AppKind::Gui | AppKind::Cli) {
        return false;
    }
    // Keep discovered AppImages visible so users can identify them, but their
    // row action and detail panel explain that removal is not supported yet.
    pkg.source == PackageSource::AppImage || can_uninstall(pkg)
}

/// True when the backend safety policy permits an uninstall plan.
pub fn can_uninstall(pkg: &InstalledPackage) -> bool {
    !safety::check_package(pkg.source, &pkg.package_id).protected
}

/// Why the backend currently blocks uninstalling this package, if applicable.
pub fn uninstall_block_reason(pkg: &InstalledPackage) -> Option<String> {
    safety::check_package(pkg.source, &pkg.package_id).reason
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
    fn lists_appimages_but_disables_uninstall_until_supported() {
        let app = package(PackageSource::AppImage, "/opt/tool.AppImage", AppKind::Gui);
        assert!(is_listable(&app));
        assert!(!can_uninstall(&app));
    }
}
