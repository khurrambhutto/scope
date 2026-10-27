//! Launcher reading and Gear Lever-compatible launcher rewriting.
//!
//! An integrated launcher keeps every key the AppImage shipped, then points
//! `TryExec`, `Exec`, action `Exec` lines, and `Icon` at the installed copy and
//! records `X-AppImage-Version` and `X-AppImage-Name`, exactly like Gear Lever.

use std::io::Read as _;
use std::path::Path;

use anyhow::{bail, Context, Result};

const MAX_LAUNCHER_BYTES: u64 = 256 * 1024;

/// One `.desktop` file kept as ordered groups of raw lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopFile {
    /// Lines before the first group header (comments).
    preamble: Vec<String>,
    groups: Vec<(String, Vec<String>)>,
}

impl DesktopFile {
    pub fn read(path: &Path) -> Result<Self> {
        let mut text = String::new();
        std::fs::File::open(path)
            .with_context(|| format!("Could not open {}", path.display()))?
            .take(MAX_LAUNCHER_BYTES + 1)
            .read_to_string(&mut text)
            .context("Launcher is not valid UTF-8")?;
        if text.len() as u64 > MAX_LAUNCHER_BYTES {
            bail!("Launcher is too large");
        }
        Ok(Self::parse(&text))
    }

    pub fn parse(text: &str) -> Self {
        let mut file = Self {
            preamble: Vec::new(),
            groups: Vec::new(),
        };
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
                file.groups.push((name.to_string(), Vec::new()));
            } else if let Some((_, lines)) = file.groups.last_mut() {
                lines.push(line.to_string());
            } else {
                file.preamble.push(line.to_string());
            }
        }
        file
    }

    /// Unlocalized value of `key` in `group`.
    pub fn get(&self, group: &str, key: &str) -> Option<&str> {
        self.groups
            .iter()
            .find(|(name, _)| name == group)?
            .1
            .iter()
            .find_map(|line| {
                let (k, v) = line.split_once('=')?;
                (k.trim() == key).then(|| v.trim())
            })
    }

    pub fn entry(&self, key: &str) -> Option<&str> {
        self.get("Desktop Entry", key)
    }

    /// Set (or append) an unlocalized key in a group.
    fn set(&mut self, group: &str, key: &str, value: &str) {
        let Some((_, lines)) = self.groups.iter_mut().find(|(name, _)| name == group) else {
            return;
        };
        let line = format!("{key}={value}");
        match lines
            .iter_mut()
            .find(|line| line.split_once('=').is_some_and(|(k, _)| k.trim() == key))
        {
            Some(existing) => *existing = line,
            None => {
                // Keep trailing blank lines after the keys.
                let at = lines
                    .iter()
                    .rposition(|line| !line.trim().is_empty())
                    .map_or(0, |i| i + 1);
                lines.insert(at, line);
            }
        }
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for line in &self.preamble {
            out.push_str(line);
            out.push('\n');
        }
        for (name, lines) in &self.groups {
            out.push('[');
            out.push_str(name);
            out.push_str("]\n");
            for line in lines {
                out.push_str(line);
                out.push('\n');
            }
        }
        out
    }

    fn action_groups(&self) -> Vec<String> {
        self.groups
            .iter()
            .map(|(name, _)| name)
            .filter(|name| name.starts_with("Desktop Action "))
            .cloned()
            .collect()
    }
}

/// Launch settings carried over when an installed AppImage is replaced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LaunchSettings {
    pub env: Vec<String>,
    pub args: Vec<String>,
}

impl LaunchSettings {
    /// Read the environment and arguments of an unescaped `Exec` value.
    pub fn from_exec(exec: &str) -> Self {
        let (env, mut tokens) = split_env(tokenize(exec));
        if !tokens.is_empty() {
            tokens.remove(0);
        }
        Self { env, args: tokens }
    }
}

