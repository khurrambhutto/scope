//! Package data structures for scope

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Source of the package installation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackageSource {
    Apt,
    Snap,
    Flatpak,
    AppImage,
    DebFile,
}

impl fmt::Display for PackageSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageSource::Apt => write!(f, "apt"),
            PackageSource::Snap => write!(f, "snap"),
            PackageSource::Flatpak => write!(f, "flatpak"),
            PackageSource::AppImage => write!(f, "appimage"),
            PackageSource::DebFile => write!(f, "deb"),
        }
    }
}

impl PackageSource {
    pub fn color(&self) -> ratatui::style::Color {
        crate::theme::get_theme().source_color(self)
    }
}

/// Type of application (GUI or CLI)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AppType {
    GUI,
    CLI,
    #[default]
    Unknown,
}

impl fmt::Display for AppType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppType::GUI => write!(f, "GUI"),
            AppType::CLI => write!(f, "CLI"),
            AppType::Unknown => write!(f, "???"),
        }
    }
}

/// Represents an installed package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Package name
    pub name: String,
    /// Installed version
    pub version: String,
    /// Package description
    pub description: String,
    /// Size in bytes
    pub size_bytes: u64,
    /// Source package manager
    pub source: PackageSource,
    /// GUI or CLI application
    pub app_type: AppType,
    /// Whether an update is available
    pub has_update: Option<bool>,
    /// Version available for update
    pub update_version: Option<String>,
    /// Installation path (mainly for AppImages)
    pub install_path: Option<String>,
    /// Whether this package is selected (for batch operations)
    #[serde(skip)]
    pub selected: bool,
}

impl Package {
    pub fn new(name: String, source: PackageSource) -> Self {
        Self {
            name,
            version: String::new(),
            description: String::new(),
            size_bytes: 0,
            source,
            app_type: AppType::Unknown,
            has_update: None,
            update_version: None,
            install_path: None,
            selected: false,
        }
    }

    /// Get human-readable size string
    pub fn size_human(&self) -> String {
        use humansize::{format_size, BINARY};
        format_size(self.size_bytes, BINARY)
    }

    /// Calculate search relevance score (lower = better match). Returns None if no match.
    pub fn search_relevance(&self, query: &str, matcher: &SkimMatcherV2) -> Option<u8> {
        if query.trim().is_empty() {
            return Some(0);
        }

        let query_lower = query.to_lowercase();
        let name_lower = self.name.to_lowercase();
        let desc_lower = self.description.to_lowercase();

        if name_lower == query_lower {
            return Some(0);
        }
        if name_lower.starts_with(&query_lower) {
            return Some(1);
        }
        if name_lower.contains(&query_lower) {
            return Some(2);
        }
        if desc_lower.contains(&query_lower) {
            return Some(3);
        }
        if matcher.fuzzy_match(&name_lower, &query_lower).is_some() {
            return Some(4);
        }
        if matcher.fuzzy_match(&desc_lower, &query_lower).is_some() {
            return Some(5);
        }

        None
    }
}

/// Sort criteria for packages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortCriteria {
    #[default]
    SizeDesc,
    SizeAsc,
    NameAsc,
    NameDesc,
    SourceAsc,
}

/// Sort packages based on criteria
pub fn sort_packages(packages: &mut [Package], criteria: SortCriteria) {
    match criteria {
        SortCriteria::SizeDesc => packages.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
        SortCriteria::SizeAsc => packages.sort_by(|a, b| a.size_bytes.cmp(&b.size_bytes)),
        SortCriteria::NameAsc => {
            packages.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }
        SortCriteria::NameDesc => {
            packages.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase()))
        }
        SortCriteria::SourceAsc => packages.sort_by(|a, b| {
            let source_cmp = (a.source as u8).cmp(&(b.source as u8));
            if source_cmp == std::cmp::Ordering::Equal {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else {
                source_cmp
            }
        }),
    }
}

