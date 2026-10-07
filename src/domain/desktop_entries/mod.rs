//! Linux `.desktop` entry discovery and parsing.
//!
//! This enriches managed packages with display names, icons, categories, and
//! launch metadata. It also finds unmatched launchers in the user's application
//! directory so locally installed GUI/CLI apps can be listed as unmanaged.
//!
//! Architecture borrowed from the local `klauncher` reference
//! (`src-tauri/src/platform/linux/desktop_entries.rs`), adapted for Scope.

mod parser;

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

    if let Some(user_applications) = user_application_dir() {
        dirs.push(user_applications);
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

fn user_application_dir() -> Option<PathBuf> {
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        Some(PathBuf::from(data_home).join("applications"))
    } else {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share/applications"))
    }
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
            if !app.menu_visible {
                // Hidden / not-for-this-desktop entries are consumed so a lower
                // precedence duplicate cannot shadow the desktop's decision.
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

/// Index of visible desktop apps for package enrichment and local-app discovery.
pub struct DesktopIndex {
    /// Indexed by normalized .desktop id (e.g. "org.gnome.Calculator").
    by_id: HashMap<String, Arc<DesktopApp>>,
    /// Indexed by the executable basename (e.g. "firefox") parsed from `Exec=`.
    by_exec: HashMap<String, Arc<DesktopApp>>,
    /// Lowercased display names for fuzzy fallback matching.
    by_name_lower: HashMap<String, Arc<DesktopApp>>,
    /// Snap desktop ids are `<snap>_<app>`; keyed by the leading segment so Snap
    /// lookups are O(1) instead of a linear scan of every entry.
    by_snap_prefix: HashMap<String, Arc<DesktopApp>>,
    by_path: HashMap<PathBuf, Arc<DesktopApp>>,
    apps: Vec<Arc<DesktopApp>>,
}

impl DesktopIndex {
    pub fn from_apps(apps: Vec<DesktopApp>) -> Self {
        let mut by_id = HashMap::new();
        let mut by_exec = HashMap::new();
        let mut by_name_lower = HashMap::new();
        let mut by_snap_prefix = HashMap::new();
        let mut by_path = HashMap::new();
        let mut indexed_apps = Vec::new();
        for app in apps {
            // Share one app allocation across every lookup index.
            let app = Arc::new(app);
            if let Some(bin) = exec_binary(&app.exec) {
                by_exec
                    .entry(bin.to_lowercase())
                    .or_insert_with(|| Arc::clone(&app));
            }
            by_name_lower
                .entry(app.name.to_lowercase())
                .or_insert_with(|| Arc::clone(&app));
            if let Some(prefix) = app.id.split('_').next() {
                by_snap_prefix
                    .entry(prefix.to_string())
                    .or_insert_with(|| Arc::clone(&app));
            }
            by_path.insert(app.file_path.clone(), Arc::clone(&app));
            by_id.insert(app.id.clone(), Arc::clone(&app));
            indexed_apps.push(app);
        }
        Self {
            by_id,
            by_exec,
            by_name_lower,
            by_snap_prefix,
            by_path,
            apps: indexed_apps,
        }
    }

    pub fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            by_exec: HashMap::new(),
            by_name_lower: HashMap::new(),
            by_snap_prefix: HashMap::new(),
            by_path: HashMap::new(),
            apps: Vec::new(),
        }
    }

    /// Try to find a desktop app for a package given the source and id/name.
    pub fn lookup(
        &self,
        source: crate::domain::package::PackageSource,
        package_id: &str,
        name: &str,
    ) -> Option<&DesktopApp> {
        match source {
            crate::domain::package::PackageSource::Flatpak => {
                self.by_id.get(package_id).map(Arc::as_ref)
            }
            crate::domain::package::PackageSource::Snap => self
                .by_snap_prefix
                .get(package_id)
                .or_else(|| self.by_id.get(package_id))
                .or_else(|| self.by_exec.get(&package_id.to_lowercase()))
                .map(Arc::as_ref),
            crate::domain::package::PackageSource::Apt => self.lookup_apt(package_id, name),
            crate::domain::package::PackageSource::AppImage => {
                let lc = package_id.to_lowercase();
                let appimage_executable = Path::new(package_id)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_lowercase());
                self.by_id
                    .get(&lc)
                    .or_else(|| self.by_exec.get(&lc))
                    .or_else(|| {
                        appimage_executable
                            .as_ref()
                            .and_then(|name| self.by_exec.get(name))
                    })
                    .or_else(|| self.by_name_lower.get(&name.to_lowercase()))
                    .map(Arc::as_ref)
                    .or_else(|| self.lookup_stripped(package_id))
            }
            crate::domain::package::PackageSource::Desktop => {
                self.by_path.get(Path::new(package_id)).map(Arc::as_ref)
            }
        }
    }

    fn lookup_apt(&self, package_id: &str, name: &str) -> Option<&DesktopApp> {
        let lc = package_id.to_lowercase();
        if let Some(app) = self.by_id.get(&lc).or_else(|| self.by_exec.get(&lc)) {
            return Some(app.as_ref());
        }
        let owned_files = crate::domain::package_files::list(package_id);
        self.lookup_apt_fallback(package_id, name, &owned_files)
    }

    fn lookup_apt_fallback(
        &self,
        package_id: &str,
        name: &str,
        owned_files: &[PathBuf],
    ) -> Option<&DesktopApp> {
        self.lookup_owned_desktop_files(owned_files)
            .or_else(|| {
                self.by_name_lower
                    .get(&name.to_lowercase())
                    .map(Arc::as_ref)
            })
            .or_else(|| self.lookup_stripped(package_id))
    }

    fn lookup_owned_desktop_files(&self, files: &[PathBuf]) -> Option<&DesktopApp> {
        files
            .iter()
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("desktop"))
            .find_map(|path| self.by_path.get(path).map(Arc::as_ref))
    }

    /// Visible desktop launchers in the user's applications directory that no
    /// installed package scanner identified. These are shown as unmanaged apps,
    /// so Scope does not offer an uninstall action for them.
    pub fn unmanaged_user_apps(
        &self,
        packages: &[crate::domain::package::InstalledPackage],
    ) -> Vec<crate::domain::package::InstalledPackage> {
        let Some(user_applications) = user_application_dir() else {
            return Vec::new();
        };
        self.unmanaged_apps_in(packages, &user_applications)
    }

    fn unmanaged_apps_in(
        &self,
        packages: &[crate::domain::package::InstalledPackage],
        user_applications: &Path,
    ) -> Vec<crate::domain::package::InstalledPackage> {
        let matched: HashSet<&str> = packages
            .iter()
            .filter_map(|pkg| self.lookup(pkg.source, &pkg.package_id, &pkg.name))
            .map(|app| app.id.as_str())
            .collect();

        self.apps
            .iter()
            .filter(|app| app.file_path.starts_with(user_applications))
            .filter(|app| !matched.contains(app.id.as_str()))
            .map(|app| {
                use crate::domain::package::{AppKind, InstalledPackage, PackageSource};

                let mut pkg = InstalledPackage::new(
                    PackageSource::Desktop,
                    app.file_path.to_string_lossy().to_string(),
                );
                pkg.name = app.name.clone();
                pkg.display_name = Some(app.name.clone());
                pkg.description = app.comment.clone();
                pkg.categories = (!app.categories.is_empty()).then(|| app.categories.join(", "));
                pkg.terminal = app.terminal;
                pkg.app_kind = if app.terminal {
                    AppKind::Cli
                } else {
                    AppKind::Gui
                };
                if let Some(icon) = app.icon.as_deref() {
                    if let Some(path) =
                        crate::domain::icons::resolve_for_app(icon, app.executable.as_deref())
                    {
                        pkg.icon = Some(crate::domain::icons::icon_url(&path));
                    }
                }
                pkg
            })
            .collect()
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
                return Some(app.as_ref());
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
    let first = if let Some(quoted) = rest.strip_prefix('"') {
        quoted.split_once('"').map(|(path, _)| path)?
    } else {
        rest.split_whitespace().next()?
    };
    let path = Path::new(first);
    Some(path.file_name()?.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package::PackageSource;

    fn app(id: &str, name: &str, exec: &str) -> DesktopApp {
        DesktopApp {
            id: id.to_string(),
            file_path: PathBuf::from(format!("/usr/share/applications/{id}.desktop")),
            name: name.to_string(),
            comment: None,
            exec: exec.to_string(),
            executable: None,
            icon: Some(id.to_string()),
            categories: Vec::new(),
            terminal: false,
            menu_visible: true,
        }
    }

    #[test]
    fn apt_exact_matches_are_unchanged() {
        let index = DesktopIndex::from_apps(vec![app("firefox", "Firefox", "firefox %u")]);
        let found = index.lookup(PackageSource::Apt, "firefox", "firefox");
        assert_eq!(found.map(|a| a.id.as_str()), Some("firefox"));
    }

    #[test]
    fn apt_package_uses_its_owned_desktop_file_when_names_differ() {
        let mut steam = app("steam", "Steam", "/usr/bin/steam %U");
        steam.file_path = PathBuf::from("/usr/share/applications/steam.desktop");
        let index = DesktopIndex::from_apps(vec![steam]);
        let files = vec![PathBuf::from("/usr/share/applications/steam.desktop")];

        let found = index.lookup_apt_fallback("steam-launcher", "steam-launcher", &files);

        assert_eq!(found.map(|app| app.name.as_str()), Some("Steam"));
    }

    #[test]
    fn unmatched_user_desktop_launchers_are_listed_as_unmanaged_apps() {
        let mut zed = app("dev.zed.Zed", "Zed", "/home/user/.local/zed.app/bin/zed %U");
        zed.file_path = PathBuf::from("/home/user/.local/share/applications/dev.zed.Zed.desktop");
        let index = DesktopIndex::from_apps(vec![zed]);

        let apps = index.unmanaged_apps_in(&[], Path::new("/home/user/.local/share/applications"));

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].source, PackageSource::Desktop);
        assert_eq!(apps[0].display_name.as_deref(), Some("Zed"));
        assert!(!crate::domain::listing::can_uninstall(&apps[0]));
    }

    #[test]
    fn visible_user_launcher_with_an_existing_executable_is_imported() {
        let root =
            std::env::temp_dir().join(format!("scope-local-launcher-{}", std::process::id()));
        let user_applications = root.join("home/.local/share/applications");
        let executable = root.join("home/.local/opt/editor/bin/editor");
        let desktop_file = user_applications.join("com.example.Editor.desktop");
        fs::create_dir_all(&user_applications).unwrap();
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "test binary").unwrap();
        fs::write(
            &desktop_file,
            format!(
                "[Desktop Entry]\nType=Application\nName=Example Editor\nTryExec={}\nExec={} %U\nTerminal=false\n",
                executable.display(),
                executable.display()
            ),
        )
        .unwrap();

        let launcher = parser::parse("com.example.Editor", &desktop_file).unwrap();
        assert!(launcher.menu_visible);
        let index = DesktopIndex::from_apps(vec![launcher]);
        let apps = index.unmanaged_apps_in(&[], &user_applications);

        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].display_name.as_deref(), Some("Example Editor"));
        assert_eq!(apps[0].source, PackageSource::Desktop);
        assert!(!crate::domain::listing::can_uninstall(&apps[0]));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn appimage_matches_desktop_launcher_by_executable_basename() {
        let index = DesktopIndex::from_apps(vec![app(
            "t3_code_nightly",
            "T3 Code (Nightly)",
            "env DESKTOPINTEGRATION=1 /home/user/AppImages/t3_code_nightly.appimage --no-sandbox %U",
        )]);
        let found = index.lookup(
            PackageSource::AppImage,
            "/home/user/AppImages/t3_code_nightly.appimage",
            "t3_code_nightly",
        );
        assert_eq!(found.map(|a| a.name.as_str()), Some("T3 Code (Nightly)"));
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
        let index = DesktopIndex::from_apps(vec![app(
            "google-chrome",
            "Google Chrome",
            "google-chrome-stable %U",
        )]);
        let found = index.lookup(
            PackageSource::Apt,
            "google-chrome-stable",
            "google-chrome-stable",
        );
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
