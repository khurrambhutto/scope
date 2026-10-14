//! Find and verify AppImages integrated by Scope or Gear Lever.
//!
//! Gear Lever treats an AppImage as installed when a launcher in the user's
//! applications directory has a `TryExec` naming a file in `~/AppImages`.
//! Scope uses the same rule, and additionally collects every other launcher
//! that runs the same file so removal does not leave broken menu entries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::desktop::{self, unescape, DesktopFile};
use super::{AppImageInstallation, Roots};

/// Launcher facts for one integrated AppImage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integration {
    pub launcher: PathBuf,
    pub launchers: Vec<PathBuf>,
    pub name: Option<String>,
    pub version: Option<String>,
}

/// Map each integrated AppImage path to its launchers.
pub fn discover(roots: &Roots) -> HashMap<PathBuf, Integration> {
    let mut by_try_exec: HashMap<PathBuf, Vec<(PathBuf, DesktopFile)>> = HashMap::new();
    let mut runners: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&roots.applications) else {
        return HashMap::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "desktop")
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        let Ok(file) = DesktopFile::read(&path) else {
            continue;
        };
        let try_exec = file.entry("TryExec").map(|v| PathBuf::from(unescape(v)));
        let program = file
            .entry("Exec")
            .and_then(|exec| desktop::exec_program(&unescape(exec)))
            .map(PathBuf::from);
        if let Some(target) = program.as_ref().filter(|p| roots.is_managed_location(p)) {
            runners
                .entry(target.clone())
                .or_default()
                .push(path.clone());
        }
        if let Some(target) = try_exec.filter(|p| roots.is_managed_location(p)) {
            if program.as_ref() != Some(&target) {
                runners
                    .entry(target.clone())
                    .or_default()
                    .push(path.clone());
            }
            by_try_exec.entry(target).or_default().push((path, file));
        }
    }
    by_try_exec
        .into_iter()
        .filter_map(|(target, mut candidates)| {
            let stem = target.file_stem()?.to_string_lossy().into_owned();
            // Gear Lever names the launcher after the AppImage file.
            candidates.sort_by_key(|(path, _)| {
                (
                    path.file_stem()
                        .map(|s| s.to_string_lossy() != stem.as_str()),
                    path.clone(),
                )
            });
            let (launcher, file) = candidates.into_iter().next()?;
            let mut launchers = runners.remove(&target).unwrap_or_default();
            if !launchers.contains(&launcher) {
                launchers.push(launcher.clone());
            }
            launchers.sort();
            launchers.dedup();
            let name = file
                .entry("X-AppImage-Name")
                .or_else(|| file.entry("Name"))
                .filter(|n| !n.is_empty())
                .map(str::to_string);
            let version = file
                .entry("X-AppImage-Version")
                .filter(|v| !v.is_empty())
                .map(str::to_string);
            Some((
                target,
                Integration {
                    launcher,
                    launchers,
                    name,
                    version,
                },
            ))
        })
        .collect()
}

/// Build the installation record for an integrated AppImage.
pub fn installation(
    roots: &Roots,
    path: &Path,
    integration: &Integration,
) -> Result<AppImageInstallation> {
    roots.verify_owned_file(path)?;
    let (size, modified_ns) = super::identity(path)?;
    let update_info = super::elf::inspect(path)
        .ok()
        .and_then(|runtime| runtime.update_info);
    Ok(AppImageInstallation {
        path: path.to_path_buf(),
        launcher: integration.launcher.clone(),
        launchers: integration.launchers.clone(),
        icons: icons(roots, path, &integration.launcher),
        size,
        modified_ns,
        update_info,
        update: None,
    })
}

/// Icons in `~/AppImages/.icons` named after the AppImage, as Gear Lever
/// writes them, plus the launcher's `Icon=` when it points into that folder.
fn icons(roots: &Roots, path: &Path, launcher: &Path) -> Vec<PathBuf> {
    let icons_dir = roots.icons();
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = std::fs::read_dir(&icons_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|icon| {
            icon.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .is_some_and(|name| {
                    name == stem
                        || name
                            .strip_prefix(stem.as_str())
                            .is_some_and(|rest| rest.starts_with('.') && !rest[1..].contains('.'))
                })
        })
        .collect();
    if let Some(icon) = DesktopFile::read(launcher)
        .ok()
        .and_then(|file| file.entry("Icon").map(|v| PathBuf::from(unescape(v))))
        .filter(|icon| icon.parent() == Some(icons_dir.as_path()) && is_regular(icon))
    {
        out.push(icon);
    }
    out.sort();
    out.dedup();
    out
}

