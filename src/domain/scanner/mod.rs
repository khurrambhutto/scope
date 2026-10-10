//! Package-source scanners.
//!
//! One module per supported source. Each scanner implements [`Scanner`] and is
//! run in parallel by [`scan_all`]. The desktop-entry enrichment layer is
//! applied afterwards in [`scan_all`] so all sources share one merge path.

pub mod appimage;
pub mod apt;
pub mod flatpak;
pub mod snap;
pub mod steam;

use std::future::Future;

use anyhow::Result;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::{AppKind, PackageSource};

    /// Smoke test that runs the real scanners on the live system. Asserts that
    /// if a source is available it returns at least one package, and that the
    /// merge step produces stable, unique keys. Ignored by default so `cargo
    /// test` stays hermetic on machines without apt/snap/flatpak; run it with
    /// `cargo test -- --ignored`.
    #[tokio::test]
    #[ignore = "runs the live package managers; run with `cargo test -- --ignored`"]
    async fn scan_all_runs_on_live_system() {
        let (pkgs, avail) = scan_all().await;
        assert!(
            pkgs.iter().all(|p| !p.key.is_empty()),
            "every package needs a key"
        );

        let mut keys = std::collections::HashSet::new();
        for p in &pkgs {
            assert!(
                keys.insert(p.key.clone()),
                "duplicate package key: {}",
                p.key
            );
        }

        if avail.apt {
            let apt = pkgs
                .iter()
                .filter(|p| p.source == PackageSource::Apt)
                .count();
            assert!(apt > 0, "APT available but no packages returned");
        }
        if avail.flatpak {
            let fp = pkgs
                .iter()
                .filter(|p| p.source == PackageSource::Flatpak)
                .count();
            assert!(fp > 0, "Flatpak available but no packages returned");
        }
        // Enrichment smoke: at least some packages should carry display names.
        let enriched = pkgs.iter().filter(|p| p.display_name.is_some()).count();
        println!(
            "scan_all: {} packages, enriched={}, avail apt={}/snap={}/flatpak={}/appimage={}/steam={}",
            pkgs.len(),
            enriched,
            avail.apt,
            avail.snap,
            avail.flatpak,
            avail.appimage,
            avail.steam
        );
        let _ = AppKind::Gui;
    }

    fn desktop_app(id: &str, name: &str, exec: &str) -> crate::domain::desktop_entries::DesktopApp {
        crate::domain::desktop_entries::DesktopApp {
            id: id.into(),
            file_path: std::path::PathBuf::from(format!("/usr/share/applications/{id}.desktop")),
            name: name.into(),
            comment: None,
            exec: exec.into(),
            executable: None,
            icon: None,
            categories: Vec::new(),
            terminal: false,
            menu_visible: true,
        }
    }

    #[test]
    fn enrich_gives_a_matching_package_its_desktop_display_name() {
        let index = DesktopIndex::from_apps(vec![desktop_app("firefox", "Firefox", "firefox %U")]);
        let mut pkg = InstalledPackage::new(PackageSource::Apt, "firefox");
        pkg.name = "firefox".into();
        enrich(&mut pkg, &index);
        assert_eq!(pkg.display_name.as_deref(), Some("Firefox"));
    }

    #[test]
    fn enrich_marks_a_matching_package_as_gui() {
        let index = DesktopIndex::from_apps(vec![desktop_app("firefox", "Firefox", "firefox %U")]);
        let mut pkg = InstalledPackage::new(PackageSource::Apt, "firefox");
        pkg.name = "firefox".into();
        enrich(&mut pkg, &index);
        assert_eq!(pkg.app_kind, AppKind::Gui);
    }

    #[test]
    fn enrich_preserves_steam_appinfo_classification() {
        let index = DesktopIndex::from_apps(vec![desktop_app(
            "steam-game",
            "SteamVR",
            "steam steam://rungameid/250820",
        )]);
        let mut pkg = InstalledPackage::new(PackageSource::Steam, "250820");
        pkg.name = "SteamVR".into();
        pkg.app_kind = AppKind::Unknown;
        enrich(&mut pkg, &index);
        assert_eq!(pkg.app_kind, AppKind::Unknown);
    }
    #[test]
    fn enrich_leaves_an_unmatched_package_alone() {
        let mut pkg = InstalledPackage::new(PackageSource::Apt, "htop");
        pkg.name = "htop".into();
        pkg.app_kind = AppKind::Cli;
        enrich(&mut pkg, &DesktopIndex::empty());
        assert_eq!(pkg.app_kind, AppKind::Cli);
    }
}
use tokio::task::JoinSet;

use crate::domain::desktop_entries::DesktopIndex;
use crate::domain::package::{AppKind, InstalledPackage, PackageSource};

/// A source-specific installed-package scanner.
pub trait Scanner: Send + Sync {
    /// Human source this scanner reports for.
    fn source(&self) -> PackageSource;

    /// True when the backing package manager appears installed on this system.
    fn is_available(&self) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;

    /// List installed packages for this source.
    fn scan(&self) -> Pin<Box<dyn Future<Output = Result<ScanReport>> + Send + '_>>;
}

use std::pin::Pin;

/// Result of one source's scan: the packages plus an optional non-fatal warning.
///
/// A warning records a partial failure (for example, one Flatpak scope failed
/// while the other succeeded). It is surfaced to the UI as a source warning
/// without discarding the packages that were found.
#[derive(Debug, Default)]
pub struct ScanReport {
    pub packages: Vec<InstalledPackage>,
    pub warning: Option<String>,
}

