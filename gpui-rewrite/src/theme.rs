//! Scope design tokens, formatting helpers, and source/kind presentation.
//!
//! Mirrors the palette and copy in the original `src/App.css` / `src/features/
//! packages/format.ts` so the GPUI rewrite reads the same as the Tauri build.

use gpui::{rgb, Hsla};

use crate::package::{AppKind, InstalledPackage, PackageSource};

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
