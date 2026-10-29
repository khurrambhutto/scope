//! Linux icon-theme resolution and a narrow URI-scheme protocol for serving
//! resolved icons to the webview.
//!
//! This module turns the `Icon=` value found in a `.desktop` entry (usually a
//! theme name such as `firefox`, sometimes an absolute path) into a real on-disk
//! image path, following the freedesktop.org Icon Theme Specification closely
//! enough for desktop use:
//!
//! 1. If the value is an absolute path, use it directly (with extension fallback).
//! 2. Otherwise search the current GTK icon theme, then `hicolor`, walking
//!    `Inherits=` parent chains declared in each theme's `index.theme`.
//! 3. Finally fall back to `/usr/share/pixmaps/<name>.<ext>`.
//!
//! Architecture borrowed from the local `klauncher` reference
//! (`src-tauri/src/platform/linux/icon_resolver.rs`), adapted for Scope and
//! renamed so the module path matches the project layout in `AGENTS.md`.
//!
//! The frontend never receives broad filesystem access. It only ever gets
//! `scope-icon://localhost/<absolute-path>` URLs produced here, and the
//! Tauri shell's `scope-icon` protocol (GPUI decodes them via `backend::icon_path`)
//! serves exactly those resolved paths.

use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// Prefer scalable artwork, then raster sizes suitable for list and dialog icons.
const ICON_SIZES: &[&str] = &[
    "scalable", "128x128", "64x64", "48x48", "32x32", "24x24", "22x22", "16x16", "256x256",
    "512x512",
];

/// File extensions tried, in priority order. SVG renders crisply at any size.
const ICON_EXTENSIONS: &[&str] = &["svg", "png", "xpm"];

/// Icon lookup state for one inventory scan.
pub struct IconResolver {
    roots: Vec<PathBuf>,
    theme: Option<String>,
    detect_theme: bool,
    resolved: HashMap<String, Option<PathBuf>>,
    private: HashMap<(String, PathBuf), Option<PathBuf>>,
    themes: HashMap<PathBuf, (Vec<PathBuf>, Vec<String>)>,
    directory_entries: HashMap<PathBuf, Option<HashSet<OsString>>>,
}

impl Default for IconResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl IconResolver {
    pub fn new() -> Self {
        let mut resolver = Self::with_config(collect_base_dirs(), None);
        resolver.detect_theme = true;
        resolver
    }

    fn with_config(mut roots: Vec<PathBuf>, theme: Option<String>) -> Self {
        let mut seen = HashSet::new();
        roots.retain(|root| seen.insert(root.clone()));
        Self {
            roots,
            theme,
            detect_theme: false,
            resolved: HashMap::new(),
            private: HashMap::new(),
            themes: HashMap::new(),
            directory_entries: HashMap::new(),
        }
    }

    pub fn resolve(&mut self, icon: &str) -> Option<PathBuf> {
        if icon.is_empty() {
            return None;
        }
        if let Some(result) = self.resolved.get(icon) {
            return result.clone();
        }
        let result = if Path::new(icon).is_absolute() {
            resolve_absolute_icon(Path::new(icon))
        } else {
            if self.detect_theme {
                self.theme = detect_icon_theme(&self.roots);
                self.detect_theme = false;
            }
            let roots = self.roots.clone();
            self.search_roots(icon, &roots)
                .or_else(|| lookup_in_pixmaps(icon))
        };
        self.resolved.insert(icon.to_owned(), result.clone());
        result
    }

    pub fn resolve_for_app(&mut self, icon: &str, executable: Option<&Path>) -> Option<PathBuf> {
        if let Some(found) = self.resolve(icon) {
            return Some(found);
        }
        let executable = executable?;
        let key = (icon.to_owned(), executable.to_path_buf());
        if let Some(result) = self.private.get(&key) {
            return result.clone();
        }
        let executable = fs::canonicalize(executable).unwrap_or_else(|_| executable.to_path_buf());
        let mut result = None;
        for prefix in executable.ancestors().take(8) {
            let roots = [prefix.join("share/icons")];
            result = self
                .search_roots(icon, &roots)
                .or_else(|| lookup_in_pixmaps_at(icon, &prefix.join("share/pixmaps")));
            if result.is_some() {
                break;
            }
        }
        self.private.insert(key, result.clone());
        result
    }

