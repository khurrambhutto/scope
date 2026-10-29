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
    if pkg.source == PackageSource::Steam {
        return matches!(pkg.app_kind, AppKind::Game | AppKind::Gui);
    }
    if pkg.source == PackageSource::JetBrainsToolbox {
        return pkg.app_kind == AppKind::Gui;
    }
    if !matches!(pkg.app_kind, AppKind::Gui | AppKind::Cli) {
        return false;
    }
    // Keep discovered AppImages and unmanaged desktop apps visible, but their
    // row action and detail panel explain why Scope cannot remove them.
    matches!(pkg.source, PackageSource::AppImage | PackageSource::Desktop)
        || (pkg.source == PackageSource::Apt && pkg.apt_automatic && pkg.app_kind == AppKind::Gui)
        || can_uninstall(pkg)
}

/// Apply the optional all-packages override to the default app/tool list policy.
pub fn is_visible(pkg: &InstalledPackage, show_all: bool) -> bool {
    show_all || is_listable(pkg)
}

/// True when the backend safety policy permits an uninstall plan.
pub fn can_uninstall(pkg: &InstalledPackage) -> bool {
    matches!(pkg.app_kind, AppKind::Gui | AppKind::Cli)
        && (pkg.source != PackageSource::AppImage || pkg.appimage_installation.is_some())
        && (!crate::domain::tool_managers::is_tool_source(pkg.source)
            || pkg
                .tool_installation
                .as_ref()
                .is_some_and(|i| i.protection_reason.is_none()))
        && !safety::check_package(pkg.source, &pkg.package_id).protected
}

/// A short caution shown only when confirming removal of an automatic APT app.
pub fn uninstall_warning(pkg: &InstalledPackage) -> Option<&'static str> {
    (pkg.source == PackageSource::Apt && pkg.apt_automatic)
        .then_some("Other apps or desktop features may depend on this app.")
}

/// Why the backend currently blocks uninstalling this package, if applicable.
pub fn uninstall_block_reason(pkg: &InstalledPackage) -> Option<String> {
    if crate::domain::tool_managers::is_tool_source(pkg.source) && pkg.tool_installation.is_none() {
        return Some("Tool installation metadata missing. Rescan before continuing.".into());
    }
    if let Some(reason) = pkg
        .tool_installation
        .as_ref()
        .and_then(|i| i.protection_reason.clone())
    {
        return Some(reason);
    }
    let protection = safety::check_package(pkg.source, &pkg.package_id);
    if protection.reason.is_none()
        && pkg.source == PackageSource::AppImage
        && pkg.appimage_installation.is_none()
    {
        return Some(
            "This AppImage has no launcher from Scope or Gear Lever. Open it with Scope to install it."
                .into(),
        );
    }
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
    fn uninstall_caution_is_specific_to_automatic_apt_packages() {
        let mut app = package(PackageSource::Apt, "gnome-calculator", AppKind::Gui);
        assert!(uninstall_warning(&app).is_none());
        app.apt_automatic = true;
        assert_eq!(
            uninstall_warning(&app),
            Some("Other apps or desktop features may depend on this app.")
        );
        app.source = PackageSource::Snap;
        assert!(uninstall_warning(&app).is_none());
    }

    #[test]
    fn automatically_installed_protected_gui_is_visible_but_not_removable() {
        let mut app = package(PackageSource::Apt, "systemd", AppKind::Gui);
        app.apt_automatic = true;
        assert!(is_listable(&app));
        assert!(!can_uninstall(&app));
        app.app_kind = AppKind::Unknown;
        assert!(!is_listable(&app));
    }

    #[test]
    fn lists_appimages_but_disables_uninstall_outside_the_managed_folder() {
        let app = package(PackageSource::AppImage, "/opt/tool.AppImage", AppKind::Gui);
        assert!(is_listable(&app));
        assert!(!can_uninstall(&app));
    }

    #[test]
    fn managed_folder_appimages_need_a_verified_integration() {
        let path = format!("{}/AppImages/tool.appimage", std::env::var("HOME").unwrap());
        let mut app = package(PackageSource::AppImage, &path, AppKind::Gui);
        assert!(!can_uninstall(&app));
        assert!(uninstall_block_reason(&app).unwrap().contains("Gear Lever"));
        app.appimage_installation = Some(Box::new(crate::domain::appimage::AppImageInstallation {
            path: path.into(),
            launcher: "/l.desktop".into(),
            launchers: vec![],
            icons: vec![],
            size: 0,
            modified_ns: 0,
            update_info: None,
            update: None,
        }));
        assert!(can_uninstall(&app));
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
    fn lists_steam_games_and_applications_but_keeps_unknown_items_out() {
        let game = package(PackageSource::Steam, "570", AppKind::Game);
        let application = package(PackageSource::Steam, "250820", AppKind::Gui);
        let unknown = package(PackageSource::Steam, "12345", AppKind::Unknown);

        assert!(is_listable(&game));
        assert!(is_listable(&application));
        assert!(!is_listable(&unknown));
        assert!(is_visible(&unknown, true));
        assert!(!can_uninstall(&game));
    }

    #[test]
    fn lists_toolbox_ides_but_never_enables_uninstall() {
        let ide = package(
            PackageSource::JetBrainsToolbox,
            "/home/user/.local/share/JetBrains/Toolbox/apps/pycharm",
            AppKind::Gui,
        );

        assert!(is_listable(&ide));
        assert!(!can_uninstall(&ide));
        assert!(uninstall_block_reason(&ide)
            .unwrap()
            .contains("JetBrains Toolbox manages IDE updates and uninstall"));
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
