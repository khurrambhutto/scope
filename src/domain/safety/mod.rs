//! Safety: protected packages, protected paths, and backend validation.
//!
//! No destructive operation may run before passing through here. The deny-list
//! is enforced in the backend, independent of any frontend state, so a crafted
//! `invoke` call can never remove a system-critical package or a protected path.

use crate::domain::package::PackageSource;

/// Reason a package/path is protected, surfaced to the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Protection {
    pub protected: bool,
    pub reason: Option<String>,
}

impl Protection {
    /// The package/path is safe to act on.
    pub fn allowed() -> Self {
        Self {
            protected: false,
            reason: None,
        }
    }

    /// The package/path must not be acted on. `reason` is shown to the user.
    pub fn denied(reason: impl Into<String>) -> Self {
        Self {
            protected: true,
            reason: Some(reason.into()),
        }
    }
}

/// Decide whether a package may be removed by Scope.
///
/// Essential/system-critical packages are always blocked. The check is purely
/// backend-side and uses the canonical package id plus the source.
pub fn check_package(source: PackageSource, package_id: &str) -> Protection {
    match source {
        PackageSource::Uv
        | PackageSource::Pipx
        | PackageSource::Cargo
        | PackageSource::Npm
        | PackageSource::Bun => {
            if crate::domain::tool_managers::validate_package_name(source, package_id).is_err() {
                Protection::denied("Invalid tool name")
            } else if ["uv", "pipx", "cargo", "rustup", "scope", "scope-gpui"].contains(&package_id)
                || (matches!(source, PackageSource::Npm | PackageSource::Bun)
                    && ["npm", "bun", "node", "corepack", "npx"].contains(&package_id))
            {
                Protection::denied("Package managers and Scope cannot be removed or replaced through this operation.")
            } else {
                Protection::allowed()
            }
        }
        PackageSource::Apt => check_apt(package_id),
        PackageSource::Snap => check_snap(package_id),
        PackageSource::Flatpak => check_flatpak(package_id),
        PackageSource::AppImage => check_appimage(package_id),
        PackageSource::Desktop => check_desktop(package_id),
        PackageSource::Steam => check_steam(package_id),
        PackageSource::JetBrainsToolbox => check_jetbrains_toolbox(package_id),
    }
}

/// Package protection plus target metadata that operations require: an
/// AppImage in the managed folder is still protected until a scan has verified
/// its Scope/Gear Lever integration.
pub fn check_target(target: &crate::domain::operations::OperationTarget) -> Protection {
    use crate::domain::operations::OperationTarget;
    match target {
        OperationTarget::AppImageInstall { .. } | OperationTarget::DebInstall { .. } => {
            Protection::denied("Install plans cannot be uninstalled or updated.")
        }
        OperationTarget::AppImage {
            installation: None, ..
        } => {
            let protection = check_package(target.source(), target.id());
            if protection.protected {
                protection
            } else {
                Protection::denied(
                    "This AppImage has no launcher from Scope or Gear Lever. Open it with Scope to install it.",
                )
            }
        }
        _ => check_package(target.source(), target.id()),
    }
}

