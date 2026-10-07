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
    // Keep discovered AppImages and unmanaged desktop apps visible, but their
    // row action and detail panel explain why Scope cannot remove them.
    matches!(pkg.source, PackageSource::AppImage | PackageSource::Desktop) || can_uninstall(pkg)
}

/// Apply the optional all-packages override to the default app/tool list policy.
pub fn is_visible(pkg: &InstalledPackage, show_all: bool) -> bool {
    show_all || is_listable(pkg)
}

/// True when the backend safety policy permits an uninstall plan.
pub fn can_uninstall(pkg: &InstalledPackage) -> bool {
    matches!(pkg.app_kind, AppKind::Gui | AppKind::Cli)
        && !safety::check_package(pkg.source, &pkg.package_id).protected
}

/// Why the backend currently blocks uninstalling this package, if applicable.
pub fn uninstall_block_reason(pkg: &InstalledPackage) -> Option<String> {
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    protection.reason.or_else(|| {
        matches!(pkg.app_kind, AppKind::Unknown)
            .then(|| "Scope cannot verify this package as a user-facing app or CLI tool.".into())
    })
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

    #[test]
    fn lists_user_desktop_apps_but_disables_uninstall() {
        let app = package(
            PackageSource::Desktop,
            "/home/user/.local/share/applications/zed.desktop",
            AppKind::Gui,
        );
        assert!(is_listable(&app));
        assert!(!can_uninstall(&app));
    }

    #[test]
    fn show_all_includes_unknown_and_protected_packages_without_bypassing_safety() {
        let lib = package(PackageSource::Apt, "libexample1", AppKind::Unknown);
        let system = package(PackageSource::Apt, "systemd", AppKind::Gui);

        assert!(!is_visible(&lib, false));
        assert!(is_visible(&lib, true));
        assert!(!can_uninstall(&lib));
        assert!(!can_uninstall(&system));
        assert!(is_visible(&system, true));
    }
}
