//! Scope design tokens, formatting helpers, and source/kind presentation.
//!
//! Dark red surfaces, burgundy borders, and coral emphasis shared by every control.

use gpui_kit::{rgb, App, Hsla};

use crate::domain::package::{
    AppKind, InstalledPackage, PackageSource, SteamUpdateStatus,
};

pub fn source_color(source: PackageSource) -> Hsla {
    match source {
        PackageSource::Apt => rgb(0xa1352c),
        PackageSource::Snap => rgb(0x2196f3),
        PackageSource::Flatpak => rgb(0x4a154b),
        PackageSource::AppImage => rgb(0x0b8a4f),
        PackageSource::Desktop => rgb(0x7569d2),
        PackageSource::Steam => rgb(0x1b5c8c),
    }
    .into()
}

pub fn kind_color(kind: AppKind) -> Hsla {
    match kind {
        AppKind::Gui => rgb(0x2f8fa3),
        AppKind::Cli => rgb(0xa97b2e),
        AppKind::Game => rgb(0x5a7dc7),
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
        PackageSource::Desktop => "Desktop",
        PackageSource::Steam => "Steam",
    }
}

pub fn kind_label(kind: AppKind) -> &'static str {
    match kind {
        AppKind::Gui => "GUI",
        AppKind::Cli => "CLI",
        AppKind::Game => "Game",
        AppKind::Unknown => "Unknown",
    }
}

/// Package-specific source text for rows and detail classifications.
pub fn package_source_label(pkg: &InstalledPackage) -> &'static str {
    if pkg.source == PackageSource::Steam && pkg.app_kind == AppKind::Game {
        "Steam Game"
    } else {
        source_label(pkg.source)
    }
}

/// Steam's locally recorded update state. This does not check Steam's live state.
pub fn steam_update_status_label(status: Option<SteamUpdateStatus>) -> &'static str {
    match status {
        Some(SteamUpdateStatus::Pending) => "Update pending in Steam",
        Some(SteamUpdateStatus::Paused) => "Update paused in Steam",
        Some(SteamUpdateStatus::InProgress) => "Update in progress in Steam",
        Some(SteamUpdateStatus::NoUpdateRecorded) => "No update recorded",
        Some(SteamUpdateStatus::Unknown) | None => "Status unknown",
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
    format_item_count(shown, total, "app", "apps")
}

/// Footer status for the unfiltered inventory view.
pub fn format_all_count(shown: usize, total: usize) -> String {
    format_item_count(shown, total, "item", "items")
}

fn format_item_count(shown: usize, total: usize, singular: &str, plural: &str) -> String {
    if shown == total {
        format!(
            "Showing {total} {}",
            if total == 1 { singular } else { plural }
        )
    } else {
        format!("Showing {shown} of {total} {plural}")
    }
}

