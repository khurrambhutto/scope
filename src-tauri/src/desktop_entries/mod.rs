//! Linux `.desktop` entry discovery and parsing.
//!
//! This is the GUI enrichment layer: it produces human display names, icon
//! hints, categories, and launch metadata for installed packages that ship a
//! `.desktop` file. It is *not* the source of truth for the package list —
//! non-GUI packages remain in the unified list even when no entry matches.
//!
//! Architecture borrowed from the local `klauncher` reference
//! (`src-tauri/src/platform/linux/desktop_entries.rs`), adapted for Scope.

mod parser;

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub use parser::DesktopApp;

/// Directories whose `.desktop` files we ignore (locale, screensavers, etc.).
const BLACKLISTED_DIRS: &[&str] = &[
    "/usr/share/locale",
    "/usr/share/app-install",
    "/usr/share/kservices5",
    "/usr/share/kf5",
    "/usr/share/kservicetypes5",
    "/usr/share/applications/screensavers",
    "/usr/share/kde4",
    "/usr/share/mimelnk",
];

/// Discover all visible GUI apps from `.desktop` files under standard data dirs.
///
/// This is a synchronous filesystem walk; it is cheap enough that callers run
/// it on a blocking thread via `tokio::task::spawn_blocking`.
pub fn discover_desktop_apps() -> Vec<DesktopApp> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut apps = Vec::new();

    for directory in application_dirs() {
        if !directory.is_dir() {
            continue;
        }
        collect(&directory, &directory, &mut seen, &mut apps);
    }

    apps.sort_by_cached_key(|a| a.name.to_lowercase());
    apps
}

fn application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("applications"));
    } else if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }

    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .map(|value| {
            env::split_paths(&value)
                .map(|p| p.join("applications"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share/applications"),
                PathBuf::from("/usr/share/applications"),
            ]
        });
    dirs.extend(data_dirs);

    // Snap-installed apps expose desktop entries here.
    dirs.push(PathBuf::from("/var/lib/snapd/desktop/applications"));

    dirs
}

fn collect(root: &Path, dir: &Path, seen: &mut HashSet<String>, out: &mut Vec<DesktopApp>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if is_blacklisted(&path) {
                continue;
            }
            collect(root, &path, seen, out);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        if is_blacklisted(&path) {
            continue;
        }
        let Some(id) = desktop_id(root, &path) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(app) = parser::parse(&id, &path) {
            if app.no_display {
                // Hidden / not-for-menus entries are skipped (but still consumed
                // so a later visible variant does not shadow its display name).
                continue;
            }
            out.push(app);
        }
    }
}

fn desktop_id(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut id = String::new();
    for comp in rel.components() {
        if !id.is_empty() {
            id.push('-');
        }
        id.push_str(&comp.as_os_str().to_string_lossy());
    }
    Some(id.trim_end_matches(".desktop").to_string())
}

fn is_blacklisted(path: &Path) -> bool {
    let s = path.to_string_lossy();
    BLACKLISTED_DIRS.iter().any(|d| s.contains(d))
}

/// An index of discovered GUI apps for fast enrichment of package lists.
pub struct DesktopIndex {
    /// Indexed by normalized .desktop id (e.g. "org.gnome.Calculator").
    by_id: HashMap<String, DesktopApp>,
    /// Indexed by the executable basename (e.g. "firefox") parsed from `Exec=`.
    by_exec: HashMap<String, DesktopApp>,
    /// Lowercased display names for fuzzy fallback matching.
    by_name_lower: HashMap<String, DesktopApp>,
}

impl DesktopIndex {
    pub fn from_apps(apps: Vec<DesktopApp>) -> Self {
        let mut by_id = HashMap::new();
        let mut by_exec = HashMap::new();
        let mut by_name_lower = HashMap::new();
        for app in apps {
            if let Some(bin) = exec_binary(&app.exec) {
                by_exec
                    .entry(bin.to_lowercase())
                    .or_insert_with(|| app.clone());
            }
            by_name_lower
                .entry(app.name.to_lowercase())
                .or_insert_with(|| app.clone());
            by_id.insert(app.id.clone(), app);
        }
        Self {
            by_id,
            by_exec,
            by_name_lower,
        }
    }

