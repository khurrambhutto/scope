//! Fresh, local installation claims; display filtering is never an ownership proof.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::{catalog, identity::Roots};
use crate::domain::{
    desktop_entries,
    package::{InstalledPackage, PackageSource},
    system, tool_managers,
};

#[derive(Default)]
pub(super) struct Claims {
    pub complete: bool,
    pub warnings: Vec<String>,
    pub packages: Vec<InstalledPackage>,
    pub launchers: Vec<desktop_entries::DesktopApp>,
    pub owned_paths: HashSet<PathBuf>,
    pub writer: Option<String>,
    /// The roots claims were collected for; reviewed places resolve against them.
    pub roots: Option<Roots>,
}

impl Claims {
    /// Whether this launcher belongs to the target: one of its own files, or a
    /// user override with the same desktop ID that still runs a program the
    /// target installed (apps such as Claude desktop rewrite their launcher in
    /// ~/.local/share/applications). Shared runners such as env or flatpak are
    /// never target files, so they cannot pass.
    pub fn owns_launcher(&self, app: &desktop_entries::DesktopApp) -> bool {
        if self.owned_paths.contains(&app.file_path) {
            return true;
        }
        let Some(executable) = &app.executable else {
            return false;
        };
        let runs_owned = self.owned_paths.contains(executable)
            || executable
                .canonicalize()
                .is_ok_and(|path| self.owned_paths.contains(&path));
        runs_owned
            && self.launchers.iter().any(|own| {
                own.file_path != app.file_path
                    && own.file_path.file_name() == app.file_path.file_name()
                    && self.owned_paths.contains(&own.file_path)
            })
    }

    pub fn shared_reason(&self, target: &InstalledPackage, path: &Path) -> Option<String> {
        let rule = catalog::known(target);
        let mut aliases: Vec<String> = match rule {
            Some(rule) => rule.aliases.iter().map(|s| s.to_string()).collect(),
            None => catalog::identities(target),
        };
        // A folder found by name may belong to any app with that name. A
        // reviewed place is the target app's own; only other installations of
        // that app (its aliases) or rules naming the place can share it.
        let reviewed = rule.zip(self.roots.as_ref()).is_some_and(|(rule, roots)| {
            rule.places.iter().any(|place| place.resolve(roots) == path)
        });
        if !reviewed {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                aliases.push(catalog::normalize(name));
            }
        }
        for other in &self.packages {
            if other.key == target.key {
                continue;
            }
            let ids = catalog::identities(other);
            let other_rule = catalog::known(other);
            let same_rule = other_rule
                .is_some_and(|other_rule| rule.is_some_and(|rule| std::ptr::eq(rule, other_rule)));
            let named_place = !path.as_os_str().is_empty()
                && other_rule
                    .zip(self.roots.as_ref())
                    .is_some_and(|(other_rule, roots)| other_rule.names_path(roots, path));
            let flatpak_same_id = target.source == PackageSource::Flatpak
                && other.source == PackageSource::Flatpak
                && target.package_id == other.package_id;
            if same_rule
                || named_place
                || flatpak_same_id
                || ids.iter().any(|id| aliases.contains(id))
            {
                return Some(format!(
                    "Shared with {} installation '{}'; retained.",
                    other.source.id(),
                    other.package_id
                ));
            }
        }
        for app in &self.launchers {
            if self.owns_launcher(app) {
                continue;
            }
            let names = [
                Some(app.id.as_str()),
                Some(app.name.as_str()),
                app.executable
                    .as_ref()
                    .and_then(|p| p.file_name()?.to_str()),
            ];
            if names
                .into_iter()
                .flatten()
                .map(catalog::normalize)
                .any(|s| aliases.contains(&s))
            {
                return Some(format!(
                    "Another launcher ({}) may use {}; retained.",
                    app.file_path.display(),
                    path.display()
                ));
            }
        }
        None
    }
}

pub(super) async fn collect(target: &InstalledPackage, roots: &Roots) -> Claims {
    match tokio::time::timeout(
        Duration::from_secs(30),
        Box::pin(collect_inner(target, roots)),
    )
    .await
    {
        Ok(claims) => claims,
        Err(_) => Claims {
            warnings: vec![
                "Installation ownership exceeded its request budget; app data is retained.".into(),
            ],
            ..Default::default()
        },
    }
}