/// Elapsed seconds as `m:ss`.
pub fn format_elapsed(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// Colored-initials fallback for packages without a resolvable icon.
pub fn initials(name: &str) -> String {
    let mut parts = name
        .trim()
        .split(|c: char| c.is_whitespace() || c == '_' || c == '-')
        .filter(|p| !p.is_empty());
    match (parts.next(), parts.next()) {
        (None, _) => "?".to_string(),
        (Some(only), None) => only.chars().take(2).collect::<String>().to_uppercase(),
        (Some(first), Some(second)) => {
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
    pkg.display_name.clone().unwrap_or_else(|| pkg.name.clone())
}

// ---- Scope palette (moved from the app view) ------------------------------

// ---- Palette ---------------------------------------------------------------

pub fn border() -> Hsla {
    rgb(0x4b3034).into()
}
pub fn text() -> Hsla {
    rgb(0xefe6e4).into()
}
pub fn text_dim() -> Hsla {
    rgb(0xbca5a1).into()
}
pub fn text_faint() -> Hsla {
    rgb(0x9e8581).into()
}
pub fn accent() -> Hsla {
    rgb(0xd4504a).into()
}
pub fn danger() -> Hsla {
    rgb(0xc9443e).into()
}
pub fn elev() -> Hsla {
    rgb(0x211719).into()
}
pub fn elev2() -> Hsla {
    rgb(0x291c1f).into()
}

/// Emphasis text stays readable on the dark red control surfaces.
pub fn accent_text() -> Hsla {
    rgb(0xf08c82).into()
}
pub fn hover_surface() -> Hsla {
    rgb(0x352327).into()
}
pub fn selected_surface() -> Hsla {
    rgb(0x3d2529).into()
}
pub fn border_hover() -> Hsla {
    rgb(0x9c625e).into()
}
pub fn primary() -> Hsla {
    rgb(0xa83a36).into()
}
pub fn primary_hover() -> Hsla {
    rgb(0xbc4540).into()
}
pub fn primary_pressed() -> Hsla {
    rgb(0x94332f).into()
}
pub fn on_accent() -> Hsla {
    rgb(0xfffaf8).into()
}
pub fn background_top() -> Hsla {
    rgb(0x2d1414).into()
}
pub fn background_bottom() -> Hsla {
    rgb(0x0b0c0f).into()
}

/// Keep kit-owned input, caret, focus, and button states in Scope's palette.
pub fn init(cx: &mut App) {
    use gpui_kit::component::{Theme, ThemeMode};
    Theme::change(ThemeMode::Dark, None, cx);
    let theme = Theme::global_mut(cx);
    theme.radius = gpui_kit::px(16.);
    theme.radius_lg = gpui_kit::px(16.);
    let colors = &mut theme.colors;
    colors.background = background_bottom();
    colors.foreground = text();
    colors.border = border();
    colors.input = border();
    colors.ring = border_hover();
    colors.caret = accent_text();
    colors.muted = elev();
    colors.muted_foreground = text_faint();
    colors.accent = selected_surface();
    colors.accent_foreground = accent_text();
    colors.secondary = elev2();
    colors.secondary_foreground = text_dim();
    colors.secondary_hover = hover_surface();
    colors.secondary_active = selected_surface();
    colors.link = accent_text();
    colors.link_hover = text();
    colors.link_active = accent_text();
    colors.selection = selected_surface();
    colors.primary = primary();
    colors.primary_foreground = on_accent();
    colors.primary_hover = primary_hover();
    colors.primary_active = primary_pressed();
    colors.button = elev2();
    colors.button_foreground = text_dim();
    colors.button_hover = hover_surface();
    colors.button_active = selected_surface();
    colors.button_primary = primary();
    colors.button_primary_foreground = on_accent();
    colors.button_primary_hover = primary_hover();
    colors.button_primary_active = primary_pressed();
    colors.danger = danger();
    colors.danger_foreground = on_accent();
    colors.danger_hover = primary_hover();
    colors.danger_active = primary_pressed();
    colors.button_danger = danger();
    colors.button_danger_foreground = on_accent();
    colors.button_danger_hover = primary_hover();
    colors.button_danger_active = primary_pressed();
    theme.tokens = (&theme.colors).into();
    Theme::sync_base(cx);
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
    fn format_all_count_names_inventory_items() {
        assert_eq!(format_all_count(12, 12), "Showing 12 items");
        assert_eq!(format_all_count(1, 1), "Showing 1 item");
        assert_eq!(format_all_count(5, 20), "Showing 5 of 20 items");
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
    fn source_label_covers_every_source() {
        assert_eq!(source_label(PackageSource::Apt), "APT");
        assert_eq!(source_label(PackageSource::Snap), "Snap");
        assert_eq!(source_label(PackageSource::Flatpak), "Flatpak");
        assert_eq!(source_label(PackageSource::AppImage), "AppImage");
        assert_eq!(source_label(PackageSource::Desktop), "Desktop");
    }


    #[test]
    fn kind_label_covers_every_kind() {
        assert_eq!(kind_label(AppKind::Gui), "GUI");
        assert_eq!(kind_label(AppKind::Cli), "CLI");
        assert_eq!(kind_label(AppKind::Unknown), "Unknown");
    }
}
