//! Opt-in removal of an integrated AppImage's own data.
//!
//! AppImages do not record where they keep settings, caches, or saved files,
//! so Scope only proposes folders whose name exactly matches the app (its file
//! name, launcher names, `Name`, `X-AppImage-Name`, `StartupWMClass`, or icon
//! name) directly inside the XDG config, data, cache, and state directories,
//! plus icons the app installed for itself. A name that any other launcher on
//! the system also uses is dropped, so a shared folder such as `~/.config/Code`
//! is never proposed for an AppImage of the same app. Everything found is
//! shown in the uninstall preview and moved to Trash only when the user opts
//! in; apply removes only paths that are still found right before removal.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::desktop::{unescape, DesktopFile};
use super::{AppImageInstallation, Roots};

/// One folder or icon attributed to an integrated AppImage by name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppData {
    pub path: PathBuf,
    pub kind: AppDataKind,
    /// Allocated bytes when a bounded measurement finished.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppDataKind {
    Config,
    Data,
    Cache,
    State,
    Icon,
}

impl AppDataKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Config => "Settings",
            Self::Data => "App data",
            Self::Cache => "Cache",
            Self::State => "State",
            Self::Icon => "Icon",
        }
    }
}

/// The per-user XDG base directories searched for app data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bases {
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
}

impl Bases {
    pub fn current(roots: &Roots) -> Self {
        let xdg = |var: &str, fallback: &str| {
            std::env::var_os(var)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .unwrap_or_else(|| roots.home.join(fallback))
        };
        Self {
            config: xdg("XDG_CONFIG_HOME", ".config"),
            data: xdg("XDG_DATA_HOME", ".local/share"),
            cache: xdg("XDG_CACHE_HOME", ".cache"),
            state: xdg("XDG_STATE_HOME", ".local/state"),
        }
    }
}

/// Names another launcher on the system answers to, lowercased.
#[derive(Debug, Clone, Default)]
pub struct OtherLaunchers {
    names: HashSet<String>,
    icon_paths: HashSet<PathBuf>,
}

impl OtherLaunchers {
    /// Every visible launcher except the ones that run this AppImage.
    pub fn current(installation: &AppImageInstallation) -> Self {
        let mut others = Self::default();
        for app in crate::domain::desktop_entries::discover_desktop_apps() {
            if installation.launchers.contains(&app.file_path) {
                continue;
            }
            others.add(
                &app.id,
                &app.name,
                app.executable.as_deref(),
                app.icon.as_deref(),
            );
        }
        others
    }

    pub fn add(&mut self, id: &str, name: &str, executable: Option<&Path>, icon: Option<&str>) {
        let program = executable.and_then(|path| path.file_name()?.to_str());
        for value in [Some(id), Some(name), program].into_iter().flatten() {
            self.names.extend(variants(value));
        }
        match icon.map(Path::new) {
            Some(path) if path.is_absolute() => {
                self.icon_paths.insert(path.to_path_buf());
            }
            Some(_) => self.names.extend(variants(icon.unwrap_or_default())),
            None => {}
        }
    }

    fn claims(&self, name: &str) -> bool {
        self.names.contains(&name.to_lowercase())
    }
}

/// Folders and generic names that are never one app's own data.
const SHARED: &[&str] = &[
    "applications",
    "appimages",
    "autostart",
    "backgrounds",
    "chromium",
    "dconf",
    "desktop-directories",
    "electron",
    "flatpak",
    "fontconfig",
    "fonts",
    "gnome-shell",
    "google-chrome",
    "gtk-2.0",
    "gtk-3.0",
    "gtk-4.0",
    "icons",
    "keyrings",
    "menus",
    "mesa_shader_cache",
    "mime",
    "nautilus",
    "nvidia",
    "pulse",
    "recently-used.xbel",
    "scope",
    "snap",
    "sounds",
    "systemd",
    "themes",
    "thumbnails",
    "trash",
    "user-dirs.dirs",
    "wireplumber",
];

