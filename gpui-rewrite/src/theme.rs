//! Scope design tokens, formatting helpers, and source/kind presentation.
//!
//! Mirrors the palette and copy in the original `src/App.css` / `src/features/
//! packages/format.ts` so the GPUI rewrite reads the same as the Tauri build.

use gpui::{rgb, Hsla};

use crate::domain::package::{AppKind, InstalledPackage, PackageSource};

pub fn source_color(source: PackageSource) -> Hsla {
    match source {
        PackageSource::Apt => rgb(0xa1352c),
        PackageSource::Snap => rgb(0x2196f3),
        PackageSource::Flatpak => rgb(0x4a154b),
        PackageSource::AppImage => rgb(0x0b8a4f),
    }
    .into()
}

pub fn kind_color(kind: AppKind) -> Hsla {
    match kind {
        AppKind::Gui => rgb(0x2f8fa3),
        AppKind::Cli => rgb(0xa97b2e),
        AppKind::Unknown => rgb(0x6b625e),
    }
    .into()
}

pub fn source_label(source: PackageSource) -> &'static str {
    match source {
        PackageSource::Apt => "APT",
        PackageSource::Snap => "Snap",
        PackageSource::Flatpak => "Flatpak",
        PackageSource::AppImage => "AppImage",
    }
}

pub fn kind_label(kind: AppKind) -> &'static str {
    match kind {
        AppKind::Gui => "GUI",
        AppKind::Cli => "CLI",
        AppKind::Unknown => "Unknown",
    }
}

/// Human-readable byte size, matching the TypeScript `formatSize`.
pub fn format_size(bytes: u64) -> String {
    if bytes == 0 {
        return "—".to_string();
    }
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    let digits = if unit == 0 {
        0
    } else if value < 10.0 {
        1
    } else {
        0
    };
    format!("{value:.digits$} {}", UNITS[unit])
}

/// Footer status line, matching the TypeScript `formatAppCount`.
pub fn format_app_count(shown: usize, total: usize) -> String {
    if shown == total {
        format!("Showing {total} {}", if total == 1 { "app" } else { "apps" })
    } else {
        format!("Showing {shown} of {total} apps")
    }
}

/// Elapsed seconds as `m:ss`.
pub fn format_elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Colored-initials fallback for packages without a resolvable icon.
pub fn initials(name: &str) -> String {
    let parts: Vec<&str> = name
        .trim()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter(|p| !p.is_empty())
        .collect();
    match parts.as_slice() {
        [] => "?".to_string(),
        [only] => only.chars().take(2).collect::<String>().to_uppercase(),
        [first, second, ..] => {
            let a = first.chars().next().unwrap_or('?');
            let b = second.chars().next().unwrap_or('?');
            format!("{a}{b}").to_uppercase()
        }
    }
}

/// Lowercased search blob for one package, matching the TypeScript `searchText`.
pub fn search_text(pkg: &InstalledPackage) -> String {
    [
        pkg.name.as_str(),
        pkg.display_name.as_deref().unwrap_or(""),
        pkg.description.as_deref().unwrap_or(""),
        pkg.package_id.as_str(),
        pkg.install_scope.map(|s| s.id()).unwrap_or(""),
        pkg.categories.as_deref().unwrap_or(""),
        pkg.version.as_str(),
    ]
    .join(" ")
    .to_lowercase()
}

/// The name a row/detail/dialog should show for a package.
pub fn display_title(pkg: &InstalledPackage) -> String {
    pkg.display_name
        .clone()
        .unwrap_or_else(|| pkg.name.clone())
}

// ---- Scope palette (moved from the app view) ------------------------------

// ---- Palette ---------------------------------------------------------------

pub fn border() -> Hsla {
    rgb(0x2d2325).into()
}
pub fn text() -> Hsla {
    rgb(0xefe6e4).into()
}
pub fn text_dim() -> Hsla {
    rgb(0xa69692).into()
}
pub fn text_faint() -> Hsla {
    rgb(0x786c68).into()
}
pub fn accent() -> Hsla {
    rgb(0xd4504a).into()
}
pub fn danger() -> Hsla {
    rgb(0xc9443e).into()
}
pub fn update_green() -> Hsla {
    rgb(0x24795f).into()
}
pub fn elev() -> Hsla {
    rgb(0x171315).into()
}
pub fn elev2() -> Hsla {
    rgb(0x1e181a).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::{InstallScope, InstalledPackage, PackageSource};

    fn pkg() -> InstalledPackage {
        InstalledPackage::new(PackageSource::Apt, "htop")
    }

    #[test]
    fn format_size_renders_human_units() {
        assert_eq!(format_size(0), "—");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(10 * 1024), "10 KB");
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_size(5 * 1024 * 1024 * 1024), "5.0 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024 * 1024), "2.0 TB");
    }

    #[test]
    fn format_app_count_matches_the_typescript_copy() {
        assert_eq!(format_app_count(3, 3), "Showing 3 apps");
        assert_eq!(format_app_count(1, 1), "Showing 1 app");
        assert_eq!(format_app_count(0, 0), "Showing 0 apps");
        assert_eq!(format_app_count(2, 10), "Showing 2 of 10 apps");
    }

    #[test]
    fn format_elapsed_is_minutes_and_seconds() {
        assert_eq!(format_elapsed(0), "0:00");
        assert_eq!(format_elapsed(9), "0:09");
        assert_eq!(format_elapsed(65), "1:05");
        assert_eq!(format_elapsed(35999), "599:59");
    }

    #[test]
    fn initials_take_the_first_letters_of_two_words() {
        assert_eq!(initials("Google Chrome"), "GC");
        assert_eq!(initials("libre-office"), "LO");
        assert_eq!(initials("htop"), "HT");
        assert_eq!(initials("  spaced  out  "), "SO");
        assert_eq!(initials("   "), "?");
    }

    #[test]
    fn search_text_covers_every_matchable_field() {
        let mut pkg = InstalledPackage::new(PackageSource::Flatpak, "org.test.App");
        pkg.name = "Test".to_string();
        pkg.display_name = Some("Test App".to_string());
        pkg.description = Some("A test app".to_string());
        pkg.version = "1.2".to_string();
        pkg.categories = Some("Utility".to_string());
        pkg.install_scope = Some(InstallScope::User);
        assert_eq!(
            search_text(&pkg),
            "test test app a test app org.test.app user utility 1.2"
        );
    }

    #[test]
    fn display_title_prefers_the_desktop_entry_name() {
        let mut pkg = pkg();
        pkg.name = "htop".to_string();
        assert_eq!(display_title(&pkg), "htop");
        pkg.display_name = Some("Htop Process Viewer".to_string());
        assert_eq!(display_title(&pkg), "Htop Process Viewer");
    }

    #[test]
    fn labels_are_stable_copy() {
        assert_eq!(source_label(PackageSource::Apt), "APT");
        assert_eq!(source_label(PackageSource::AppImage), "AppImage");
        assert_eq!(kind_label(AppKind::Gui), "GUI");
        assert_eq!(kind_label(AppKind::Unknown), "Unknown");
    }
}