/// Re-check, right before acting, that the scanned integration still holds.
pub fn verify(roots: &Roots, installation: &AppImageInstallation) -> Result<()> {
    roots.verify_owned_file(&installation.path)?;
    if super::identity(&installation.path)? != (installation.size, installation.modified_ns) {
        bail!("The AppImage changed since the last scan. Rescan and try again.");
    }
    let launcher = DesktopFile::read(&installation.launcher)
        .context("The AppImage launcher is no longer readable. Rescan and try again.")?;
    if launcher.entry("TryExec").map(unescape).as_deref()
        != Some(installation.path.to_string_lossy().as_ref())
    {
        bail!("The AppImage launcher no longer points at this file. Rescan and try again.");
    }
    for path in &installation.launchers {
        if path.parent() != Some(roots.applications.as_path()) || !is_regular(path) {
            bail!("A launcher changed since the last scan. Rescan and try again.");
        }
        let file = DesktopFile::read(path)?;
        let runs = file
            .entry("Exec")
            .and_then(|exec| desktop::exec_program(&unescape(exec)))
            .is_some_and(|program| Path::new(&program) == installation.path)
            || file.entry("TryExec").map(unescape).as_deref()
                == Some(installation.path.to_string_lossy().as_ref());
        if !runs {
            bail!("A launcher changed since the last scan. Rescan and try again.");
        }
    }
    let icons_dir = roots.icons();
    for icon in &installation.icons {
        if icon.parent() != Some(icons_dir.as_path()) || !is_regular(icon) {
            bail!("An AppImage icon changed since the last scan. Rescan and try again.");
        }
    }
    Ok(())
}

fn is_regular(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::appimage::{elf, test_roots};

    fn setup() -> (tempfile::TempDir, Roots, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let app = roots.appimages.join("demo_app.appimage");
        std::fs::write(&app, elf::test_appimage("", b"hsqs")).unwrap();
        (dir, roots, app)
    }

    fn launcher(roots: &Roots, name: &str, body: &str) -> PathBuf {
        let path = roots.applications.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn finds_gear_lever_launchers_and_every_launcher_that_runs_the_file() {
        let (_dir, roots, app) = setup();
        let app_text = app.display().to_string();
        let primary = launcher(
            &roots,
            "demo_app.desktop",
            &format!("[Desktop Entry]\nType=Application\nName=Demo\nTryExec={app_text}\nExec=env DESKTOPINTEGRATION=1 {app_text} %U\nX-AppImage-Version=1.0\nX-AppImage-Name=Demo App\n"),
        );
        let extra = launcher(
            &roots,
            "demo-url.desktop",
            &format!("[Desktop Entry]\nType=Application\nName=Demo URL\nExec=\"{app_text}\" %U\n"),
        );
        launcher(
            &roots,
            "other.desktop",
            "[Desktop Entry]\nType=Application\nName=Other\nExec=/usr/bin/other\n",
        );
        std::fs::write(roots.icons().join("demo_app"), b"png").unwrap();
        std::fs::write(roots.icons().join("demo_app_2"), b"png").unwrap();

        let found = discover(&roots);
        let integration = &found[&app];
        assert_eq!(integration.launcher, primary);
        assert_eq!(integration.launchers, vec![extra, primary]);
        assert_eq!(integration.name.as_deref(), Some("Demo App"));
        assert_eq!(integration.version.as_deref(), Some("1.0"));

        let installation = installation(&roots, &app, integration).unwrap();
        assert_eq!(installation.icons, vec![roots.icons().join("demo_app")]);
        assert!(verify(&roots, &installation).is_ok());
    }

    #[test]
    fn launchers_without_try_exec_do_not_make_an_appimage_managed() {
        let (_dir, roots, app) = setup();
        launcher(
            &roots,
            "demo.desktop",
            &format!(
                "[Desktop Entry]\nType=Application\nName=Demo\nExec={}\n",
                app.display()
            ),
        );
        assert!(discover(&roots).is_empty());
    }

    #[test]
    fn appimages_outside_the_managed_folder_are_ignored() {
        let (dir, roots, _) = setup();
        let elsewhere = dir.path().join("Downloads/demo.appimage");
        launcher(
            &roots,
            "demo.desktop",
            &format!(
                "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
                elsewhere.display()
            ),
        );
        assert!(discover(&roots).is_empty());
    }

    #[test]
    fn verification_rejects_a_replaced_file_or_retargeted_launcher() {
        let (_dir, roots, app) = setup();
        let primary = launcher(
            &roots,
            "demo_app.desktop",
            &format!(
                "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
                app.display()
            ),
        );
        let integration = discover(&roots).remove(&app).unwrap();
        let installation = installation(&roots, &app, &integration).unwrap();

        std::fs::write(
            &primary,
            "[Desktop Entry]\nType=Application\nName=Demo\nTryExec=/usr/bin/x\nExec=/usr/bin/x\n",
        )
        .unwrap();
        assert!(verify(&roots, &installation).is_err());

        std::fs::write(
            &primary,
            format!(
                "[Desktop Entry]\nType=Application\nName=Demo\nTryExec={0}\nExec={0}\n",
                app.display()
            ),
        )
        .unwrap();
        assert!(verify(&roots, &installation).is_ok());
        std::fs::write(&app, elf::test_appimage("changed", b"hsqs-longer")).unwrap();
        assert!(verify(&roots, &installation).is_err());
    }
}