/// Find app data for `installation`, measuring each folder within a short bound.
pub fn find(
    roots: &Roots,
    bases: &Bases,
    installation: &AppImageInstallation,
    others: &OtherLaunchers,
) -> Vec<AppData> {
    let launchers: Vec<DesktopFile> = installation
        .launchers
        .iter()
        .filter_map(|path| DesktopFile::read(path).ok())
        .collect();
    let names = candidate_names(installation, &launchers, others);
    if names.is_empty() {
        return Vec::new();
    }
    let Ok(owner) = std::fs::metadata(&roots.home).map(|m| m.uid()) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (base, kind) in [
        (&bases.config, AppDataKind::Config),
        (&bases.data, AppDataKind::Data),
        (&bases.cache, AppDataKind::Cache),
        (&bases.state, AppDataKind::State),
    ] {
        let Ok(entries) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_lowercase) else {
                continue;
            };
            let path = entry.path();
            if names.contains(&name) && owned(&path, owner) {
                found.push(AppData {
                    bytes: measure(&path),
                    path,
                    kind,
                });
            }
        }
    }
    for path in icons(roots, bases, installation, &launchers, &names, others) {
        if owned(&path, owner) && !found.iter().any(|data| data.path == path) {
            found.push(AppData {
                bytes: measure(&path),
                path,
                kind: AppDataKind::Icon,
            });
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found
}

/// Keep the planned entries that a fresh search still finds unchanged.
pub fn still_found(planned: &[AppData], fresh: &[AppData]) -> Vec<AppData> {
    planned
        .iter()
        .filter(|data| {
            fresh
                .iter()
                .any(|now| now.path == data.path && now.kind == data.kind)
        })
        .cloned()
        .collect()
}

/// Lowercased names this app answers to that no other launcher claims.
fn candidate_names(
    installation: &AppImageInstallation,
    launchers: &[DesktopFile],
    others: &OtherLaunchers,
) -> HashSet<String> {
    let mut raw: Vec<String> = Vec::new();
    let stem = |path: &Path| path.file_stem().map(|s| s.to_string_lossy().into_owned());
    raw.extend(stem(&installation.path));
    raw.extend(installation.launchers.iter().filter_map(|p| stem(p)));
    for file in launchers {
        for key in ["Name", "X-AppImage-Name", "StartupWMClass"] {
            raw.extend(file.entry(key).map(unescape));
        }
        if let Some(icon) = file.entry("Icon").map(unescape) {
            if !icon.contains('/') {
                raw.push(icon);
            }
        }
    }
    raw.iter()
        .flat_map(|value| variants(value))
        .filter(|name| {
            name.chars().count() >= 3
                && !name.starts_with('.')
                && !name.contains('/')
                && !SHARED.contains(&name.as_str())
                && !others.claims(name)
        })
        .collect()
}

/// `Open Research` also matches `open-research`, `open_research`, and `openresearch`.
fn variants(value: &str) -> Vec<String> {
    let value = value.trim().to_lowercase();
    if value.is_empty() {
        return Vec::new();
    }
    let mut out = vec![value.clone()];
    if value.contains(' ') {
        out.push(value.replace(' ', "-"));
        out.push(value.replace(' ', "_"));
        out.push(value.replace(' ', ""));
    }
    out
}

/// Icons the app installed for itself in the user's icon theme.
fn icons(
    roots: &Roots,
    bases: &Bases,
    installation: &AppImageInstallation,
    launchers: &[DesktopFile],
    names: &HashSet<String>,
    others: &OtherLaunchers,
) -> Vec<PathBuf> {
    let theme = bases.data.join("icons");
    let mut found = Vec::new();
    // Absolute `Icon=` paths written by an app's own desktop integration.
    for file in launchers {
        let Some(icon) = file.entry("Icon").map(|v| PathBuf::from(unescape(v))) else {
            continue;
        };
        if icon.is_absolute()
            && icon.starts_with(&theme)
            && !icon.starts_with(roots.icons())
            && !others.icon_paths.contains(&icon)
            && is_plain_file(&icon)
        {
            found.push(icon);
        }
    }
    // `hicolor/<size>/apps/<name>.<ext>` for the app's own names.
    let hicolor = theme.join("hicolor");
    if let Ok(sizes) = std::fs::read_dir(&hicolor) {
        for size in sizes.flatten() {
            let Ok(files) = std::fs::read_dir(size.path().join("apps")) else {
                continue;
            };
            for file in files.flatten() {
                let path = file.path();
                let matches = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| names.contains(&s.to_lowercase()))
                    && path
                        .extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|e| matches!(e, "png" | "svg" | "svgz" | "xpm"));
                if matches && !others.icon_paths.contains(&path) && is_plain_file(&path) {
                    found.push(path);
                }
            }
        }
    }
    found.retain(|path| !installation.icons.contains(path));
    found.sort();
    found.dedup();
    found
}

fn is_plain_file(path: &Path) -> bool {
    path.symlink_metadata()
        .is_ok_and(|m| m.file_type().is_file())
}

/// A regular file or real directory owned by the user; never a symlink.
fn owned(path: &Path, owner: u32) -> bool {
    path.symlink_metadata().is_ok_and(|m| {
        let kind = m.file_type();
        (kind.is_dir() || kind.is_file()) && m.uid() == owner
    })
}

