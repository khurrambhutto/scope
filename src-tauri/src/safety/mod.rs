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
        PackageSource::Manual => check_manual(package_id),
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
    check_path_kind(path, PathKind::AppImage)
}

fn check_manual(package_id: &str) -> Protection {
    // package_id may be a packed "binary\x1fdesktop" id, or a single path
    // (when apply re-checks individual trash targets).
    let (primary, desktop) = crate::scanner::split_manual_id(package_id);
    let kind = if primary.ends_with(".desktop") {
        PathKind::DesktopEntry
    } else {
        PathKind::Manual
    };
    let primary_check = check_path_kind(primary, kind);
    if primary_check.protected {
        return primary_check;
    }
    if let Some(desktop_path) = desktop {
        let desktop_check = check_path_kind(desktop_path, PathKind::DesktopEntry);
        if desktop_check.protected {
            return desktop_check;
        }
    }
    Protection::allowed()
}

#[derive(Clone, Copy)]
enum PathKind {
    AppImage,
    Manual,
    DesktopEntry,
}

/// Guard arbitrary filesystem paths used by AppImage removal (public for tests).
#[cfg_attr(not(test), allow(dead_code))]
pub fn check_path(path: &str) -> Protection {
    check_path_kind(path, PathKind::AppImage)
}

fn check_path_kind(path: &str, kind: PathKind) -> Protection {
    let cleaned = std::path::Path::new(path);
    // Allow directories for Manual bundle roots (e.g. Foo.app, /opt/idea-*).
    let Ok(abs) = cleaned.canonicalize() else {
        return Protection::denied("Path does not resolve to a real file.");
    };
    let s = abs.to_string_lossy();

    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let mut allowed_roots: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from("/opt")];
    if let Some(h) = &home {
        allowed_roots.push(h.join("Applications"));
        allowed_roots.push(h.join("apps"));
        allowed_roots.push(h.join("AppImages"));
        allowed_roots.push(h.join("Downloads"));
        allowed_roots.push(h.join(".local/bin"));
        allowed_roots.push(h.join(".local/opt"));
        allowed_roots.push(h.join(".local/share"));
        allowed_roots.push(h.join(".local/zed.app"));
        // Claude, hermes, etc. often live under ~/.local/share or ~/.claude
        allowed_roots.push(h.join(".claude"));
        allowed_roots.push(h.join(".hermes"));
        allowed_roots.push(h.join(".config"));
    }
    let inside_allowed = allowed_roots.iter().any(|root| {
        let rs = root.display().to_string();
        s.starts_with(&format!("{rs}/")) || s == rs || {
            // Also allow the root itself when it is an install bundle
            // (e.g. ~/.local/zed.app is both root and target).
            root.file_name().is_some() && s == rs
        }
    });
    // Special case: ~/.local/zed.app is listed as a root; also accept any path
    // whose parent chain is under ~/.local and ends with .app
    let local_app_ok = home.as_ref().is_some_and(|h| {
        let local = h.join(".local");
        s.starts_with(&format!("{}/", local.display())) && s.contains(".app")
    });
    if !inside_allowed && !local_app_ok {
        return Protection::denied("File is outside the allowed install directories.");
    }
    match kind {
        PathKind::AppImage => {
            if !s.to_lowercase().ends_with(".appimage") {
                return Protection::denied("Only .AppImage files can be removed this way.");
            }
            if !abs.is_file() {
                return Protection::denied("Path is not a regular file.");
            }
        }
        PathKind::DesktopEntry => {
            if !s.ends_with(".desktop") {
                return Protection::denied("Expected a .desktop file path.");
            }
            // Only allow trashing user-local desktop entries, never system ones.
            let user_apps = home
                .as_ref()
                .map(|h| h.join(".local/share/applications").display().to_string())
                .unwrap_or_default();
            if !user_apps.is_empty() && !s.starts_with(&user_apps) {
                return Protection::denied(
                    "Only user-local .desktop entries can be removed this way.",
                );
            }
            if !abs.is_file() {
                return Protection::denied("Path is not a regular file.");
            }
        }
        PathKind::Manual => {
            // Binary file or install-bundle directory inside allowed roots.
            if !abs.is_file() && !abs.is_dir() {
                return Protection::denied("Path is not a file or directory.");
            }
            // Never allow deleting the allowed root itself (e.g. whole /opt,
            // whole ~/.local/bin) — only children / bundle dirs.
            if allowed_roots.iter().any(|r| {
                r.canonicalize()
                    .map(|c| c == abs)
                    .unwrap_or(false)
                    && !s.ends_with(".app")
            }) {
                // ~/.local/zed.app is both a root and a valid bundle — allow .app
                if !s.ends_with(".app") {
                    return Protection::denied("Refusing to remove an entire system directory.");
                }
            }
        }
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

    #[test]
    fn blocks_manual_outside_allowed_dirs() {
        assert!(check_package(PackageSource::Manual, "/etc/passwd").protected);
        assert!(check_package(PackageSource::Manual, "/usr/bin/bash").protected);
    }
}
