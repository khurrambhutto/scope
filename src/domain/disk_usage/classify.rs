//! Pure directory classification for the disk usage tree.
//!
//! Children inherit their parent's category and rebuildable flag unless a rule
//! matches. Nothing here touches the filesystem except [`XdgDirs::current`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Coarse category shown for a directory or file in the disk view.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
///
/// Labels follow the names Linux already uses: the XDG user folders
/// (Documents, Downloads, Pictures, Videos, Music) plus the buckets common to
/// desktop storage screens (Apps, Developer, Cache, Trash, System, Other).
pub enum DiskCategory {
    Apps,
    Documents,
    Downloads,
    Pictures,
    Videos,
    Music,
    Developer,
    Cache,
    Trash,
    System,
    Other,
}

impl DiskCategory {
    /// Every category, in display order.
    pub const ALL: [DiskCategory; 11] = [
        DiskCategory::Apps,
        DiskCategory::Documents,
        DiskCategory::Downloads,
        DiskCategory::Pictures,
        DiskCategory::Videos,
        DiskCategory::Music,
        DiskCategory::Developer,
        DiskCategory::Cache,
        DiskCategory::Trash,
        DiskCategory::System,
        DiskCategory::Other,
    ];

    /// Short human label.
    pub fn label(self) -> &'static str {
        match self {
            DiskCategory::Apps => "Apps",
            DiskCategory::Documents => "Documents",
            DiskCategory::Downloads => "Downloads",
            DiskCategory::Pictures => "Pictures",
            DiskCategory::Videos => "Videos",
            DiskCategory::Music => "Music",
            DiskCategory::Developer => "Developer",
            DiskCategory::Cache => "Cache",
            DiskCategory::Trash => "Trash",
            DiskCategory::System => "System",
            DiskCategory::Other => "Other",
        }
    }
}

/// Resolved XDG locations used by the path rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct XdgDirs {
    pub cache: PathBuf,
    pub pictures: PathBuf,
    pub videos: PathBuf,
    pub music: PathBuf,
    pub download: PathBuf,
    pub documents: PathBuf,
    pub desktop: PathBuf,
}

const USER_DIR_KEYS: [(&str, &str); 6] = [
    ("XDG_PICTURES_DIR", "Pictures"),
    ("XDG_VIDEOS_DIR", "Videos"),
    ("XDG_MUSIC_DIR", "Music"),
    ("XDG_DOWNLOAD_DIR", "Downloads"),
    ("XDG_DOCUMENTS_DIR", "Documents"),
    ("XDG_DESKTOP_DIR", "Desktop"),
];

impl XdgDirs {
    /// Resolves the XDG directories from the environment and `~/.config/user-dirs.dirs`.
    pub fn current(home: &Path) -> Self {
        let cache = absolute_env("XDG_CACHE_HOME").unwrap_or_else(|| home.join(".cache"));
        let config = absolute_env("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        let user_dirs = std::fs::read_to_string(config.join("user-dirs.dirs")).ok();
        Self::from_user_dirs(home, cache, user_dirs.as_deref())
    }

    /// Builds the directories from an explicit cache root and optional `user-dirs.dirs` text.
    pub fn from_user_dirs(home: &Path, cache: PathBuf, user_dirs: Option<&str>) -> Self {
        let parsed = user_dirs
            .map(|text| parse_user_dirs(text, home))
            .unwrap_or_default();
        let resolve = |key: &str, fallback: &str| {
            parsed
                .get(key)
                .cloned()
                .unwrap_or_else(|| home.join(fallback))
        };
        Self {
            cache,
            pictures: resolve(USER_DIR_KEYS[0].0, USER_DIR_KEYS[0].1),
            videos: resolve(USER_DIR_KEYS[1].0, USER_DIR_KEYS[1].1),
            music: resolve(USER_DIR_KEYS[2].0, USER_DIR_KEYS[2].1),
            download: resolve(USER_DIR_KEYS[3].0, USER_DIR_KEYS[3].1),
            documents: resolve(USER_DIR_KEYS[4].0, USER_DIR_KEYS[4].1),
            desktop: resolve(USER_DIR_KEYS[5].0, USER_DIR_KEYS[5].1),
        }
    }
}

fn absolute_env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

/// Parses `XDG_*_DIR="..."` lines, expanding `$HOME` and `${HOME}`.
///
/// Entries that are not absolute after expansion (including the disabled
/// `$HOME`-only form) are dropped so the caller falls back to the default.
pub fn parse_user_dirs(text: &str, home: &Path) -> HashMap<String, PathBuf> {
    let home_text = home.to_string_lossy();
    let mut dirs = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !USER_DIR_KEYS.iter().any(|(known, _)| *known == key) {
            continue;
        }
        let value = value.trim().trim_matches('"');
        let expanded = value
            .replace("${HOME}", &home_text)
            .replace("$HOME", &home_text);
        let path = PathBuf::from(expanded);
        if path.is_absolute() && path != home {
            dirs.insert(key.to_string(), path);
        }
    }
    dirs
}