    fn search_roots(&mut self, icon: &str, roots: &[PathBuf]) -> Option<PathBuf> {
        let mut names = Vec::new();
        if let Some(theme) = &self.theme {
            names.push(theme.clone());
        }
        if !names.iter().any(|name| name == "hicolor") {
            names.push("hicolor".into());
        }
        for name in names {
            for root in roots {
                if let Some(found) =
                    self.lookup(&root.join(&name), icon, roots, &mut HashSet::new())
                {
                    return Some(found);
                }
            }
        }
        None
    }

    fn lookup(
        &mut self,
        root: &Path,
        icon: &str,
        roots: &[PathBuf],
        visited: &mut HashSet<PathBuf>,
    ) -> Option<PathBuf> {
        if !visited.insert(root.to_path_buf()) {
            return None;
        }
        let (dirs, parents) = self
            .themes
            .entry(root.to_path_buf())
            .or_insert_with(|| {
                if !root.is_dir() {
                    return (Vec::new(), Vec::new());
                }
                let mut dirs = Vec::new();
                for size in ICON_SIZES {
                    for context in ["apps", ""] {
                        let dir = root.join(size).join(context);
                        if dir.is_dir() {
                            dirs.push(dir);
                        }
                    }
                }
                let content = fs::read_to_string(root.join("index.theme")).unwrap_or_default();
                let (declared, parents) = theme_metadata(&content);
                for name in declared {
                    let path = Path::new(&name);
                    if path
                        .components()
                        .all(|part| matches!(part, std::path::Component::Normal(_)))
                    {
                        let dir = root.join(path);
                        if !dirs.contains(&dir) && dir.is_dir() {
                            dirs.push(dir);
                        }
                    }
                }
                (dirs, parents)
            })
            .clone();
        for dir in dirs {
            let names = self
                .directory_entries
                .entry(dir.clone())
                .or_insert_with(|| directory_names(&dir));
            for ext in ICON_EXTENSIONS {
                let filename = format!("{icon}.{ext}");
                if names
                    .as_ref()
                    .is_some_and(|names| !names.contains(std::ffi::OsStr::new(&filename)))
                {
                    continue;
                }
                let path = dir.join(filename);
                if path.is_file() {
                    return Some(path);
                }
            }
        }
        for parent in parents {
            for base in roots {
                if let Some(found) = self.lookup(&base.join(&parent), icon, roots, visited) {
                    return Some(found);
                }
            }
        }
        None
    }
}

// If a directory cannot be fully indexed, retain direct lookup as a fallback.
fn directory_names(path: &Path) -> Option<HashSet<OsString>> {
    let mut names = HashSet::new();
    for (index, entry) in fs::read_dir(path).ok()?.enumerate() {
        if index >= 25_000 {
            return None;
        }
        names.insert(entry.ok()?.file_name());
    }
    Some(names)
}

/// Absolute `Icon=` values: use the file as-is, or try common extensions when
/// the given path has none / does not exist.
fn resolve_absolute_icon(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }

    for ext in ICON_EXTENSIONS {
        let mut p = path.to_path_buf();
        p.set_extension(ext);
        if p.is_file() {
            return Some(p);
        }
    }

    None
}

fn collect_base_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // User themes take priority over system themes.
    if let Some(data_home) = env::var_os("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(data_home).join("icons"));
    } else if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".local/share/icons"));
    }
    if let Some(home) = env::var_os("HOME") {
        dirs.push(PathBuf::from(&home).join(".icons"));
    }

    let data_dirs = env::var_os("XDG_DATA_DIRS")
        .map(|value| env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_else(|| {
            vec![
                PathBuf::from("/usr/local/share"),
                PathBuf::from("/usr/share"),
            ]
        });
    for data_dir in data_dirs {
        dirs.push(data_dir.join("icons"));
    }

    dirs
}

