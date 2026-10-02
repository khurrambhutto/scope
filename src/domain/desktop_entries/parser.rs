//! Minimal `.desktop` (INI-ish) parser for the fields Scope needs.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A parsed, visible GUI app entry.
#[derive(Debug, Clone, Serialize)]
pub struct DesktopApp {
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub terminal: bool,
    /// `NoDisplay=true` entries are skipped by the discoverer but kept here for
    /// internal lookups (we filter them out before sorting).
    pub no_display: bool,
}

/// Parse one `.desktop` file into a [`DesktopApp`], or `None` when it cannot be
/// read or carries no usable `Name`.
pub fn parse(id: &str, path: &Path) -> Option<DesktopApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut sections: HashMap<String, Vec<(String, String)>> = HashMap::new();
    let mut current = String::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
            current = rest.to_string();
            sections.entry(current.clone()).or_default();
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            sections
                .entry(current.clone())
                .or_default()
                .push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    let entry = sections
        .iter()
        .find(|(name, _)| name.as_str() == "Desktop Entry")
        .map(|(_, kv)| kv)?;

    // Locale-suffixed keys (e.g. `Name[en]`) — pick the bare one, else the first
    // locale variant as a fallback.
    fn field<'a>(entry: &'a [(String, String)], key: &str) -> Option<&'a str> {
        if let Some((_, v)) = entry.iter().find(|(k, _)| k == key) {
            return Some(v.as_str());
        }
        for (k, v) in entry {
            if let Some(rest) = k.strip_prefix(&format!("{key}[")) {
                if rest.ends_with(']') {
                    return Some(v.as_str());
                }
            }
        }
        None
    }

    let type_ = field(entry, "Type").unwrap_or("Application");
    if type_ != "Application" {
        return None;
    }

    let exec_raw = field(entry, "Exec").unwrap_or("").to_string();
    // Only `Application` entries with an Exec are useful here; Link types etc. lack one.
    if exec_raw.is_empty() {
        return None;
    }

    Some(DesktopApp {
        id: id.to_string(),
        name: field(entry, "Name").unwrap_or(id).to_string(),
        comment: field(entry, "Comment").map(str::to_string),
        exec: exec_raw,
        icon: field(entry, "Icon").map(str::to_string),
        categories: field(entry, "Categories")
            .unwrap_or("")
            .split(';')
            .filter(|c| !c.is_empty())
            .map(str::to_string)
            .collect(),
        terminal: field(entry, "Terminal")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
        no_display: field(entry, "NoDisplay")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false),
    })
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    fn write_desktop(body: &str) -> std::path::PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "scope-parser-test-{}-{n}.desktop",
            std::process::id()
        ));
        let mut file = fs::File::create(&path).expect("create temp desktop file");
        file.write_all(body.as_bytes()).expect("write temp desktop file");
        path
    }

    #[test]
    fn parse_reads_name_exec_icon_and_categories() {
        let path = write_desktop(
            "[Desktop Entry]\nType=Application\nName=Test App\nExec=testapp %U\n\
             Icon=testapp\nCategories=Utility;System;\n",
        );
        let app = parse("testapp", &path).unwrap();
        assert_eq!(app.name, "Test App");
        assert_eq!(app.exec, "testapp %U");
        assert_eq!(app.icon.as_deref(), Some("testapp"));
        assert_eq!(app.categories, vec!["Utility", "System"]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_marks_a_no_display_entry() {
        let path = write_desktop(
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        );
        assert!(parse("hidden", &path).unwrap().no_display);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_reads_the_terminal_flag() {
        let path =
            write_desktop("[Desktop Entry]\nType=Application\nName=Tool\nExec=tool\nTerminal=true\n");
        assert!(parse("tool", &path).unwrap().terminal);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_rejects_a_non_application_entry() {
        let path = write_desktop("[Desktop Entry]\nType=Link\nName=A link\nURL=https://x\n");
        assert!(parse("link", &path).is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_rejects_an_entry_without_exec() {
        let path = write_desktop("[Desktop Entry]\nType=Application\nName=No Exec\n");
        assert!(parse("no-exec", &path).is_none());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_returns_none_for_a_missing_file() {
        assert!(parse("x", Path::new("/no/such/file.desktop")).is_none());
    }
}