async fn collect_inner(target: &InstalledPackage, roots: &Roots) -> Claims {
    let mut claims = Claims {
        complete: true,
        roots: Some(roots.clone()),
        ..Default::default()
    };
    let native = [
        PackageSource::Apt,
        PackageSource::Snap,
        PackageSource::Flatpak,
    ];
    for source in native {
        let (program, args): (&str, &[&str]) = match source {
            PackageSource::Apt => (
                "dpkg-query",
                &["-W", "-f=${binary:Package}\\t${db:Status-Status}\\n"],
            ),
            PackageSource::Snap => ("snap", &["list"]),
            PackageSource::Flatpak => (
                "flatpak",
                &["list", "--app", "--columns=application", "--user"],
            ),
            _ => continue,
        };
        if system::resolve(program).is_none() {
            let state = match source {
                PackageSource::Apt => PathBuf::from("/var/lib/dpkg/status"),
                PackageSource::Snap => PathBuf::from("/var/lib/snapd/state.json"),
                _ => roots.data.join("flatpak"),
            };
            if state.exists()
                || (source == PackageSource::Flatpak && Path::new("/var/lib/flatpak").exists())
            {
                unavailable(
                    &mut claims,
                    format!("{} installation ownership is unavailable.", source.id()),
                );
            }
            continue;
        }
        for scope in if source == PackageSource::Flatpak {
            vec!["--user", "--system"]
        } else {
            vec![""]
        } {
            let args = if source == PackageSource::Flatpak {
                vec!["list", "--app", "--columns=application", scope]
            } else {
                args.to_vec()
            };
            match query(program, &args).await {
                Ok(output) => match parse_native(source, &output, scope) {
                    Ok(packages) => claims.packages.extend(packages),
                    Err(reason) => unavailable(&mut claims, reason),
                },
                Err(reason) => unavailable(
                    &mut claims,
                    format!("{} ownership query failed: {reason}", source.id()),
                ),
            }
        }
    }
    for source in [
        PackageSource::Uv,
        PackageSource::Pipx,
        PackageSource::Cargo,
        PackageSource::Npm,
        PackageSource::Bun,
    ] {
        if system::resolve(tool_managers::manager_name(source)).is_none() {
            continue;
        }
        match tokio::time::timeout(Duration::from_secs(10), tool_managers::inventory(source)).await
        {
            Ok(Ok(packages)) => claims.packages.extend(packages),
            _ => unavailable(
                &mut claims,
                format!("{} tool ownership could not be enumerated.", source.id()),
            ),
        }
    }
    // A selected tool can belong to a non-default root; query that recorded
    // context as well instead of reconstructing it from PATH.
    if let Some(installation) = &target.tool_installation {
        match tokio::time::timeout(
            Duration::from_secs(10),
            tool_managers::inventory_recorded(installation),
        )
        .await
        {
            Ok(Ok(packages)) => {
                for package in packages {
                    if !claims.packages.iter().any(|p| p.key == package.key) {
                        claims.packages.push(package);
                    }
                }
            }
            _ => unavailable(
                &mut claims,
                "Recorded tool-root ownership is unavailable.".into(),
            ),
        }
    }
    let own = owned_paths(target).await;
    match own {
        Ok(paths) => claims.owned_paths = paths,
        Err(error) => unavailable(&mut claims, error),
    }
    let launcher_result = tokio::task::spawn_blocking(collect_launchers).await;
    match launcher_result {
        Ok(Ok(launchers)) => {
            if target.source == PackageSource::Snap {
                for app in &launchers {
                    let raw_program = crate::domain::appimage::desktop::exec_program(&crate::domain::appimage::desktop::unescape(&app.exec));
                    let prefix = format!("/snap/bin/{}", target.package_id);
                    let owned_program = raw_program.as_ref().is_some_and(|program| program == &prefix || program.strip_prefix(&prefix).is_some_and(|suffix| suffix.starts_with('.')));
                    if owned_program && app.file_path.starts_with("/var/lib/snapd/desktop/applications") && app.id.starts_with(&format!("{}_", target.package_id)) {
                        claims.owned_paths.insert(app.file_path.clone());
                    }
                }
            }
            claims.launchers = launchers;
        },
        Ok(Err(reason)) => unavailable(&mut claims, format!("Desktop ownership enumeration is incomplete: {reason}")),
        Err(_) => unavailable(&mut claims, "Desktop ownership worker stopped; hidden and visible launchers could not all be checked.".into()),
    }
    claims
}

fn unavailable(claims: &mut Claims, message: String) {
    claims.complete = false;
    claims.warnings.push(message);
}

async fn query(program: &str, args: &[&str]) -> Result<String, String> {
    let output = system::capture_output_context(
        program,
        args,
        &[("LC_ALL", "C")],
        None,
        Duration::from_secs(8),
    )
    .await
    .map_err(|e| e.to_string())?;
    if !output.success || !output.stderr.trim().is_empty() {
        return Err(output.stderr.trim().to_string());
    }
    if output.stdout.len() > 4 * 1024 * 1024 {
        return Err("Ownership inventory exceeded its bound".into());
    }
    Ok(output.stdout)
}