/// Detect the active GTK icon theme name. Best-effort: falls back to none
/// (which leaves `hicolor` as the only searched theme).
fn detect_icon_theme(base_dirs: &[PathBuf]) -> Option<String> {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            if !value.is_empty() && base_dirs.iter().any(|dir| dir.join(&value).is_dir()) {
                return Some(value);
            }
        }
    }

    if let Ok(home) = env::var("HOME") {
        let settings_ini = PathBuf::from(&home).join(".config/gtk-3.0/settings.ini");
        if let Ok(content) = fs::read_to_string(&settings_ini) {
            for line in content.lines() {
                if let Some(value) = line.strip_prefix("gtk-icon-theme-name=") {
                    let value = value.trim();
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Application directories and inherited themes from one metadata read.
fn theme_metadata(content: &str) -> (Vec<String>, Vec<String>) {
    let mut section = "";
    let mut declared = Vec::new();
    let mut parents = Vec::new();
    let mut contexts = HashMap::new();
    for line in content.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|name| name.strip_suffix(']'))
        {
            section = name;
        } else if let Some((key, value)) = line.split_once('=') {
            match (section, key.trim()) {
                ("Icon Theme", "Directories" | "ScaledDirectories") => {
                    declared.extend(value.split(',').map(str::trim))
                }
                ("Icon Theme", "Inherits") => parents.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                ),
                (_, "Context") => {
                    contexts.insert(section, value.trim());
                }
                _ => {}
            }
        }
    }
    let declared = declared
        .into_iter()
        .filter(|name| {
            !name.is_empty()
                && contexts
                    .get(name)
                    .is_none_or(|context| *context == "Applications")
        })
        .map(str::to_string)
        .collect();
    (declared, parents)
}

/// Last-resort fallback: legacy `/usr/share/pixmaps/<name>.<ext>` icons.
fn lookup_in_pixmaps(icon_name: &str) -> Option<PathBuf> {
    lookup_in_pixmaps_at(icon_name, Path::new("/usr/share/pixmaps"))
}

fn lookup_in_pixmaps_at(icon_name: &str, pixmaps: &Path) -> Option<PathBuf> {
    for ext in ICON_EXTENSIONS {
        let path = pixmaps.join(format!("{icon_name}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }

    let path = pixmaps.join(icon_name);
    if path.is_file() {
        return Some(path);
    }

    None
}

/// MIME type for an icon path, used by the `scope-icon://` protocol handler.
/// Tauri shell uses this; GPUI loads icon files directly via `backend::icon_path`.
#[allow(dead_code)]
pub fn mime_for_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".svg") || lower.ends_with(".svgz") {
        "image/svg+xml"
    } else if lower.ends_with(".xpm") {
        "image/x-xpixmap"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".ico") {
        "image/x-icon"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "application/octet-stream"
    }
}

/// Build the webview-facing URL for a resolved icon path.
///
/// Returns `scope-icon://localhost/<percent-encoded-path>`. The path is
/// percent-encoded so spaces / non-ASCII never break URL parsing; the protocol
/// handler decodes it back to a filesystem path before reading.
pub fn icon_url(path: &Path) -> String {
    let encoded = percent_encode_path(&path.to_string_lossy());
    format!("scope-icon://localhost{encoded}")
}

/// Minimal percent-encoder for a path component, sufficient for icon paths.
/// Encodes everything outside the small "unreserved + path" set, including
/// spaces and non-ASCII bytes (as UTF-8 percent-escapes).
fn percent_encode_path(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{byte:02X}"));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_resolves_to_none() {
        assert!(IconResolver::new().resolve("").is_none());
    }

    #[test]
    fn absolute_missing_path_is_none() {
        assert!(IconResolver::new()
            .resolve("/this/does/not/exist/anywhere.png")
            .is_none());
    }

    #[test]
    fn app_icon_resolution_falls_back_to_the_executable_install_prefix() {
        let name = format!("scope-private-icon-{}", std::process::id());
        let prefix = std::env::temp_dir().join(format!("scope-icon-test-{}", std::process::id()));
        let executable = prefix.join("usr/bin/editor");
        let icon = prefix
            .join("usr/share/icons/hicolor/scalable/apps")
            .join(format!("{name}.svg"));
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(&executable, "test executable").unwrap();
        fs::write(&icon, "<svg/>").unwrap();

        assert_eq!(
            IconResolver::new().resolve_for_app(&name, Some(&executable)),
            Some(icon)
        );

        let _ = fs::remove_dir_all(prefix);
    }

    #[test]
    fn fresh_resolvers_observe_icon_and_inheritance_changes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let active = root.join("active");
        let parent = root.join("parent");
        let dir = parent.join("custom/apps");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            active.join("index.theme"),
            "[Icon Theme]\nInherits=parent\n",
        )
        .unwrap();
        fs::write(
            parent.join("index.theme"),
            "[Icon Theme]\nDirectories=custom/apps\n",
        )
        .unwrap();
        let config =
            || IconResolver::with_config(vec![root.into(), root.into()], Some("active".into()));
        let icon = dir.join("sample.png");
        let mut old_scan = config();
        assert_eq!(old_scan.resolve("sample"), None);
        fs::write(&icon, "first").unwrap();
        assert_eq!(old_scan.resolve("sample"), None);
        assert_eq!(config().resolve("sample"), Some(icon.clone()));
        fs::remove_file(&icon).unwrap();
        let replacement = dir.join("sample.svg");
        fs::write(&replacement, "second").unwrap();
        assert_eq!(config().resolve("sample"), Some(replacement));
        fs::write(active.join("index.theme"), "[Icon Theme]\nInherits=other\n").unwrap();
        assert_eq!(config().resolve("sample"), None);
    }

    #[test]
    fn theme_directories_keep_application_context_and_skip_unrelated_artwork() {
        let (dirs, parents) = theme_metadata("[Icon Theme]\nDirectories=custom/apps,custom/status,legacy\nScaledDirectories=custom/hires\nInherits=parent,hicolor\n[custom/apps]\nContext=Applications\n[custom/status]\nContext=Status\n[custom/hires]\nContext=Applications\n");
        assert_eq!(dirs, vec!["custom/apps", "legacy", "custom/hires"]);
        assert_eq!(parents, vec!["parent", "hicolor"]);
    }

    #[test]
    fn private_cache_distinguishes_executable_prefixes() {
        let temp = tempfile::tempdir().unwrap();
        let mut resolver = IconResolver::with_config(vec![], None);
        let first = temp.path().join("one/bin/app");
        let second = temp.path().join("two/bin/app");
        let icon = temp.path().join("two/share/pixmaps/private.png");
        fs::create_dir_all(icon.parent().unwrap()).unwrap();
        fs::write(&icon, "icon").unwrap();
        assert_eq!(resolver.resolve_for_app("private", Some(&first)), None);
        assert_eq!(
            resolver.resolve_for_app("private", Some(&second)),
            Some(icon)
        );
    }

    #[test]
    fn url_encodes_spaces_and_keeps_slashes() {
        let url = icon_url(Path::new("/usr/share/icons/a b/firefox.svg"));
        assert_eq!(
            url,
            "scope-icon://localhost/usr/share/icons/a%20b/firefox.svg"
        );
    }

    #[test]
    fn mime_table_covers_common_formats() {
        assert_eq!(mime_for_path("x.PNG"), "image/png");
        assert_eq!(mime_for_path("icon.svg"), "image/svg+xml");
        assert_eq!(mime_for_path("icon.xpm"), "image/x-xpixmap");
        assert_eq!(mime_for_path("icon.unknown"), "application/octet-stream");
    }

    #[test]
    fn cyclic_theme_inheritance_terminates() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a");
        let b = root.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(a.join("index.theme"), "[Icon Theme]\nInherits=b\n").unwrap();
        fs::write(b.join("index.theme"), "[Icon Theme]\nInherits=a\n").unwrap();
        assert!(
            IconResolver::with_config(vec![root.path().to_path_buf()], Some("a".into()))
                .resolve("missing")
                .is_none()
        );
    }
}
