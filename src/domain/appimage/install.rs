//! Install an opened AppImage the way Gear Lever does.
//!
//! The file moves into `~/AppImages` under a name derived from its launcher
//! (`Demo App` becomes `demo_app.appimage`), its icon is saved as
//! `~/AppImages/.icons/demo_app.<ext>`, and its launcher is rewritten into the
//! user's applications directory as `demo_app.desktop`. Installing an app that
//! is already integrated under the same name replaces it in place, keeping
//! its file name, launcher, and launch settings.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use super::desktop::{self, LaunchSettings};
use super::extract::{self, Metadata};
use super::{managed, AppImageInstallation, Roots};

/// Everything the install preview showed, checked again before applying.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPlan {
    pub source: PathBuf,
    pub source_size: u64,
    pub source_modified_ns: u128,
    pub source_sha256: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub destination: PathBuf,
    pub launcher: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<PathBuf>,
    /// The integrated app this install replaces, when names match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replaces: Option<Box<AppImageInstallation>>,
    /// Remove the opened file once its copy is in place (Gear Lever's move).
    pub move_source: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_source: Option<String>,
    pub notes: Vec<String>,
}

/// Inspect an opened AppImage and plan its integration.
pub async fn preview(source: &Path) -> Result<InstallPlan> {
    let roots = Roots::current()?;
    let source = std::path::absolute(source)?;
    let metadata = std::fs::symlink_metadata(&source)
        .with_context(|| format!("{} could not be opened", source.display()))?;
    if !metadata.file_type().is_file() {
        bail!("{} is not a regular file", source.display());
    }
    let (source_size, source_modified_ns) = super::identity(&source)?;
    let integrations = managed::discover(&roots);
    if integrations.contains_key(&source) {
        bail!("This AppImage is already installed. Find it in the list to update or remove it.");
    }
    let meta = extract::read(&source).await?;
    let host = super::host_architecture();
    if meta.runtime.architecture != host && meta.runtime.architecture != "unknown" {
        bail!(
            "This AppImage is built for {}, but this computer is {host}.",
            meta.runtime.architecture
        );
    }
    let hashed = source.clone();
    let source_sha256 = tokio::task::spawn_blocking(move || super::hashes::compute(&hashed))
        .await
        .context("Hash worker stopped")??
        .sha256;

    let mut replaces = None;
    for (path, integration) in &integrations {
        let Ok((size, _)) = super::identity(path) else {
            continue;
        };
        if size == source_size
            && super::hashes::digests(path).is_ok_and(|d| d.sha256 == source_sha256)
        {
            bail!("This AppImage is already installed as {}.", path.display());
        }
        if integration
            .name
            .as_deref()
            .is_some_and(|name| name.eq_ignore_ascii_case(&meta.name))
        {
            // An integration that no longer verifies is left alone; the new
            // copy installs under a fresh name instead.
            if let Ok(existing) = managed::installation(&roots, path, integration) {
                replaces = Some(Box::new(existing));
            }
        }
    }

    let in_place = roots.is_managed_location(&source);
    let (destination, launcher) = match &replaces {
        Some(existing) => (existing.path.clone(), existing.launcher.clone()),
        None if in_place => {
            let stem = source
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            (
                source.clone(),
                roots.applications.join(format!("{stem}.desktop")),
            )
        }
        None => free_names(&roots, &meta)?,
    };
    if replaces.is_none() && launcher.exists() {
        bail!(
            "A launcher already exists at {}. Remove it before installing.",
            launcher.display()
        );
    }
    let icon = meta.icon.as_ref().map(|icon| {
        let stem = destination
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        roots.icons().join(format!("{stem}.{}", icon.extension))
    });

    let mut notes = Vec::new();
    let update_source = match meta.runtime.update_info.as_deref() {
        None => {
            notes.push(
                "This AppImage does not publish update information, so Scope cannot update it."
                    .into(),
            );
            None
        }
        Some(info) => match super::updates::Source::parse(info) {
            Ok(source) => Some(source.describe()),
            Err(error) => {
                notes.push(format!("{error}; Scope cannot update it."));
                None
            }
        },
    };
    if !super::fuse2_available() {
        notes
            .push("FUSE 2 (libfuse2t64) is not installed. Most AppImages need it to start.".into());
    }
    if let Some(existing) = &replaces {
        notes.push(format!(
            "Replaces the installed copy at {}, keeping its launcher settings.",
            existing.path.display()
        ));
    }

    Ok(InstallPlan {
        move_source: source != destination,
        source,
        source_size,
        source_modified_ns,
        source_sha256,
        name: meta.name,
        version: meta.version,
        destination,
        launcher,
        icon,
        replaces,
        update_source,
        notes,
    })
}