    pub fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            by_exec: HashMap::new(),
            by_name_lower: HashMap::new(),
        }
    }

    /// Try to find a desktop app for a package given the source and id/name.
    pub fn lookup(
        &self,
        source: crate::package::PackageSource,
        package_id: &str,
        name: &str,
    ) -> Option<&DesktopApp> {
        match source {
            crate::package::PackageSource::Flatpak => self.by_id.get(package_id),
            crate::package::PackageSource::Snap => {
                // Snap desktop ids are often "<snap>_<app>.desktop" or "<app>.desktop".
                let needle = format!("{package_id}_");
                self.by_id
                    .iter()
                    .find(|(id, _)| id.starts_with(&needle) || id.as_str() == package_id)
                    .map(|(_, app)| app)
                    .or_else(|| self.by_id.get(package_id))
                    .or_else(|| self.by_exec.get(&package_id.to_lowercase()))
            }
            crate::package::PackageSource::Apt | crate::package::PackageSource::AppImage => {
                let lc = package_id.to_lowercase();
                self.by_id
                    .get(&lc)
                    .or_else(|| self.by_exec.get(&lc))
                    .or_else(|| self.by_name_lower.get(&name.to_lowercase()))
                    .or_else(|| self.lookup_stripped(package_id))
            }
        }
    }

    /// Last-resort APT/AppImage lookup that strips a known packaging suffix and
    /// retries (e.g. `helium-bin` → `helium`, `google-chrome-stable` →
    /// `google-chrome`). Only consulted after every exact key has failed, so
    /// packages that already match by id, `Exec=`, or `Name=` are unaffected.
    fn lookup_stripped(&self, package_id: &str) -> Option<&DesktopApp> {
        let lc = package_id.to_lowercase();
        for suffix in STRIPPABLE_SUFFIXES {
            let Some(base) = lc.strip_suffix(suffix) else {
                continue;
            };
            let Some(base) = base.strip_suffix('-') else {
                continue;
            };
            if base.is_empty() {
                continue;
            }
            if let Some(app) = self
                .by_id
                .get(base)
                .or_else(|| self.by_exec.get(base))
                .or_else(|| self.by_name_lower.get(base))
            {
                return Some(app);
            }
        }
        None
    }
}

/// Packaging suffixes that commonly separate an APT package name from the app
/// it ships. Stripped only as a fallback during [`DesktopIndex::lookup`].
const STRIPPABLE_SUFFIXES: &[&str] = &["bin", "desktop", "stable", "browser"];

/// Extract the executable basename from a `.desktop` `Exec=` value.
fn exec_binary(exec: &str) -> Option<String> {
    // Strip leading env assignments (e.g. "env VAR=1 foo --bar").
    let mut rest = exec.trim();
    while let Some(stripped) = rest.strip_prefix("env ") {
        rest = stripped;
        while let Some(space) = rest.find(' ') {
            let token = &rest[..space];
            if token.contains('=') {
                rest = rest[space..].trim_start();
                continue;
            }
            break;
        }
    }
    let first = rest.split_whitespace().next()?;
    let path = Path::new(first);
    Some(path.file_name()?.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::PackageSource;

    fn app(id: &str, name: &str, exec: &str) -> DesktopApp {
        DesktopApp {
            id: id.to_string(),
            name: name.to_string(),
            comment: None,
            exec: exec.to_string(),
            icon: Some(id.to_string()),
            categories: Vec::new(),
            terminal: false,
            no_display: false,
        }
    }

    #[test]
    fn apt_exact_matches_are_unchanged() {
        let index = DesktopIndex::from_apps(vec![app("firefox", "Firefox", "firefox %u")]);
        let found = index.lookup(PackageSource::Apt, "firefox", "firefox");
        assert_eq!(found.map(|a| a.id.as_str()), Some("firefox"));
    }

    #[test]
    fn apt_strips_bin_suffix_to_find_app() {
        // `helium-bin` ships `/usr/share/applications/helium.desktop`.
        let index = DesktopIndex::from_apps(vec![app("helium", "Helium", "helium %U")]);
        let found = index.lookup(PackageSource::Apt, "helium-bin", "helium-bin");
        assert_eq!(found.map(|a| a.id.as_str()), Some("helium"));
    }

    #[test]
    fn apt_strips_stable_suffix_to_find_app() {
        let index =
            DesktopIndex::from_apps(vec![app("google-chrome", "Google Chrome", "google-chrome-stable %U")]);
        let found = index.lookup(PackageSource::Apt, "google-chrome-stable", "google-chrome-stable");
        assert_eq!(found.map(|a| a.id.as_str()), Some("google-chrome"));
    }

    #[test]
    fn apt_exact_id_wins_over_stripped_candidate() {
        let index = DesktopIndex::from_apps(vec![
            app("foo", "Foo", "foo"),
            app("foo-bin", "Foo Bin", "foo-bin"),
        ]);
        let found = index.lookup(PackageSource::Apt, "foo-bin", "foo-bin");
        assert_eq!(found.map(|a| a.id.as_str()), Some("foo-bin"));
    }

    #[test]
    fn apt_unrelated_package_stays_unmatched() {
        let index = DesktopIndex::from_apps(vec![app("helium", "Helium", "helium %U")]);
        assert!(index
            .lookup(PackageSource::Apt, "ncurses-bin", "ncurses-bin")
            .is_none());
    }
}