pub(super) fn parse_native(
    source: PackageSource,
    output: &str,
    scope: &str,
) -> Result<Vec<InstalledPackage>, String> {
    let mut packages = Vec::new();
    if output.trim().is_empty() && source != PackageSource::Flatpak {
        return Err("Native ownership inventory is unexpectedly empty.".into());
    }
    if source == PackageSource::Snap && output.trim() == "No snaps are installed yet." {
        return Ok(packages);
    }
    for (index, line) in output.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let id = if source == PackageSource::Apt {
            let Some((id, status)) = line.split_once('\t') else {
                return Err("APT ownership inventory is malformed.".into());
            };
            match status {
                "installed" | "config-files" | "unpacked" | "half-installed"
                | "half-configured" | "triggers-awaited" | "triggers-pending" => {}
                "not-installed" => continue,
                _ => {
                    return Err("APT ownership inventory contains an unknown package state.".into())
                }
            }
            id.split(':').next().unwrap_or(id)
        } else if source == PackageSource::Snap {
            if index == 0 && line.split_whitespace().next() == Some("Name") {
                continue;
            }
            if line.split_whitespace().count() < 5 {
                return Err("Snap ownership inventory is malformed.".into());
            }
            line.split_whitespace().next().unwrap_or_default()
        } else {
            line.trim()
        };
        if id.is_empty() || id.contains(char::is_whitespace) {
            return Err("Ownership inventory contains an invalid identity.".into());
        }
        let mut pkg = InstalledPackage::new(source, id);
        pkg.name = id.into();
        if source == PackageSource::Flatpak {
            pkg.install_scope = Some(if scope == "--user" {
                crate::domain::package::InstallScope::User
            } else {
                crate::domain::package::InstallScope::System
            });
            pkg.key = format!("flatpak:{}:{id}", scope.trim_start_matches("--"));
        }
        packages.push(pkg);
    }
    Ok(packages)
}

async fn owned_paths(target: &InstalledPackage) -> Result<HashSet<PathBuf>, String> {
    if let Some(app) = &target.appimage_installation {
        return Ok(app.launchers.iter().cloned().collect());
    }
    if let Some(app) = &target.extracted_installation {
        return Ok(app.launchers.iter().cloned().collect());
    }
    let mut paths = HashSet::new();
    match target.source {
        PackageSource::Apt => {
            // Once removed, dpkg -L can fail. Its launchers have then gone;
            // any remaining matching launcher must be treated as a claimant.
            if let Ok(output) = query("dpkg-query", &["-L", &target.package_id]).await {
                paths.extend(output.lines().map(PathBuf::from));
            }
        }
        PackageSource::Flatpak => {
            for dir in desktop_entries::application_dirs() {
                let scope = target.install_scope.map(|s| s.id()).unwrap_or("");
                let matches_scope = (scope == "user"
                    && dir.starts_with(
                        std::env::var_os("HOME")
                            .map(PathBuf::from)
                            .unwrap_or_default(),
                    ))
                    || (scope == "system" && dir.starts_with("/var/lib/flatpak"));
                if matches_scope {
                    paths.insert(dir.join(format!("{}.desktop", target.package_id)));
                }
            }
        }
        PackageSource::Desktop => {
            paths.insert(PathBuf::from(&target.package_id));
        }
        _ => {}
    }
    Ok(paths)
}

fn collect_launchers() -> Result<Vec<desktop_entries::DesktopApp>, String> {
    let mut apps = Vec::new();
    let mut budget = 20_000;
    for dir in desktop_entries::application_dirs() {
        if !dir.try_exists().map_err(|error| error.to_string())? {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir).follow_links(false).max_depth(8) {
            let entry = entry.map_err(|e| e.to_string())?;
            if budget == 0 {
                return Err("Launcher inspection exceeded its bound".into());
            }
            budget -= 1;
            if entry.file_type().is_symlink() && entry.path().is_dir() {
                return Err(
                    "A linked application directory prevents complete ownership inspection.".into(),
                );
            }
            if entry.file_type().is_dir() {
                if entry.depth() == 8
                    && std::fs::read_dir(entry.path())
                        .map_err(|error| error.to_string())?
                        .next()
                        .is_some()
                {
                    return Err("Application ownership inspection exceeded its depth bound.".into());
                }
                continue;
            }
            if entry.path().extension().is_none_or(|ext| ext != "desktop") {
                continue;
            }
            let raw = crate::domain::appimage::desktop::DesktopFile::read(entry.path())
                .map_err(|e| e.to_string())?;
            if raw.entry("Type").is_some_and(|kind| kind != "Application") {
                continue;
            }
            // Hidden=true with no Exec is a valid masking tombstone, not an
            // installed application. Continue scanning lower-precedence roots.
            if raw
                .entry("Hidden")
                .is_some_and(|v| v.eq_ignore_ascii_case("true"))
                && raw.entry("Exec").is_none()
            {
                continue;
            }
            apps.push(parse_claim(entry.path(), &raw)?);
        }
    }
    Ok(apps)
}