fn check_apt(name: &str) -> Protection {
    let lower = name.to_lowercase();
    let stripped = lower
        .strip_suffix(":amd64")
        .or_else(|| lower.strip_suffix(":i386"))
        .unwrap_or(&lower);
    let n = stripped;

    // Core system packages whose removal would break the OS or the GUI session.
    const CRITICAL: &[&str] = &[
        "ubuntu-desktop",
        "ubuntu-standard",
        "ubuntu-minimal",
        "ubuntu-release-upgrader-core",
        "systemd",
        "systemd-sysv",
        "systemd-timesyncd",
        "systemd-resolved",
        "systemd-logind",
        "polkitd",
        "policykit-1",
        "polkit",
        "pkexec",
        "apt",
        "apt-utils",
        "dpkg",
        "base-files",
        "base-passwd",
        "bash",
        "coreutils",
        "linux-image-generic",
        "linux-headers-generic",
        "linux-generic",
        "gnome-shell",
        "gnome-session",
        "gnome-control-center",
        "gdm3",
        "gdm",
        "xorg",
        "xserver-xorg-core",
        "xserver-xorg",
        "wayland",
        "network-manager",
        "network-manager-gnome",
        "netplan.io",
        "iproute2",
        "sudo",
        "login",
        "passwd",
        "shadow",
        "adduser",
        "snapd",
        "flatpak",
        "libc6",
        "libssl3",
        "libgtk-3-0",
        "libgtk-4-1",
    ];

    if CRITICAL.contains(&n) {
        return Protection::denied(format!(
            "'{n}' is a system-critical package and cannot be removed through Scope."
        ));
    }
    // Kernel images and headers (any version).
    if n.starts_with("linux-image-")
        || n.starts_with("linux-headers-")
        || n.starts_with("linux-modules-")
    {
        return Protection::denied(format!("'{n}' is a kernel package and is protected."));
    }
    // Libraries: removing a shared lib through a GUI is risky and rarely what
    // the user means by "uninstall an app". Block lib* packages.
    if n.starts_with("lib") && n.len() > 3 {
        let after_lib = &n[3..];
        // Allow things like "libreoffice-*" (apps, not libs) — they start with "lib" but aren't libs.
        if !n.starts_with("libreoffice") && !n.starts_with("libre2") && is_library_name(after_lib) {
            return Protection::denied(format!(
                "'{n}' is a shared library; Scope removes applications, not libraries."
            ));
        }
    }
    Protection::allowed()
}

/// A library soname looks like `ssl3`, `gtk-3-0`, `c6` — short with digits/dashes.
fn is_library_name(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_digit()) || s.contains('-')
}

fn check_snap(name: &str) -> Protection {
    let n = name.to_lowercase();
    // Runtime/base snaps that other snaps depend on.
    const RUNTIME: &[&str] = &[
        "snapd", "bare", "core", "core18", "core20", "core22", "core24",
    ];
    if RUNTIME.iter().any(|c| n == *c)
        || n.starts_with("gtk-")
        || n.starts_with("gnome-")
        || n.ends_with("-gtk3")
    {
        return Protection::denied(format!("'{n}' is a Snap runtime/base and is protected."));
    }
    Protection::allowed()
}

fn check_flatpak(_app_id: &str) -> Protection {
    // Flatpaks are user applications; runtimes are excluded from the scan
    // already. We allow removal but the preview still confirms the id exists.
    Protection::allowed()
}

/// Scope manages only AppImages kept directly in `~/AppImages`, the folder
/// Scope and Gear Lever install into. Everything else stays discovery-only, and
/// the running Scope AppImage is never removed or replaced from the list.
/// Integration ownership (launcher `TryExec`, file owner) is verified
/// separately by `appimage::managed::verify` at preview and apply.
fn check_appimage(path: &str) -> Protection {
    let path = std::path::Path::new(path);
    if std::env::var_os("APPIMAGE").is_some_and(|own| std::path::Path::new(&own) == path) {
        return Protection::denied(
            "This is the running copy of Scope. Update it from Settings instead.",
        );
    }
    match crate::domain::appimage::Roots::current() {
        Ok(roots) if roots.is_managed_location(path) => Protection::allowed(),
        _ => Protection::denied(
            "Scope manages AppImages installed in ~/AppImages by Scope or Gear Lever. Open this file with Scope to install it there.",
        ),
    }
}

fn check_desktop(_desktop_file: &str) -> Protection {
    Protection::denied("This app is not managed by a supported package manager.")
}

fn check_steam(_app_id: &str) -> Protection {
    Protection::denied("Steam items are read-only in Scope. Manage updates in Steam.")
}