/// Classifies one directory from its parent's category, its path and name.
///
/// Returns the category, whether the directory is rebuildable, and the owning
/// app or game name when a rule identifies one. Paths outside `home` are
/// always [`DiskCategory::System`].
pub fn classify_dir(
    home: &Path,
    xdg: &XdgDirs,
    parent: (DiskCategory, bool),
    path: &Path,
    name: &str,
    has_cachedir_tag: bool,
) -> (DiskCategory, bool, Option<String>) {
    let (inherited, inherited_rebuildable) = parent;
    if !path.starts_with(home) {
        return (DiskCategory::System, false, None);
    }
    if let Some(found) = path_rule(home, xdg, path, name) {
        return found;
    }
    if name == "node_modules" || name == ".venv" {
        return (DiskCategory::Developer, true, None);
    }
    if name == "target" && has_cachedir_tag {
        return (DiskCategory::Developer, true, None);
    }
    if has_cachedir_tag {
        let category = if inherited == DiskCategory::Other {
            DiskCategory::Cache
        } else {
            inherited
        };
        return (category, true, None);
    }
    (inherited, inherited_rebuildable, None)
}

fn path_rule(
    home: &Path,
    xdg: &XdgDirs,
    path: &Path,
    name: &str,
) -> Option<(DiskCategory, bool, Option<String>)> {
    let rel = |p: &str| home.join(p);
    let owned = |category| Some((category, false, None));

    if path == rel(".local/share/Trash") {
        return owned(DiskCategory::Trash);
    }
    if path == rel(".local/share/Steam")
        || path == rel(".var/app/com.valvesoftware.Steam")
        || path == rel("Games")
        || path == rel("AppImages")
        || path == rel("Applications")
    {
        return owned(DiskCategory::Apps);
    }
    if path.parent() == Some(rel(".var/app").as_path()) {
        return Some((DiskCategory::Apps, false, Some(name.to_string())));
    }
    if path.parent() == Some(rel("snap").as_path()) {
        return Some((DiskCategory::Apps, false, Some(name.to_string())));
    }
    if path == rel(".local/share/flatpak") {
        return owned(DiskCategory::Apps);
    }
    if path == xdg.cache {
        return Some((DiskCategory::Cache, true, None));
    }
    let developer = [
        ".cargo",
        ".rustup",
        ".npm",
        ".bun",
        "go",
        ".gradle",
        ".m2",
        ".local/share/uv",
        ".local/share/pipx",
        ".nvm",
        ".pyenv",
        ".local/share/JetBrains",
        "code",
        "Projects",
        "src",
        "dev",
    ];
    if developer.iter().any(|p| path == rel(p)) {
        return owned(DiskCategory::Developer);
    }
    if path == xdg.pictures {
        return owned(DiskCategory::Pictures);
    }
    if path == xdg.videos {
        return owned(DiskCategory::Videos);
    }
    if path == xdg.music {
        return owned(DiskCategory::Music);
    }
    if path == xdg.download {
        return owned(DiskCategory::Downloads);
    }
    if path == xdg.documents || path == xdg.desktop {
        return owned(DiskCategory::Documents);
    }
    if is_steam_common_child(path) {
        return Some((DiskCategory::Apps, false, Some(name.to_string())));
    }
    None
}

