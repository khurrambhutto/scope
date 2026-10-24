//! Find and verify AppImages integrated by Scope or Gear Lever.
//!
//! Accept a direct `Exec` into `~/AppImages`, with a matching `TryExec` when
//! present. Apps can rewrite their launchers and drop Gear Lever's extra keys.
//! Collect every launcher that runs the file so removal leaves no broken entries.

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
    let mut candidates: HashMap<PathBuf, Vec<(PathBuf, DesktopFile)>> = HashMap::new();
    let Ok(entries) = std::fs::read_dir(&roots.applications) else {
        return HashMap::new();
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "desktop") || !roots.is_owned_regular(&path) {
            continue;
        }
        let Ok(file) = DesktopFile::read(&path) else {
            continue;
        };
        let Some(target) = launcher_target(roots, &file) else {
            continue;
        };
        candidates.entry(target).or_default().push((path, file));
    }
    candidates
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
            let mut launchers: Vec<_> = candidates.iter().map(|(path, _)| path.clone()).collect();
            launchers.sort();
            launchers.dedup();
            let (launcher, file) = candidates.into_iter().next()?;
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

fn launcher_target(roots: &Roots, file: &DesktopFile) -> Option<PathBuf> {
    if file.entry("Type") != Some("Application") {
        return None;
    }
    let program = PathBuf::from(desktop::exec_program(&unescape(file.entry("Exec")?))?);
    if !roots.is_managed_location(&program) {
        return None;
    }
    if file
        .entry("TryExec")
        .filter(|value| !value.is_empty())
        .is_some_and(|value| Path::new(&unescape(value)) != program)
    {
        return None;
    }
    Some(program)
}

/// Build the installation record for an integrated AppImage.
pub fn installation(
    roots: &Roots,
    path: &Path,
    integration: &Integration,
) -> Result<AppImageInstallation> {
    roots.verify_owned_file(path)?;
    if integration.launcher.parent() != Some(roots.applications.as_path())
        || !roots.is_owned_regular(&integration.launcher)
    {
        bail!("The AppImage launcher is not owned by you in the applications directory");
    }
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
    if icons_dir.canonicalize().ok() != Some(roots.appimages.join(".icons")) {
        return Vec::new();
    }
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = std::fs::read_dir(&icons_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|icon| roots.is_owned_regular(icon))
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
        .filter(|icon| icon.parent() == Some(icons_dir.as_path()) && roots.is_owned_regular(icon))
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
    if installation.launcher.parent() != Some(roots.applications.as_path())
        || !roots.is_owned_regular(&installation.launcher)
        || !installation.launchers.contains(&installation.launcher)
    {
        bail!("The AppImage launcher changed. Rescan and try again.");
    }
    let launcher = DesktopFile::read(&installation.launcher)
        .context("The AppImage launcher is no longer readable. Rescan and try again.")?;
    if launcher_target(roots, &launcher).as_ref() != Some(&installation.path) {
        bail!("The AppImage launcher no longer points at this file. Rescan and try again.");
    }
    for path in &installation.launchers {
        if path.parent() != Some(roots.applications.as_path()) || !roots.is_owned_regular(path) {
            bail!("A launcher changed since the last scan. Rescan and try again.");
        }
        let file = DesktopFile::read(path)?;
        if launcher_target(roots, &file).as_ref() != Some(&installation.path) {
            bail!("A launcher changed since the last scan. Rescan and try again.");
        }
    }
    let icons_dir = roots.icons();
    if !installation.icons.is_empty()
        && icons_dir.canonicalize().ok() != Some(roots.appimages.join(".icons"))
    {
        bail!("The AppImage icon directory changed. Rescan and try again.");
    }
    for icon in &installation.icons {
        if icon.parent() != Some(icons_dir.as_path()) || !roots.is_owned_regular(icon) {
            bail!("An AppImage icon changed since the last scan. Rescan and try again.");
        }
    }
    let current = discover(roots)
        .remove(&installation.path)
        .context("The AppImage integration changed. Rescan and try again.")?;
    if current.launchers != installation.launchers
        || icons(roots, &installation.path, &current.launcher) != installation.icons
    {
        bail!("The AppImage launchers or icons changed. Rescan and try again.");
    }
    Ok(())
}

#[cfg(test)]
#[path = "managed_tests.rs"]
mod tests;
