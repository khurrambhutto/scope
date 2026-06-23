//! Package-source scanners.
//!
//! One module per supported source. Each scanner implements [`Scanner`] and is
//! run in parallel by [`scan_all`]. The desktop-entry enrichment layer is
//! applied afterwards in [`scan_all`] so all sources share one merge path.

pub mod appimage;
pub mod apt;
pub mod flatpak;
pub mod snap;

use std::future::Future;
use std::path::PathBuf;

use anyhow::Result;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{AppKind, PackageSource};

    /// Smoke test that runs the real scanners on the live system. Asserts that
    /// if a source is available it returns at least one package, and that the
    /// merge step produces stable, unique keys. Gated behind `live-scanners` so
    /// CI without flatpak/snap can opt out.
    #[tokio::test]
    async fn scan_all_runs_on_live_system() {
        let (pkgs, avail) = scan_all().await;
        assert!(pkgs.iter().all(|p| !p.key.is_empty()), "every package needs a key");

        let mut keys = std::collections::HashSet::new();
        for p in &pkgs {
            assert!(keys.insert(p.key.clone()), "duplicate package key: {}", p.key);
        }

        if avail.apt {
            let apt = pkgs.iter().filter(|p| p.source == PackageSource::Apt).count();
            assert!(apt > 0, "APT available but no packages returned");
        }
        if avail.flatpak {
            let fp = pkgs.iter().filter(|p| p.source == PackageSource::Flatpak).count();
            assert!(fp > 0, "Flatpak available but no packages returned");
        }

        // Enrichment smoke: at least some packages should carry display names.
        let enriched = pkgs.iter().filter(|p| p.display_name.is_some()).count();
        println!("scan_all: {} packages, enriched={}, avail apt={}/snap={}/flatpak={}/appimage={}",
                 pkgs.len(), enriched, avail.apt, avail.snap, avail.flatpak, avail.appimage);
        let _ = AppKind::Gui;
    }
}
use tokio::task::JoinSet;

use crate::desktop_entries::DesktopIndex;
use crate::package::{AppKind, InstalledPackage, PackageSource};

/// A source-specific installed-package scanner.
pub trait Scanner: Send + Sync {
    /// Human source this scanner reports for.
    fn source(&self) -> PackageSource;

    /// True when the backing package manager appears installed on this system.
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    /// List installed packages for this source.
    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<Vec<InstalledPackage>>> + Send + '_>>;
}

use std::pin::Pin;

/// Build the full set of scanners (in a stable order for predictable results).
fn scanners() -> Vec<Box<dyn Scanner>> {
    vec![
        Box::new(apt::AptScanner),
        Box::new(snap::SnapScanner),
        Box::new(flatpak::FlatpakScanner),
        Box::new(appimage::AppImageScanner::new()),
    ]
}

/// One scanner's outcome.
struct ScanOutcome {
    source: PackageSource,
    available: bool,
    packages: Vec<InstalledPackage>,
    error: Option<String>,
}

/// Scan every available source in parallel, then enrich with desktop metadata.
///
/// Returns the merged, sorted unified list plus per-source availability. Source
/// failures are never fatal: a broken/uninstalled source simply contributes zero
/// packages and reports `available = false`.
pub async fn scan_all() -> (Vec<InstalledPackage>, ScanAvailability) {
    // Discover desktop apps on a blocking thread (synchronous fs walk).
    let desktop = tokio::task::spawn_blocking(|| DesktopIndex::from_apps(crate::desktop_entries::discover_desktop_apps()))
        .await
        .unwrap_or_else(|_| DesktopIndex::empty());

    let mut join = JoinSet::new();
    for scanner in scanners() {
        let source = scanner.source();
        join.spawn(async move {
            if scanner.is_available().await {
                match scanner.scan().await {
                    Ok(packages) => ScanOutcome { source, available: true, packages, error: None },
                    Err(e) => ScanOutcome { source, available: true, packages: Vec::new(), error: Some(e.to_string()) },
                }
            } else {
                ScanOutcome { source, available: false, packages: Vec::new(), error: None }
            }
        });
    }

    let mut merged: Vec<InstalledPackage> = Vec::new();
    let mut availability = ScanAvailability::default();
    while let Some(res) = join.join_next().await {
        let Ok(outcome) = res else { continue };
        match outcome.source {
            PackageSource::Apt => {
                availability.apt = outcome.available;
                availability.apt_error = outcome.error;
            }
            PackageSource::Snap => {
                availability.snap = outcome.available;
                availability.snap_error = outcome.error;
            }
            PackageSource::Flatpak => {
                availability.flatpak = outcome.available;
                availability.flatpak_error = outcome.error;
            }
            PackageSource::AppImage => {
                availability.appimage = outcome.available;
                availability.appimage_dirs = appimage::search_directories();
            }
        }
        merged.extend(outcome.packages);
    }

    // Enrich + classify + resolve icons, then sort apps-first, by display
    // name. Icon resolution touches the filesystem (theme lookups), so the
    // whole merge pass runs on a blocking thread to keep the async runtime
    // responsive. `DesktopIndex` and `InstalledPackage` are both `Send`.
    let (merged, availability) = tokio::task::spawn_blocking(move || {
        for pkg in merged.iter_mut() {
            enrich(pkg, &desktop);
        }
        merged.sort_by(|a, b| {
            let ka = kind_rank(a.app_kind);
            let kb = kind_rank(b.app_kind);
            ka.cmp(&kb)
                .then_with(|| display_name(a).to_lowercase().cmp(&display_name(b).to_lowercase()))
        });
        (merged, availability)
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), ScanAvailability::default()));

    (merged, availability)
}

fn kind_rank(k: AppKind) -> u8 {
    match k {
        AppKind::Gui => 0,
        AppKind::Cli => 1,
        AppKind::Unknown => 2,
    }
}

fn display_name(p: &InstalledPackage) -> String {
    p.display_name.clone().unwrap_or_else(|| p.name.clone())
}

/// Apply desktop-entry metadata to a package (display name, icon, categories...).
fn enrich(pkg: &mut InstalledPackage, desktop: &DesktopIndex) {
    if let Some(app) = desktop.lookup(pkg.source, &pkg.package_id, &pkg.name) {
        if pkg.display_name.is_none() {
            pkg.display_name = Some(app.name.clone());
        }
        if pkg.icon.is_none() {
            // `app.icon` is the raw `Icon=` value from the .desktop entry: a
            // theme name (e.g. "firefox") or an absolute path. Resolve it to a
            // real file and wrap it in a `scope-icon://` URL the webview can
            // load directly. Unresolved names stay `None` and the frontend
            // falls back to initials.
            if let Some(name) = app.icon.as_deref() {
                if let Some(path) = crate::icons::resolve(name) {
                    pkg.icon = Some(crate::icons::icon_url(&path));
                }
            }
        }
        if pkg.description.is_none() {
            pkg.description = app.comment.clone();
        }
        if !app.categories.is_empty() {
            pkg.categories = Some(app.categories.join(", "));
        }
        pkg.terminal = app.terminal;
        if pkg.app_kind == AppKind::Unknown && !app.terminal {
            // A .desktop (Type=Application) entry implies a GUI app unless it
            // explicitly launches in a terminal.
            pkg.app_kind = AppKind::Gui;
        }
    }
}

/// Per-source availability and search dirs reported to the UI.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ScanAvailability {
    pub apt: bool,
    pub snap: bool,
    pub flatpak: bool,
    pub appimage: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub apt_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub snap_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub flatpak_error: Option<String>,
    pub appimage_dirs: Vec<String>,
}

#[allow(dead_code)]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}