/// Gear Lever's naming: the launcher name, lowercased, spaces to `_`, other
/// special characters dropped, then `_<version>` or `_<n>` on collisions.
fn free_names(roots: &Roots, meta: &Metadata) -> Result<(PathBuf, PathBuf)> {
    let base = slug(&meta.name);
    let mut candidates = vec![base.clone()];
    if let Some(version) = &meta.version {
        candidates.push(format!("{base}_{}", slug(&version.replace('.', "_"))));
    }
    candidates.extend((1..100).map(|i| format!("{base}_{i}")));
    for stem in candidates {
        let destination = roots.appimages.join(format!("{stem}.appimage"));
        let launcher = roots.applications.join(format!("{stem}.desktop"));
        if std::fs::symlink_metadata(&destination).is_err()
            && std::fs::symlink_metadata(&launcher).is_err()
        {
            return Ok((destination, launcher));
        }
    }
    bail!("Could not find a free file name in ~/AppImages")
}

pub fn slug(name: &str) -> String {
    let slug: String = name
        .to_lowercase()
        .replace(' ', "_")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        .take(64)
        .collect();
    let slug = slug.trim_matches('.').to_string();
    if slug.is_empty() {
        "appimage".into()
    } else {
        slug
    }
}

/// The plan is stale when the opened file changed since the preview.
pub fn revalidate(plan: &InstallPlan) -> Result<()> {
    if super::identity(&plan.source)? != (plan.source_size, plan.source_modified_ns) {
        bail!("The AppImage changed since the preview. Open it again.");
    }
    let roots = Roots::current()?;
    match &plan.replaces {
        Some(existing) => managed::verify(&roots, existing)?,
        None => {
            if plan.destination != plan.source && plan.destination.exists() {
                bail!(
                    "{} already exists. Open the AppImage again.",
                    plan.destination.display()
                );
            }
            if plan.launcher.exists() {
                bail!(
                    "{} already exists. Open the AppImage again.",
                    plan.launcher.display()
                );
            }
        }
    }
    Ok(())
}

pub async fn apply(plan: &InstallPlan, on_line: &(dyn Fn(&str) + Send + Sync)) -> Result<String> {
    let roots = Roots::current()?;
    let meta = extract::read(&plan.source).await?;
    on_line("[scope] read the AppImage launcher and icon");
    integrate(
        &roots,
        &plan.source,
        &plan.source_sha256,
        &meta,
        &Target {
            destination: plan.destination.clone(),
            launcher: plan.launcher.clone(),
            replaces: plan.replaces.as_deref(),
        },
        on_line,
    )
    .await?;
    if plan.move_source {
        std::fs::remove_file(&plan.source).with_context(|| {
            format!(
                "Installed, but could not remove the original {}",
                plan.source.display()
            )
        })?;
        on_line("[scope] moved the original into ~/AppImages");
    }
    super::refresh_launchers(&roots, on_line).await;
    Ok(format!("{} was installed to ~/AppImages", plan.name))
}

pub(super) struct Target<'a> {
    pub destination: PathBuf,
    pub launcher: PathBuf,
    pub replaces: Option<&'a AppImageInstallation>,
}

/// Place a verified AppImage, its icon, and its launcher.
pub(super) async fn integrate(
    roots: &Roots,
    source: &Path,
    expected_sha256: &str,
    meta: &Metadata,
    target: &Target<'_>,
    on_line: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    std::fs::create_dir_all(&roots.appimages)?;
    std::fs::create_dir_all(&roots.applications)?;
    let previous = match target.replaces {
        Some(existing) => Some(
            desktop::DesktopFile::read(&existing.launcher)?
                .entry("Exec")
                .map(|exec| LaunchSettings::from_exec(&desktop::unescape(exec)))
                .unwrap_or_default(),
        ),
        None => None,
    };

    if source != target.destination {
        let source = source.to_path_buf();
        let destination = target.destination.clone();
        let appimages = roots.appimages.clone();
        let expected = expected_sha256.to_string();
        let replace = target.replaces.is_some();
        tokio::task::spawn_blocking(move || {
            place_appimage(&source, &appimages, &destination, &expected, replace)
        })
        .await
        .context("Copy worker stopped")??;
    } else {
        if super::hashes::compute(source)?.sha256 != expected_sha256 {
            bail!("The AppImage changed since the preview. Open it again.");
        }
        set_mode(source, 0o755)?;
    }
    on_line(&format!("[scope] placed {}", target.destination.display()));

    let icon = match &meta.icon {
        Some(icon) => {
            let stem = target
                .destination
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let path = roots.icons().join(format!("{stem}.{}", icon.extension));
            write_atomic(&roots.icons(), &path, &icon.bytes, 0o644)?;
            Some(path)
        }
        None => None,
    };
    let text = desktop::integrated_launcher(
        &meta.launcher,
        &target.destination,
        icon.as_deref(),
        meta.version.as_deref(),
        previous.as_ref(),
    )?;
    write_atomic(
        &roots.applications,
        &target.launcher,
        text.as_bytes(),
        0o644,
    )?;
    on_line(&format!(
        "[scope] wrote launcher {}",
        target.launcher.display()
    ));

    if let Some(existing) = target.replaces {
        for old in existing
            .icons
            .iter()
            .filter(|old| Some(*old) != icon.as_ref())
        {
            let _ = crate::domain::system::trash::trash(old).await;
        }
    }
    Ok(())
}

