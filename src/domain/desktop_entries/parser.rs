//! Minimal `.desktop` (INI-ish) parser for the fields Scope needs.

use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A parsed launcher visible in the current desktop's app list.
#[derive(Debug, Clone, Serialize)]
pub struct DesktopApp {
    pub id: String,
    pub file_path: std::path::PathBuf,
    pub name: String,
    pub comment: Option<String>,
    pub exec: String,
    pub executable: Option<PathBuf>,
    pub icon: Option<String>,
    pub categories: Vec<String>,
    pub terminal: bool,
    /// Whether this launcher belongs in the current desktop's app list.
    pub menu_visible: bool,
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
    let executable = exec_program_path(&exec_raw);

    let desktops = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .split(':')
        .filter(|desktop| !desktop.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    let menu_visible = !field(entry, "NoDisplay")
        .is_some_and(|value| value.eq_ignore_ascii_case("true"))
        && !field(entry, "Hidden").is_some_and(|value| value.eq_ignore_ascii_case("true"))
        && desktop_visibility_matches(
            field(entry, "OnlyShowIn").unwrap_or(""),
            field(entry, "NotShowIn").unwrap_or(""),
            &desktops,
        )
        && match field(entry, "TryExec") {
            Some(command) => try_exec_exists(command),
            None => true,
        }
        && executable.is_some();

    Some(DesktopApp {
        id: id.to_string(),
        file_path: path.to_path_buf(),
        name: field(entry, "Name").unwrap_or(id).to_string(),
        comment: field(entry, "Comment").map(str::to_string),
        exec: exec_raw,
        executable,
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
        menu_visible,
    })
}

fn desktop_visibility_matches(only_show_in: &str, not_show_in: &str, desktops: &[String]) -> bool {
    // If launched outside a desktop session there is no reliable environment
    // to match against; don't hide otherwise valid launchers on that basis.
    if desktops.is_empty() {
        return true;
    }
    let only_show_in = desktop_list(only_show_in);
    let not_show_in = desktop_list(not_show_in);
    (only_show_in.is_empty()
        || only_show_in
            .iter()
            .any(|entry| desktops.iter().any(|desktop| desktop == entry)))
        && !not_show_in
            .iter()
            .any(|entry| desktops.iter().any(|desktop| desktop == entry))
}

fn desktop_list(value: &str) -> Vec<&str> {
    value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn try_exec_exists(command: &str) -> bool {
    let path = Path::new(command);
    if path.is_absolute() {
        return path.is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
}

fn exec_program(exec: &str) -> Option<&str> {
    let mut rest = exec.trim();
    if let Some(without_env) = rest.strip_prefix("env ") {
        rest = without_env;
        loop {
            let token = rest.split_whitespace().next()?;
            if !token.contains('=') {
                break;
            }
            rest = rest[token.len()..].trim_start();
        }
    }
    if let Some(quoted) = rest.strip_prefix('"') {
        return quoted.split_once('"').map(|(program, _)| program);
    }
    rest.split_whitespace().next()
}

fn exec_program_path(exec: &str) -> Option<PathBuf> {
    let program = exec_program(exec)?;
    let path = Path::new(program);
    if path.is_absolute() {
        return try_exec_exists(program).then(|| path.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
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
        file.write_all(body.as_bytes())
            .expect("write temp desktop file");
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
    fn parse_marks_a_no_display_entry_hidden_from_the_app_menu() {
        let path = write_desktop(
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nNoDisplay=true\n",
        );
        assert!(!parse("hidden", &path).unwrap().menu_visible);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn desktop_visibility_obeys_only_show_in_and_not_show_in() {
        let desktops = vec!["ubuntu".to_string(), "GNOME".to_string()];
        assert!(desktop_visibility_matches("GNOME;", "", &desktops));
        assert!(!desktop_visibility_matches("KDE;", "", &desktops));
        assert!(!desktop_visibility_matches("", "GNOME;", &desktops));
        assert!(desktop_visibility_matches("", "KDE;", &desktops));
        assert!(desktop_visibility_matches("GNOME;", "", &[]));
    }

    #[test]
    fn executable_parser_handles_environment_wrappers_and_quoted_paths() {
        assert_eq!(
            exec_program("env DESKTOPINTEGRATION=1 /home/user/.local/zed.app/bin/zed %U"),
            Some("/home/user/.local/zed.app/bin/zed")
        );
        assert_eq!(
            exec_program("\"/home/user/My App/bin/editor\" %F"),
            Some("/home/user/My App/bin/editor")
        );
    }

    #[test]
    fn hidden_entries_are_not_menu_visible() {
        let path = write_desktop(
            "[Desktop Entry]\nType=Application\nName=Hidden\nExec=hidden\nHidden=true\n",
        );
        assert!(!parse("hidden", &path).unwrap().menu_visible);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_reads_the_terminal_flag() {
        let path = write_desktop(
            "[Desktop Entry]\nType=Application\nName=Tool\nExec=tool\nTerminal=true\n",
        );
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
