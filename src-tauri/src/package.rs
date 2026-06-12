//! Package data structures for scope

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
    /// Alternative names commonly used to search for this package
    pub aliases: Vec<String>,


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
            aliases: Vec::new(),
        }
    }

}

/// Sort criteria for packages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortCriteria {
    #[default]
    SizeDesc,
}

/// Sort packages based on criteria
pub fn sort_packages(packages: &mut [Package], criteria: SortCriteria) {
    match criteria {
        SortCriteria::SizeDesc => packages.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes)),
    }
}