fn check_jetbrains_toolbox(_installation_root: &str) -> Protection {
    Protection::denied("JetBrains Toolbox manages IDE updates and uninstall.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_package_blocks_critical_apt_packages() {
        for name in ["ubuntu-desktop", "systemd", "apt", "pkexec"] {
            assert!(
                check_package(PackageSource::Apt, name).protected,
                "expected '{name}' to be protected"
            );
        }
    }

    #[test]
    fn check_package_blocks_kernel_images() {
        assert!(check_package(PackageSource::Apt, "linux-image-6.8.0-45-generic").protected);
    }

    #[test]
    fn check_package_blocks_shared_libraries() {
        for name in ["libssl3", "libgtk-3-0"] {
            assert!(
                check_package(PackageSource::Apt, name).protected,
                "expected '{name}' to be protected"
            );
        }
    }

    #[test]
    fn check_package_allows_libreoffice_despite_the_lib_heuristic() {
        assert!(!check_package(PackageSource::Apt, "libreoffice-writer").protected);
    }

    #[test]
    fn check_package_allows_regular_apps_across_sources() {
        assert!(!check_package(PackageSource::Apt, "firefox").protected);
        assert!(!check_package(PackageSource::Apt, "vlc").protected);
        assert!(!check_package(PackageSource::Snap, "firefox").protected);
        assert!(!check_package(PackageSource::Flatpak, "org.mozilla.firefox").protected);
    }

    #[test]
    fn check_package_blocks_snap_runtimes() {
        for name in ["core20", "gtk-common-themes", "snapd"] {
            assert!(
                check_package(PackageSource::Snap, name).protected,
                "expected '{name}' to be protected"
            );
        }
    }

    /// Only direct children of `~/AppImages` (the Scope/Gear Lever folder)
    /// may be managed; system folders, Downloads, and nested paths stay
    /// protected at preview and revalidation.
    #[test]
    fn check_package_allows_only_the_managed_appimage_folder() {
        let home = std::env::var("HOME").unwrap();
        for path in [
            "/opt/Firefox-130.0.AppImage".to_string(),
            "/usr/local/bin/Firefox-130.0.AppImage".to_string(),
            format!("{home}/Downloads/app-1.2.AppImage"),
            format!("{home}/AppImages/nested/app.appimage"),
            format!("{home}/AppImages/.scope-update-x/download.appimage"),
            format!("{home}/AppImages/../app.appimage"),
            "AppImages/app.appimage".to_string(),
        ] {
            assert!(
                check_package(PackageSource::AppImage, &path).protected,
                "expected '{path}' to be protected"
            );
        }
        assert!(
            !check_package(
                PackageSource::AppImage,
                &format!("{home}/AppImages/app.appimage")
            )
            .protected
        );
    }

    #[test]
    fn check_package_explains_appimage_protection() {
        let protection = check_package(PackageSource::AppImage, "/opt/Firefox-130.0.AppImage");
        assert!(protection.reason.unwrap().contains("~/AppImages"));
    }

    #[test]
    fn desktop_launcher_apps_are_not_removable_through_scope() {
        assert!(
            check_package(
                PackageSource::Desktop,
                "/home/user/.local/share/applications/zed.desktop"
            )
            .protected
        );
    }

    #[test]
    fn toolbox_ides_are_read_only() {
        let protection = check_package(
            PackageSource::JetBrainsToolbox,
            "/home/user/.local/share/JetBrains/Toolbox/apps/pycharm",
        );

        assert!(protection.protected);
        assert_eq!(
            protection.reason.as_deref(),
            Some("JetBrains Toolbox manages IDE updates and uninstall.")
        );
    }

    #[test]
    fn steam_items_are_read_only() {
        let protection = check_package(PackageSource::Steam, "570");

        assert!(protection.protected);
        assert_eq!(
            protection.reason.as_deref(),
            Some("Steam items are read-only in Scope. Manage updates in Steam.")
        );
    }
}