fn parse_claim(
    path: &Path,
    raw: &crate::domain::appimage::desktop::DesktopFile,
) -> Result<desktop_entries::DesktopApp, String> {
    if let Some(app) = desktop_entries::parse_launcher(path) {
        return Ok(app);
    }
    // A launcher without Exec (D-Bus activated, a Snap background app, or a
    // hidden entry such as qemu-system-data's qemu.desktop) still claims its
    // name and icon; it just names no executable.
    if raw.entry("Name").is_some() && raw.entry("Exec").is_none() {
        return Ok(desktop_entries::DesktopApp {
            id: path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("Invalid launcher identity")?
                .into(),
            file_path: path.into(),
            name: raw.entry("Name").unwrap_or_default().into(),
            comment: None,
            exec: String::new(),
            executable: None,
            icon: raw.entry("Icon").map(str::to_string),
            categories: Vec::new(),
            terminal: false,
            menu_visible: false,
        });
    }
    Err(format!("Could not parse {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapd_background_launcher_without_exec_remains_an_ownership_claim() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("prompting-client_daemon.desktop");
        let text = "[Desktop Entry]\nType=Application\nX-SnapInstanceName=prompting-client\nNoDisplay=true\nName=Prompting Client\nIcon=prompting-client\n";
        std::fs::write(&path, text).unwrap();
        let raw = crate::domain::appimage::desktop::DesktopFile::read(&path).unwrap();
        let claim = parse_claim(&path, &raw).unwrap();
        assert_eq!(claim.name, "Prompting Client");
        assert!(!claim.menu_visible && claim.executable.is_none());
    }

    #[test]
    fn hidden_launcher_without_exec_claims_its_name_instead_of_failing() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("qemu.desktop");
        let text = "[Desktop Entry]\nVersion=1.0\nName=QEMU\nIcon=qemu\nType=Application\nTerminal=false\nNoDisplay=true\n";
        std::fs::write(&path, text).unwrap();
        let raw = crate::domain::appimage::desktop::DesktopFile::read(&path).unwrap();
        let claim = parse_claim(&path, &raw).unwrap();
        assert_eq!((claim.id.as_str(), claim.name.as_str()), ("qemu", "QEMU"));
        assert_eq!(claim.icon.as_deref(), Some("qemu"));
        assert!(claim.executable.is_none());
    }

    #[test]
    fn launcher_without_a_name_still_fails_closed() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("broken.desktop");
        std::fs::write(&path, "[Desktop Entry]\nType=Application\n").unwrap();
        let raw = crate::domain::appimage::desktop::DesktopFile::read(&path).unwrap();
        assert!(parse_claim(&path, &raw).is_err());
    }

    fn launcher(path: &Path, executable: &str) -> desktop_entries::DesktopApp {
        desktop_entries::DesktopApp {
            id: path.file_stem().unwrap().to_str().unwrap().into(),
            file_path: path.into(),
            name: "Claude".into(),
            comment: None,
            exec: executable.into(),
            executable: Some(PathBuf::from(executable)),
            icon: None,
            categories: Vec::new(),
            terminal: false,
            menu_visible: true,
        }
    }

    #[test]
    fn user_override_of_an_owned_launcher_belongs_to_the_target() {
        let system = Path::new("/usr/share/applications/com.anthropic.Claude.desktop");
        let user = Path::new("/home/u/.local/share/applications/com.anthropic.Claude.desktop");
        let other = Path::new("/home/u/.local/share/applications/other-claude.desktop");
        let claims = Claims {
            owned_paths: HashSet::from([system.into(), PathBuf::from("/usr/bin/claude-desktop")]),
            launchers: vec![
                launcher(system, "/usr/bin/claude-desktop"),
                launcher(user, "/usr/bin/claude-desktop"),
                launcher(other, "/usr/bin/claude-desktop"),
            ],
            ..Default::default()
        };
        assert!(claims.owns_launcher(&claims.launchers[0]));
        assert!(claims.owns_launcher(&claims.launchers[1]));
        // A different desktop ID is another app, even with the same program.
        assert!(!claims.owns_launcher(&claims.launchers[2]));
        // An override that runs something else is not the target's.
        assert!(!claims.owns_launcher(&launcher(user, "/usr/bin/env")));
    }
}