/// Build the installed launcher from the AppImage's own launcher.
///
/// `previous` keeps the installed launcher's environment and arguments on
/// replacement, as Gear Lever does; a fresh install takes the shipped
/// arguments and adds `DESKTOPINTEGRATION=1` so the AppImage does not try to
/// integrate itself a second time.
pub fn integrated_launcher(
    shipped: &DesktopFile,
    appimage: &Path,
    icon: Option<&Path>,
    version: Option<&str>,
    previous: Option<&LaunchSettings>,
) -> Result<String> {
    let name = shipped
        .entry("Name")
        .filter(|name| !name.is_empty())
        .context("The AppImage launcher has no Name")?
        .to_string();
    if shipped
        .entry("Type")
        .is_some_and(|kind| kind != "Application")
    {
        bail!("The AppImage launcher is not an application launcher");
    }
    let program = quote_exec_arg(&appimage.to_string_lossy());
    let shipped_settings =
        LaunchSettings::from_exec(&unescape(shipped.entry("Exec").unwrap_or("")));
    let mut settings = previous.cloned().unwrap_or(shipped_settings);
    if !settings
        .env
        .iter()
        .any(|var| var.starts_with("DESKTOPINTEGRATION="))
    {
        settings.env.push("DESKTOPINTEGRATION=1".into());
    }
    let icon = icon
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "applications-other".into());

    let mut file = shipped.clone();
    file.set("Desktop Entry", "Type", "Application");
    let mut exec = String::new();
    if !settings.env.is_empty() {
        exec.push_str("env ");
        for var in &settings.env {
            exec.push_str(&quote_exec_arg(var));
            exec.push(' ');
        }
    }
    exec.push_str(&program);
    for arg in &settings.args {
        exec.push(' ');
        exec.push_str(&quote_exec_arg(arg));
    }
    file.set(
        "Desktop Entry",
        "TryExec",
        &escape_value(&appimage.to_string_lossy()),
    );
    file.set("Desktop Entry", "Exec", &escape_value(&exec));
    file.set("Desktop Entry", "Icon", &escape_value(&icon));
    for action in file.action_groups() {
        let action_args =
            LaunchSettings::from_exec(&unescape(file.get(&action, "Exec").unwrap_or(""))).args;
        let mut action_exec = program.clone();
        for arg in action_args {
            action_exec.push(' ');
            action_exec.push_str(&quote_exec_arg(&arg));
        }
        file.set(&action, "Exec", &escape_value(&action_exec));
        file.set(&action, "Icon", &escape_value(&icon));
    }
    if let Some(version) = version.filter(|v| !v.is_empty()) {
        file.set("Desktop Entry", "X-AppImage-Version", version);
    }
    file.set("Desktop Entry", "X-AppImage-Name", &name);
    Ok(file.to_text())
}

/// Split an `Exec` value into arguments, honouring desktop-entry quoting.
pub fn tokenize(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(current);
    }
    out
}

/// Separate a leading `env VAR=value ...` prefix from the command.
fn split_env(tokens: Vec<String>) -> (Vec<String>, Vec<String>) {
    let mut iter = tokens.into_iter().peekable();
    let mut env = Vec::new();
    if iter.peek().is_some_and(|t| t == "env") {
        iter.next();
        while iter.peek().is_some_and(|t| is_env_assignment(t)) {
            env.extend(iter.next());
        }
    }
    (env, iter.collect())
}

fn is_env_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// The program an `Exec` line runs, skipping an `env` prefix.
pub fn exec_program(exec: &str) -> Option<String> {
    split_env(tokenize(exec)).1.into_iter().next()
}

