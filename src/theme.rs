//! Theme configuration for scope TUI
//!
//! Centralized theme system for consistent styling across the application.
//! Uses a "Retro Warmth" Gruvbox-inspired color palette.

use ratatui::style::{Color, Modifier, Style};

/// Main theme struct containing all color definitions
#[derive(Debug, Clone)]
pub struct Theme {
    // Base colors
    pub background: Color,
    pub selection_bg: Color,

    // Text hierarchy
    pub primary_text: Color,
    pub secondary_text: Color,
    pub tertiary_text: Color,

    // UI element colors
    pub border: Color,
    pub border_focused: Color,

    // Semantic colors
    pub cli_indicator: Color,
    pub warning: Color,
    pub success: Color,

    // Package source colors (for visual distinction)
    pub source_apt: Color,
    pub source_snap: Color,
    pub source_flatpak: Color,
    pub source_appimage: Color,
    pub source_deb: Color,
}

impl Default for Theme {
    fn default() -> Self {
        // Retro Warmth - Gruvbox inspired palette
        let background = Color::Rgb(29, 32, 33);        // #1d2021 - Soft dark background
        let selection_bg = Color::Rgb(60, 56, 54);      // #3c3836 - Selection background

        let primary_text = Color::Rgb(235, 219, 178);   // #ebdbb2 - Warm cream/beige
        let secondary_text = Color::Rgb(213, 196, 161); // #d5c4a1 - Muted beige
        let tertiary_text = Color::Rgb(168, 153, 132);  // #a89984 - Darkened beige

        let border = Color::Rgb(184, 187, 38);          // #b8bb26 - Warm yellow-green
        let cli_indicator = Color::Rgb(254, 128, 25);   // #fe8019 - Warm orange (informational)
        let warning = Color::Rgb(251, 73, 52);          // #fb4934 - Bright red (alerts/errors)
        let success = Color::Rgb(184, 187, 38);         // #b8bb26 - Yellow-green (same as border)

        // Gruvbox accent colors for package sources
        let aqua = Color::Rgb(142, 192, 124);           // #8ec07c - Aqua/green
        let purple = Color::Rgb(211, 134, 155);         // #d3869b - Purple/pink
        let blue = Color::Rgb(131, 165, 152);           // #83a598 - Blue/teal

        Self {
            // Base colors
            background,
            selection_bg,

            // Text hierarchy
            primary_text,
            secondary_text,
            tertiary_text,

            // UI elements
            border,
            border_focused: cli_indicator, // Use orange for focused state

            // Semantic colors
            cli_indicator,
            warning,
            success,

            // Package source colors (distinct but harmonious)
            source_apt: primary_text,       // Default/apt uses primary text
            source_snap: purple,            // Snap = purple
            source_flatpak: blue,           // Flatpak = blue
            source_appimage: aqua,          // AppImage = aqua
            source_deb: secondary_text,     // Deb files = secondary
        }
    }
}

impl Theme {
    /// Create a new theme with the Retro Warmth color scheme
    pub fn new() -> Self {
        Self::default()
    }

    // === Style helpers ===

    /// Get the base style with background
    pub fn base_style(&self) -> Style {
        Style::default().bg(self.background).fg(self.primary_text)
    }

    /// Get style for primary text (package names, main content)
    pub fn primary_style(&self) -> Style {
        Style::default().fg(self.primary_text)
    }

    /// Get style for primary text with bold
    pub fn primary_bold(&self) -> Style {
        Style::default()
            .fg(self.primary_text)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for secondary/muted text (metadata, descriptions)
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.secondary_text)
    }

    /// Get style for borders
    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    /// Get style for focused borders
    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.border_focused)
    }

    /// Get style for selected/highlighted items
    pub fn selection_style(&self) -> Style {
        Style::default()
            .bg(self.selection_bg)
            .fg(self.primary_text)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for success messages (same as border for cohesion)
    pub fn success_style(&self) -> Style {
        Style::default().fg(self.success)
    }

    /// Get style for warning messages (bright red for critical info)
    pub fn warning_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Get style for error messages (same as warning - red for alerts)
    pub fn error_style(&self) -> Style {
        Style::default().fg(self.warning)
    }

    /// Get style for table headers (Name, Source, Type, etc.) - tertiary text
    pub fn header_style(&self) -> Style {
        Style::default()
            .fg(self.tertiary_text)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for block/panel titles
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.primary_text)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for labels (like "Version:", "Size:", etc.)
    pub fn label_style(&self) -> Style {
        Style::default().fg(self.secondary_text)
    }

    /// Get style for sidebar items
    pub fn sidebar_style(&self) -> Style {
        Style::default().bg(self.background).fg(self.secondary_text)
    }

    /// Get style for selected sidebar items
    pub fn sidebar_selected_style(&self) -> Style {
        Style::default()
            .bg(self.selection_bg)
            .fg(self.primary_text)
            .add_modifier(Modifier::BOLD)
    }

    /// Get style for CLI type indicator (informational orange)
    pub fn cli_style(&self) -> Style {
        Style::default().fg(self.cli_indicator)
    }

    // === Package source color helpers ===

    /// Get color for a package source
    pub fn source_color(&self, source: &crate::package::PackageSource) -> Color {
        use crate::package::PackageSource;
        match source {
            PackageSource::Apt => self.source_apt,
            PackageSource::Snap => self.source_snap,
            PackageSource::Flatpak => self.source_flatpak,
            PackageSource::AppImage => self.source_appimage,
            PackageSource::DebFile => self.source_deb,
        }
    }

    /// Get color for an app type
    pub fn app_type_color(&self, app_type: &crate::package::AppType) -> Color {
        use crate::package::AppType;
        match app_type {
            AppType::GUI => self.success,        // Yellow-green for GUI apps
            AppType::CLI => self.cli_indicator,  // Orange for CLI apps (distinct from warnings)
            AppType::Unknown => self.tertiary_text,
        }
    }
}

// Legacy compatibility - keeping 'secondary' field accessible for existing code
impl Theme {
    /// Legacy: Get secondary color (now maps to secondary_text for backwards compatibility)
    #[inline]
    pub fn secondary(&self) -> Color {
        self.secondary_text
    }
}

/// Global theme instance - for easy access across the app
/// In the future, this could be loaded from a config file
pub fn get_theme() -> Theme {
    Theme::default()
}
