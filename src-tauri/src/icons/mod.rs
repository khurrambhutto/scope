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
//! [`crate::lib`] URI-scheme protocol serves exactly those resolved paths.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

/// Size directories searched, largest-useful first. `scalable` covers SVGs.
const ICON_SIZES: &[&str] = &[
    "512x512",
    "256x256",
    "128x128",
    "64x64",
    "48x48",
    "32x32",
    "24x24",
    "22x22",
    "16x16",
    "scalable",
];

/// File extensions tried, in priority order. SVG renders crisply at any size.
const ICON_EXTENSIONS: &[&str] = &["svg", "png", "xpm"];

static RESOLVED_CACHE: OnceLock<Mutex<HashMap<String, Option<PathBuf>>>> = OnceLock::new();
static THEME_NAME: OnceLock<Option<String>> = OnceLock::new();
static BASE_DIRS: OnceLock<Vec<PathBuf>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<String, Option<PathBuf>>> {
    RESOLVED_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn theme_name() -> &'static Option<String> {
    THEME_NAME.get_or_init(detect_icon_theme)
}

fn base_dirs() -> &'static Vec<PathBuf> {
    BASE_DIRS.get_or_init(collect_base_dirs)
}

/// Resolve an `Icon=` value to an absolute file path, or `None` if not found.
///
/// Results are memoized in a process-global cache; repeated lookups for the
/// same icon name are O(1).
pub fn resolve(icon_value: &str) -> Option<PathBuf> {
    if icon_value.is_empty() {
        return None;
    }

    if let Some(cached) = cache().lock().unwrap().get(icon_value) {
        return cached.clone();
    }

    let result = resolve_uncached(icon_value);
    cache()
        .lock()
        .unwrap()
        .insert(icon_value.to_string(), result.clone());
    result
}

fn resolve_uncached(icon_value: &str) -> Option<PathBuf> {
    let path = Path::new(icon_value);
    if path.is_absolute() {
        return resolve_absolute_icon(path);
    }

    // Search the active theme first, then the universal `hicolor` fallback.
    // Parent themes declared via `Inherits=` are walked recursively inside
    // `lookup_in_theme`.
    let mut themes = Vec::new();
    if let Some(current) = theme_name() {
        themes.push(current.as_str());
    }
    themes.push("hicolor");

    let basedirs = base_dirs();
    for theme in &themes {
        for basedir in basedirs {
            let theme_root = basedir.join(theme);
            if !theme_root.is_dir() {
                continue;
            }
            if let Some(found) =
                lookup_in_theme(&theme_root, icon_value, basedirs, &mut HashMap::new())
            {
                return Some(found);
            }
        }
    }

    lookup_in_pixmaps(icon_value)
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
fn detect_icon_theme() -> Option<String> {
    if let Ok(output) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "icon-theme"])
        .output()
    {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout)
                .trim()
                .trim_matches('\'')
                .to_string();
            if !value.is_empty() && base_dirs().iter().any(|dir| dir.join(&value).is_dir()) {
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

/// Search one theme directory for `icon_name`, then recurse into inherited
/// parent themes. `parent_cache` avoids re-parsing `index.theme` for the same
/// theme across sibling searches.
fn lookup_in_theme(
    theme_root: &Path,
    icon_name: &str,
    basedirs: &[PathBuf],
    parent_cache: &mut HashMap<PathBuf, Vec<String>>,
) -> Option<PathBuf> {
    let parents = if let Some(cached) = parent_cache.get(theme_root) {
        cached.clone()
    } else {
        let parents = parse_theme_parents(&theme_root.join("index.theme"));
        parent_cache.insert(theme_root.to_path_buf(), parents.clone());
        parents
    };

    // Prefer the `apps` subdirectory (where application icons live), then a
    // bare size-directory match for themes that don't categorize by context.
    for size in ICON_SIZES {
        for context in ["apps", ""] {
            let dir = if context.is_empty() {
                theme_root.join(size)
            } else {
                theme_root.join(size).join(context)
            };
            if !dir.is_dir() {
                continue;
            }
            for ext in ICON_EXTENSIONS {
                let path = dir.join(format!("{icon_name}.{ext}"));
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }

    for parent_name in &parents {
        for basedir in basedirs {
            let parent_root = basedir.join(parent_name);
            if parent_root.is_dir() && parent_root != theme_root {
                if let Some(found) =
                    lookup_in_theme(&parent_root, icon_name, basedirs, parent_cache)
                {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// Parse the `Inherits=` line from a theme's `index.theme` `[Icon Theme]` table.
fn parse_theme_parents(index_theme_path: &Path) -> Vec<String> {
    let Ok(content) = fs::read_to_string(index_theme_path) else {
        return Vec::new();
    };

    let mut in_icon_theme = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.eq_ignore_ascii_case("[Icon Theme]") {
            in_icon_theme = true;
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            // A new section ends the [Icon Theme] table.
            if in_icon_theme {
                break;
            }
            continue;
        }
        if !in_icon_theme {
            continue;
        }
        if let Some(value) = line.strip_prefix("Inherits=") {
            return value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }

    Vec::new()
}

/// Last-resort fallback: legacy `/usr/share/pixmaps/<name>.<ext>` icons.
fn lookup_in_pixmaps(icon_name: &str) -> Option<PathBuf> {
    let pixmaps = PathBuf::from("/usr/share/pixmaps");

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
        assert!(resolve("").is_none());
    }

    #[test]
    fn absolute_missing_path_is_none() {
        assert!(resolve("/this/does/not/exist/anywhere.png").is_none());
    }

    #[test]
    fn url_encodes_spaces_and_keeps_slashes() {
        let url = icon_url(Path::new("/usr/share/icons/a b/firefox.svg"));
        assert_eq!(url, "scope-icon://localhost/usr/share/icons/a%20b/firefox.svg");
    }

    #[test]
    fn mime_table_covers_common_formats() {
        assert_eq!(mime_for_path("x.PNG"), "image/png");
        assert_eq!(mime_for_path("icon.svg"), "image/svg+xml");
        assert_eq!(mime_for_path("icon.xpm"), "image/x-xpixmap");
        assert_eq!(mime_for_path("icon.unknown"), "application/octet-stream");
    }
}