/// Quote one argument per the desktop-entry `Exec` rules when needed.
fn quote_exec_arg(arg: &str) -> String {
    const RESERVED: &str = " \t\n\"'\\><~|&;$*?#()`";
    if !arg.chars().any(|c| RESERVED.contains(c)) {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Undo `.desktop` string escapes.
pub fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Escape a string value for a `.desktop` file line.
fn escape_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
        .replace('\r', "\\r")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = "[Desktop Entry]\nType=Application\nName=Demo App\nExec=AppRun --flag %U\nIcon=demo\nTryExec=AppRun\nX-AppImage-Version=1.2.3\n\n[Desktop Action new]\nName=New\nExec=AppRun --new\n";

    #[test]
    fn integrated_launcher_points_every_command_at_the_installed_file() {
        let shipped = DesktopFile::parse(SHIPPED);
        let text = integrated_launcher(
            &shipped,
            Path::new("/home/u/AppImages/demo_app.appimage"),
            Some(Path::new("/home/u/AppImages/.icons/demo_app.png")),
            Some("1.2.3"),
            None,
        )
        .unwrap();
        let file = DesktopFile::parse(&text);
        assert_eq!(
            file.entry("Exec"),
            Some("env DESKTOPINTEGRATION=1 /home/u/AppImages/demo_app.appimage --flag %U")
        );
        assert_eq!(
            file.entry("TryExec"),
            Some("/home/u/AppImages/demo_app.appimage")
        );
        assert_eq!(
            file.entry("Icon"),
            Some("/home/u/AppImages/.icons/demo_app.png")
        );
        assert_eq!(file.entry("X-AppImage-Name"), Some("Demo App"));
        assert_eq!(
            file.get("Desktop Action new", "Exec"),
            Some("/home/u/AppImages/demo_app.appimage --new")
        );
    }

    #[test]
    fn replacement_keeps_the_installed_launch_settings() {
        let shipped = DesktopFile::parse(SHIPPED);
        let previous = LaunchSettings::from_exec(
            "env DESKTOPINTEGRATION=1 /home/u/AppImages/demo.appimage --no-sandbox %U",
        );
        let text = integrated_launcher(
            &shipped,
            Path::new("/home/u/AppImages/demo.appimage"),
            None,
            Some("2.0"),
            Some(&previous),
        )
        .unwrap();
        let file = DesktopFile::parse(&text);
        assert_eq!(
            file.entry("Exec"),
            Some("env DESKTOPINTEGRATION=1 /home/u/AppImages/demo.appimage --no-sandbox %U")
        );
        assert_eq!(file.entry("Icon"), Some("applications-other"));
        assert_eq!(file.entry("X-AppImage-Version"), Some("2.0"));
    }

    #[test]
    fn fresh_install_preserves_the_shipped_environment() {
        let shipped = DesktopFile::parse(
            "[Desktop Entry]\nType=Application\nName=Demo\nExec=env QT_QPA_PLATFORM=xcb AppRun --flag %U\n",
        );
        let installed = integrated_launcher(
            &shipped,
            Path::new("/home/u/AppImages/demo.appimage"),
            None,
            None,
            None,
        )
        .unwrap();
        let installed = DesktopFile::parse(&installed);
        assert_eq!(
            installed.entry("Exec"),
            Some("env QT_QPA_PLATFORM=xcb DESKTOPINTEGRATION=1 /home/u/AppImages/demo.appimage --flag %U")
        );
    }

    #[test]
    fn an_accepted_launcher_without_a_type_is_normalized_to_application() {
        let shipped = DesktopFile::parse("[Desktop Entry]\nName=Demo\nExec=AppRun\n");
        let text = integrated_launcher(
            &shipped,
            Path::new("/home/u/AppImages/demo.appimage"),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(DesktopFile::parse(&text).entry("Type"), Some("Application"));
    }

    #[test]
    fn paths_with_spaces_are_quoted_and_escaped() {
        let shipped = DesktopFile::parse(SHIPPED);
        let text = integrated_launcher(
            &shipped,
            Path::new("/home/my user/AppImages/demo.appimage"),
            None,
            None,
            None,
        )
        .unwrap();
        let file = DesktopFile::parse(&text);
        let exec = file.entry("Exec").unwrap();
        assert!(exec.contains("\"/home/my user/AppImages/demo.appimage\""));
        assert_eq!(
            exec_program(exec).as_deref(),
            Some("/home/my user/AppImages/demo.appimage")
        );
    }

    #[test]
    fn exec_program_skips_env_prefixes_and_quotes() {
        assert_eq!(
            exec_program("env DESKTOPINTEGRATION=1 /a/b.appimage --x %U").as_deref(),
            Some("/a/b.appimage")
        );
        assert_eq!(exec_program("\"/a b/c\" %U").as_deref(), Some("/a b/c"));
        assert_eq!(exec_program(""), None);
    }

    #[test]
    fn quoted_arguments_survive_rewriting() {
        let shipped = DesktopFile::parse(
            "[Desktop Entry]\nType=Application\nName=X\nExec=AppRun \"--title=a b\" %F\n",
        );
        let text =
            integrated_launcher(&shipped, Path::new("/a/x.appimage"), None, None, None).unwrap();
        let exec = unescape(DesktopFile::parse(&text).entry("Exec").unwrap());
        assert_eq!(
            tokenize(&exec),
            vec![
                "env",
                "DESKTOPINTEGRATION=1",
                "/a/x.appimage",
                "--title=a b",
                "%F"
            ]
        );
    }

    #[test]
    fn unescape_handles_desktop_escapes() {
        assert_eq!(unescape(r"/a\sb\\c"), "/a b\\c");
    }

    #[test]
    fn non_application_launchers_are_rejected() {
        let shipped = DesktopFile::parse("[Desktop Entry]\nType=Link\nName=X\nURL=https://x\n");
        assert!(integrated_launcher(&shipped, Path::new("/a"), None, None, None).is_err());
    }
}