/// Copy into a hidden temporary file beside the destination while hashing,
/// then rename it into place so a running copy is never half-written.
fn place_appimage(
    source: &Path,
    appimages: &Path,
    destination: &Path,
    expected_sha256: &str,
    replace: bool,
) -> Result<()> {
    let mut input = std::fs::File::open(source)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".scope-install-")
        .tempfile_in(appimages)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        temp.write_all(&buffer[..read])?;
    }
    if super::hashes::hex(&hasher.finalize()) != expected_sha256 {
        bail!("The AppImage changed since it was checked. Nothing was installed.");
    }
    temp.as_file().sync_all()?;
    set_mode(temp.path(), 0o755)?;
    if replace {
        temp.persist(destination)?;
    } else {
        temp.persist_noclobber(destination).map_err(|e| {
            anyhow::anyhow!("{} already exists: {}", destination.display(), e.error)
        })?;
    }
    Ok(())
}

fn write_atomic(dir: &Path, path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut temp = tempfile::Builder::new()
        .prefix(".scope-")
        .tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    set_mode(temp.path(), mode)?;
    temp.persist(path)?;
    Ok(())
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::appimage::{elf, test_roots};

    #[test]
    fn slugs_follow_gear_lever_naming() {
        assert_eq!(slug("T3 Code (Nightly)"), "t3_code_nightly");
        assert_eq!(slug("OpenResearch"), "openresearch");
        assert_eq!(slug("../.."), "appimage");
        assert_eq!(slug("Ünïcode App"), "ncode_app");
    }

    fn meta(name: &str, version: Option<&str>) -> Metadata {
        Metadata {
            runtime: elf::RuntimeInfo {
                payload_offset: 0,
                update_info: None,
                architecture: "x86_64",
            },
            launcher: desktop::DesktopFile::parse(&format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec=AppRun %U\nIcon=demo\n"
            )),
            name: name.into(),
            version: version.map(str::to_string),
            icon: Some(extract::Icon {
                bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
                extension: "png",
            }),
        }
    }

    #[test]
    fn free_names_skip_existing_files_and_launchers() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let m = meta("Demo App", Some("1.2"));
        assert_eq!(
            free_names(&roots, &m).unwrap().0,
            roots.appimages.join("demo_app.appimage")
        );
        std::fs::write(roots.appimages.join("demo_app.appimage"), b"x").unwrap();
        assert_eq!(
            free_names(&roots, &m).unwrap().0,
            roots.appimages.join("demo_app_1_2.appimage")
        );
        std::fs::write(roots.applications.join("demo_app_1_2.desktop"), b"x").unwrap();
        assert_eq!(
            free_names(&roots, &m).unwrap().0,
            roots.appimages.join("demo_app_1.appimage")
        );
    }

    #[tokio::test]
    async fn integrate_places_file_icon_and_a_discoverable_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let source = dir.path().join("Demo-1.2-x86_64.AppImage");
        std::fs::write(&source, elf::test_appimage("", b"hsqs")).unwrap();
        let sha = super::super::hashes::compute(&source).unwrap().sha256;
        let target = Target {
            destination: roots.appimages.join("demo_app.appimage"),
            launcher: roots.applications.join("demo_app.desktop"),
            replaces: None,
        };

        integrate(
            &roots,
            &source,
            &sha,
            &meta("Demo App", Some("1.2")),
            &target,
            &|_| {},
        )
        .await
        .unwrap();

        assert!(
            source.exists(),
            "the caller removes the source after success"
        );
        let found = managed::discover(&roots);
        let integration = &found[&target.destination];
        assert_eq!(integration.launcher, target.launcher);
        assert_eq!(integration.version.as_deref(), Some("1.2"));
        assert!(roots.icons().join("demo_app.png").exists());
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&target.destination)
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }

    #[tokio::test]
    async fn integrate_refuses_bytes_that_differ_from_the_preview() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let source = dir.path().join("demo.AppImage");
        std::fs::write(&source, elf::test_appimage("", b"hsqs")).unwrap();
        let target = Target {
            destination: roots.appimages.join("demo.appimage"),
            launcher: roots.applications.join("demo.desktop"),
            replaces: None,
        };
        let result = integrate(
            &roots,
            &source,
            &"0".repeat(64),
            &meta("Demo", None),
            &target,
            &|_| {},
        )
        .await;
        assert!(result.is_err());
        assert!(!target.destination.exists());
        assert!(!target.launcher.exists());
    }
}