/// True for a directory directly inside a `steamapps/common` folder.
fn is_steam_common_child(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    parent.file_name().is_some_and(|n| n == "common")
        && parent
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|n| n == "steamapps")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/home/tester";

    fn home() -> PathBuf {
        PathBuf::from(HOME)
    }

    fn xdg() -> XdgDirs {
        XdgDirs::from_user_dirs(&home(), home().join(".cache"), None)
    }

    fn classify(parent: DiskCategory, path: &str, has_tag: bool) -> (DiskCategory, bool, Option<String>) {
        let p = PathBuf::from(path);
        let name = p.file_name().unwrap_or_default().to_string_lossy().into_owned();
        classify_dir(&home(), &xdg(), (parent, false), &p, &name, has_tag)
    }

    #[test]
    fn cache_root_is_rebuildable_and_children_inherit() {
        let (cat, rebuild, owner) = classify(DiskCategory::Other, "/home/tester/.cache", false);
        assert_eq!((cat, rebuild, owner), (DiskCategory::Cache, true, None));
        let child = PathBuf::from("/home/tester/.cache/foo");
        let (cat, rebuild, _) = classify_dir(&home(), &xdg(), (DiskCategory::Cache, true), &child, "foo", false);
        assert_eq!((cat, rebuild), (DiskCategory::Cache, true));
    }

    #[test]
    fn node_modules_and_venv_are_developer_rebuildable() {
        let (cat, rebuild, _) = classify(DiskCategory::Other, "/home/tester/x/node_modules", false);
        assert_eq!((cat, rebuild), (DiskCategory::Developer, true));
        let (cat, rebuild, _) = classify(DiskCategory::Other, "/home/tester/x/.venv", false);
        assert_eq!((cat, rebuild), (DiskCategory::Developer, true));
    }

    #[test]
    fn target_needs_cachedir_tag() {
        let (cat, rebuild, _) = classify(DiskCategory::Other, "/home/tester/x/target", false);
        assert_eq!((cat, rebuild), (DiskCategory::Other, false));
        let (cat, rebuild, _) = classify(DiskCategory::Developer, "/home/tester/x/target", true);
        assert_eq!((cat, rebuild), (DiskCategory::Developer, true));
    }

    #[test]
    fn cachedir_tag_marks_other_as_caches_but_keeps_other_categories() {
        let (cat, rebuild, _) = classify(DiskCategory::Other, "/home/tester/x/build", true);
        assert_eq!((cat, rebuild), (DiskCategory::Cache, true));
        let (cat, rebuild, _) = classify(DiskCategory::Pictures, "/home/tester/Pictures/cache", true);
        assert_eq!((cat, rebuild), (DiskCategory::Pictures, true));
    }

    #[test]
    fn flatpak_and_snap_owners_are_their_ids() {
        let (cat, _, owner) = classify(DiskCategory::Other, "/home/tester/.var/app/org.x.App", false);
        assert_eq!((cat, owner), (DiskCategory::Apps, Some("org.x.App".to_string())));
        let (cat, _, owner) = classify(DiskCategory::Other, "/home/tester/snap/firefox", false);
        assert_eq!((cat, owner), (DiskCategory::Apps, Some("firefox".to_string())));
        let (cat, _, owner) = classify(DiskCategory::Other, "/home/tester/.var/app", false);
        assert_eq!((cat, owner), (DiskCategory::Other, None));
    }

    #[test]
    fn steamapps_common_children_are_owned_by_name() {
        let (cat, _, owner) = classify(
            DiskCategory::Other,
            "/home/tester/Library/steamapps/common/Hades",
            false,
        );
        assert_eq!((cat, owner), (DiskCategory::Apps, Some("Hades".to_string())));
        let (cat, _, owner) = classify(DiskCategory::Other, "/home/tester/steamapps/common", false);
        assert_eq!((cat, owner), (DiskCategory::Other, None));
    }

    #[test]
    fn trash_games_and_developer_roots() {
        let (cat, _, _) = classify(DiskCategory::Other, "/home/tester/.local/share/Trash", false);
        assert_eq!(cat, DiskCategory::Trash);
        let (cat, _, _) = classify(DiskCategory::Other, "/home/tester/.local/share/Steam", false);
        assert_eq!(cat, DiskCategory::Apps);
        let (cat, _, _) = classify(DiskCategory::Other, "/home/tester/.cargo", false);
        assert_eq!(cat, DiskCategory::Developer);
        let (cat, _, _) = classify(DiskCategory::Developer, "/home/tester/.cargo/registry", false);
        assert_eq!(cat, DiskCategory::Developer);
    }

    #[test]
    fn paths_outside_home_are_system() {
        let (cat, rebuild, owner) = classify(DiskCategory::System, "/usr/lib/node_modules", false);
        assert_eq!((cat, rebuild, owner), (DiskCategory::System, false, None));
        let (cat, _, _) = classify(DiskCategory::Other, "/var/cache", false);
        assert_eq!(cat, DiskCategory::System);
    }

    #[test]
    fn user_dirs_parsing_expands_home_and_skips_bad_lines() {
        let text = "# comment\n\
            XDG_PICTURES_DIR=\"$HOME/Images\"\n\
            XDG_VIDEOS_DIR=\"${HOME}/Clips\"\n\
            XDG_MUSIC_DIR=\"$HOME\"\n\
            XDG_DOWNLOAD_DIR=\"/data/dl\"\n\
            XDG_DESKTOP_DIR=\"relative/Desk\"\n\
            UNRELATED=\"$HOME/Nope\"\n";
        let parsed = parse_user_dirs(text, &home());
        assert_eq!(parsed.get("XDG_PICTURES_DIR"), Some(&home().join("Images")));
        assert_eq!(parsed.get("XDG_VIDEOS_DIR"), Some(&home().join("Clips")));
        assert_eq!(parsed.get("XDG_DOWNLOAD_DIR"), Some(&PathBuf::from("/data/dl")));
        assert!(!parsed.contains_key("XDG_MUSIC_DIR"));
        assert!(!parsed.contains_key("XDG_DESKTOP_DIR"));
        assert!(!parsed.contains_key("UNRELATED"));
    }

    #[test]
    fn xdg_dirs_use_user_dirs_then_english_defaults() {
        let text = "XDG_PICTURES_DIR=\"$HOME/Fotos\"\n";
        let dirs = XdgDirs::from_user_dirs(&home(), home().join(".cache"), Some(text));
        assert_eq!(dirs.pictures, home().join("Fotos"));
        assert_eq!(dirs.music, home().join("Music"));
        assert_eq!(dirs.download, home().join("Downloads"));
        assert_eq!(dirs.desktop, home().join("Desktop"));
        assert_eq!(dirs.cache, home().join(".cache"));
    }

    #[test]
    fn xdg_media_and_document_rules_use_resolved_dirs() {
        let text = "XDG_PICTURES_DIR=\"$HOME/Fotos\"\nXDG_DOCUMENTS_DIR=\"$HOME/Docs\"\n";
        let dirs = XdgDirs::from_user_dirs(&home(), home().join(".cache"), Some(text));
        let check = |path: &str| {
            let p = PathBuf::from(path);
            classify_dir(&home(), &dirs, (DiskCategory::Other, false), &p, "x", false).0
        };
        assert_eq!(check("/home/tester/Fotos"), DiskCategory::Pictures);
        assert_eq!(check("/home/tester/Docs"), DiskCategory::Documents);
        assert_eq!(check("/home/tester/Downloads"), DiskCategory::Downloads);
    }
}
