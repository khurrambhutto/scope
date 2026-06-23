//! Safety: protected packages, protected paths, and backend validation.
//!
//! No destructive operation may run before passing through here. The deny-list
//! is enforced in the backend, independent of any frontend state, so a crafted
//! `invoke` call can never remove a system-critical package or a protected path.

use crate::package::PackageSource;

/// Reason a package/path is protected, surfaced to the UI.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Protection {
    pub protected: bool,
    pub reason: Option<String>,
}

impl Protection {
    pub fn allowed() -> Self {
        Self {
            protected: false,
            reason: None,
        }
    }
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
        PackageSource::Apt => check_apt(package_id),
        PackageSource::Snap => check_snap(package_id),
        PackageSource::Flatpak => check_flatpak(package_id),
        PackageSource::AppImage => check_appimage(package_id),
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

    if CRITICAL.iter().any(|c| n == *c) {
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

fn check_appimage(path: &str) -> Protection {
    check_path(path)
}

/// Guard arbitrary filesystem paths used by AppImage removal.
pub fn check_path(path: &str) -> Protection {
    let cleaned = std::path::Path::new(path);
    let Ok(abs) = cleaned.canonicalize() else {
        return Protection::denied("Path does not resolve to a real file.");
    };
    let s = abs.to_string_lossy();

    // Never allow operations outside expected AppImage locations or on system dirs.
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let mut allowed_roots: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from("/opt")];
    if let Some(h) = &home {
        allowed_roots.push(h.join("Applications"));
        allowed_roots.push(h.join("apps"));
        allowed_roots.push(h.join("AppImages"));
        allowed_roots.push(h.join("Downloads"));
        allowed_roots.push(h.join(".local/bin"));
    }
    let inside_allowed = allowed_roots.iter().any(|root| {
        s.starts_with(&format!("{}/", root.display())) || s == root.display().to_string()
    });
    if !inside_allowed {
        return Protection::denied("File is outside the allowed AppImage directories.");
    }
    // Must be an AppImage.
    if !s.to_lowercase().ends_with(".appimage") {
        return Protection::denied("Only .AppImage files can be removed this way.");
    }
    if !abs.is_file() {
        return Protection::denied("Path is not a regular file.");
    }
    Protection::allowed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_critical_apt() {
        assert!(check_package(PackageSource::Apt, "ubuntu-desktop").protected);
        assert!(check_package(PackageSource::Apt, "systemd").protected);
        assert!(check_package(PackageSource::Apt, "linux-image-6.8.0-45-generic").protected);
        assert!(check_package(PackageSource::Apt, "apt").protected);
        assert!(check_package(PackageSource::Apt, "pkexec").protected);
    }

    #[test]
    fn blocks_libraries_but_allows_libreoffice() {
        assert!(check_package(PackageSource::Apt, "libssl3").protected);
        assert!(check_package(PackageSource::Apt, "libgtk-3-0").protected);
        assert!(!check_package(PackageSource::Apt, "libreoffice-writer").protected);
    }

    #[test]
    fn allows_regular_apps() {
        assert!(!check_package(PackageSource::Apt, "firefox").protected);
        assert!(!check_package(PackageSource::Apt, "vlc").protected);
        assert!(!check_package(PackageSource::Snap, "firefox").protected);
        assert!(!check_package(PackageSource::Flatpak, "org.mozilla.firefox").protected);
    }

    #[test]
    fn blocks_snap_runtimes() {
        assert!(check_package(PackageSource::Snap, "core20").protected);
        assert!(check_package(PackageSource::Snap, "gtk-common-themes").protected);
        assert!(check_package(PackageSource::Snap, "snapd").protected);
    }

    #[test]
    fn blocks_appimage_outside_allowed_dirs() {
        assert!(check_path("/etc/passwd").protected);
        assert!(check_path("/usr/bin/bash").protected);
        assert!(check_path("/nonexistent.AppImage").protected);
    }
}