fn measure(path: &Path) -> Option<u64> {
    if is_plain_file(path) {
        return path.symlink_metadata().ok().map(|m| m.len());
    }
    let measurement = crate::domain::package_details::measurement::measure(
        path,
        &std::sync::atomic::AtomicBool::new(false),
    );
    measurement.bytes.filter(|_| measurement.complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        roots: Roots,
        bases: Bases,
        installation: AppImageInstallation,
    }

    fn fixture(launcher: &str) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().to_path_buf();
        let roots = Roots::new(home.clone(), home.join(".local/share/applications"));
        let bases = Bases {
            config: home.join(".config"),
            data: home.join(".local/share"),
            cache: home.join(".cache"),
            state: home.join(".local/state"),
        };
        for dir in [
            &bases.config,
            &bases.cache,
            &bases.state,
            &roots.applications,
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        std::fs::create_dir_all(roots.icons()).unwrap();
        let path = roots.appimages.join("openresearch.appimage");
        std::fs::write(&path, b"").unwrap();
        let desktop = roots.applications.join("openresearch.desktop");
        std::fs::write(&desktop, launcher).unwrap();
        let installation = AppImageInstallation {
            path,
            launcher: desktop.clone(),
            launchers: vec![desktop],
            icons: vec![],
            size: 0,
            modified_ns: 0,
            update_info: None,
            update: None,
        };
        Fixture {
            _dir: dir,
            roots,
            bases,
            installation,
        }
    }

    const LAUNCHER: &str = "[Desktop Entry]\nType=Application\nName=OpenResearch\n\
        Exec=/x/openresearch.appimage\nStartupWMClass=Open Research\n";

    #[test]
    fn finds_folders_named_after_the_app_in_every_base() {
        let f = fixture(LAUNCHER);
        let wanted = [
            f.bases.config.join("openresearch"),
            f.bases.data.join("OpenResearch"),
            f.bases.cache.join("open-research"),
            f.bases.state.join("openresearch"),
        ];
        for dir in &wanted {
            std::fs::create_dir_all(dir.join("inner")).unwrap();
            std::fs::write(dir.join("inner/file"), b"12345").unwrap();
        }
        std::fs::create_dir_all(f.bases.config.join("openresearch-helper")).unwrap();
        std::fs::create_dir_all(f.bases.config.join("other")).unwrap();

        let found = find(
            &f.roots,
            &f.bases,
            &f.installation,
            &OtherLaunchers::default(),
        );
        let paths: HashSet<_> = found.iter().map(|d| d.path.clone()).collect();
        assert_eq!(paths, wanted.iter().cloned().collect());
        assert!(found.iter().all(|d| d.bytes.is_some()));
    }

    #[test]
    fn names_claimed_by_another_launcher_are_never_proposed() {
        let f = fixture(
            "[Desktop Entry]\nType=Application\nName=Code\nExec=/x/code.appimage\nIcon=code\n",
        );
        std::fs::create_dir_all(f.bases.config.join("Code")).unwrap();
        let mut others = OtherLaunchers::default();
        others.add(
            "code",
            "Visual Studio Code",
            Some(Path::new("/usr/bin/code")),
            Some("vscode"),
        );
        // The AppImage's own file name stem is "openresearch"; only "Code" is shared.
        let found = find(&f.roots, &f.bases, &f.installation, &others);
        assert!(found.iter().all(|d| !d.path.ends_with("Code")));
    }

    #[test]
    fn symlinks_and_shared_folders_are_skipped() {
        let f = fixture(
            "[Desktop Entry]\nType=Application\nName=Icons\nExec=/x/openresearch.appimage\n",
        );
        std::fs::create_dir_all(f.bases.data.join("icons")).unwrap();
        let elsewhere = f.roots.home.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, f.bases.config.join("openresearch")).unwrap();
        let found = find(
            &f.roots,
            &f.bases,
            &f.installation,
            &OtherLaunchers::default(),
        );
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn finds_icons_the_app_installed_for_itself() {
        let f = fixture(LAUNCHER);
        let theme = f.bases.data.join("icons/hicolor/256x256/apps");
        std::fs::create_dir_all(&theme).unwrap();
        std::fs::write(theme.join("openresearch.png"), b"png").unwrap();
        std::fs::write(theme.join("unrelated.png"), b"png").unwrap();
        let absolute = f.bases.data.join("icons/hicolor/48x48/apps/custom.svg");
        std::fs::create_dir_all(absolute.parent().unwrap()).unwrap();
        std::fs::write(&absolute, b"svg").unwrap();
        std::fs::write(
            &f.installation.launcher,
            format!("{LAUNCHER}Icon={}\n", absolute.display()),
        )
        .unwrap();

        let found = find(
            &f.roots,
            &f.bases,
            &f.installation,
            &OtherLaunchers::default(),
        );
        let icons: Vec<_> = found
            .iter()
            .filter(|d| d.kind == AppDataKind::Icon)
            .map(|d| d.path.clone())
            .collect();
        assert_eq!(icons, vec![theme.join("openresearch.png"), absolute]);

        let mut others = OtherLaunchers::default();
        others.add("other", "Other", None, Some("openresearch"));
        let found = find(&f.roots, &f.bases, &f.installation, &others);
        assert!(!found.iter().any(|d| d.path.ends_with("openresearch.png")));
    }

    #[test]
    fn apply_keeps_only_entries_a_fresh_search_still_finds() {
        let data = |p: &str| AppData {
            path: PathBuf::from(p),
            kind: AppDataKind::Config,
            bytes: None,
        };
        let planned = vec![data("/h/.config/a"), data("/h/.config/b")];
        let fresh = vec![data("/h/.config/b"), data("/h/.config/c")];
        assert_eq!(still_found(&planned, &fresh), vec![data("/h/.config/b")]);
    }
}