impl ScanReport {
    pub fn ok(packages: Vec<InstalledPackage>) -> Self {
        Self {
            packages,
            warning: None,
        }
    }
}

/// Build the full set of scanners (in a stable order for predictable results).
fn scanners() -> Vec<Box<dyn Scanner>> {
    vec![
        Box::new(apt::AptScanner),
        Box::new(snap::SnapScanner),
        Box::new(flatpak::FlatpakScanner),
        Box::new(appimage::AppImageScanner::new()),
        Box::new(steam::SteamScanner::new()),
    ]
}

/// One scanner's outcome.
struct ScanOutcome {
    source: PackageSource,
    available: bool,
    packages: Vec<InstalledPackage>,
    error: Option<String>,
}

/// Scan package sources in parallel, then enrich and append unmanaged desktop apps.
///
/// Returns the merged, sorted unified list plus per-source availability. Source
/// failures are never fatal: a broken/uninstalled source simply contributes zero
/// packages and reports `available = false`.
pub async fn scan_all() -> (Vec<InstalledPackage>, ScanAvailability) {
    // Discover desktop apps on a blocking thread (synchronous fs walk). Started
    // here, ahead of the package scans, so the walk overlaps them instead of
    // running serially in front of them.
    let desktop = tokio::task::spawn_blocking(|| {
        DesktopIndex::from_apps(crate::domain::desktop_entries::discover_desktop_apps())
    });

    let mut join = JoinSet::new();
    for scanner in scanners() {
        let source = scanner.source();
        join.spawn(async move {
            if scanner.is_available().await {
                match scanner.scan().await {
                    Ok(report) => ScanOutcome {
                        source,
                        available: true,
                        packages: report.packages,
                        error: report.warning,
                    },
                    Err(e) => ScanOutcome {
                        source,
                        available: true,
                        packages: Vec::new(),
                        error: Some(scan_error_message(&e)),
                    },
                }
            } else {
                ScanOutcome {
                    source,
                    available: false,
                    packages: Vec::new(),
                    error: None,
                }
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
            PackageSource::Steam => {
                availability.steam = outcome.available;
                availability.steam_error = outcome.error;
            }
            PackageSource::Desktop => {
                // Unmanaged desktop apps are synthesized from visible launchers
                // below, not reported by a package-manager scanner.
            }
        }
        merged.extend(outcome.packages);
    }

    // Enrich + classify + resolve icons, then sort apps-first, by display
    // name. Icon resolution touches the filesystem (theme lookups), so the
    // whole merge pass runs on a blocking thread to keep the async runtime
    // responsive. `DesktopIndex` and `InstalledPackage` are both `Send`.
    let desktop = desktop.await.unwrap_or_else(|_| DesktopIndex::empty());
    let (merged, availability) = tokio::task::spawn_blocking(move || {
        for pkg in merged.iter_mut() {
            enrich(pkg, &desktop);
        }
        let unmanaged_apps = desktop.unmanaged_user_apps(&merged);
        merged.extend(unmanaged_apps);
        // `sort_by_cached_key` computes the lowercase display name once per
        // package instead of once per comparison.
        merged.sort_by_cached_key(|p| {
            (
                kind_rank(p.app_kind),
                p.display_name.as_deref().unwrap_or(&p.name).to_lowercase(),
            )
        });
        (merged, availability)
    })
    .await
    .unwrap_or_else(|_| (Vec::new(), ScanAvailability::default()));

    (merged, availability)
}

fn kind_rank(k: AppKind) -> u8 {
    match k {
        AppKind::Game | AppKind::Gui => 0,
        AppKind::Cli => 1,
        AppKind::Unknown => 2,
    }
}

/// Turn a scanner failure into a short, source-specific availability message.
///
/// `capture_stdout` reports a typed [`SystemError`], so a timeout reads as a
/// timeout instead of an opaque command string. Any other error keeps its full
/// `anyhow` context.
fn scan_error_message(error: &anyhow::Error) -> String {
    match error.downcast_ref::<crate::domain::system::SystemError>() {
        Some(crate::domain::system::SystemError::Timeout { program, timeout }) => {
            format!("{program} timed out after {timeout:?}")
        }
        Some(crate::domain::system::SystemError::Spawn { program, .. }) => {
            format!("could not run {program}")
        }
        Some(crate::domain::system::SystemError::NonZero {
            program, exit_code, ..
        }) => {
            format!("{program} exited with {exit_code:?}")
        }
        None => error.to_string(),
    }
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
                if let Some(path) =
                    crate::domain::icons::resolve_for_app(name, app.executable.as_deref())
                {
                    pkg.icon = Some(crate::domain::icons::icon_url(&path));
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
        // Steam type classification comes from local binary appinfo metadata;
        // a launcher can identify a matching AppID but cannot distinguish a
        // game from SteamVR, Proton, or other tools.
        if pkg.source != PackageSource::Steam {
            pkg.app_kind = if app.terminal {
                AppKind::Cli
            } else {
                AppKind::Gui
            };
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
    #[serde(default)]
    pub steam: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub apt_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub snap_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub flatpak_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub steam_error: Option<String>,
    pub appimage_dirs: Vec<String>,
}
