//! Gear Lever-compatible AppImage integration.
//!
//! Scope installs, updates, and removes AppImages the way Gear Lever does: the
//! file lives directly in `~/AppImages`, a launcher in the user's applications
//! directory points at it through `Exec`, and its icon sits in
//! `~/AppImages/.icons`. Either tool can therefore manage what the other
//! integrated. AppImages anywhere else stay discovery-only.

pub mod app_data;
pub mod desktop;
pub mod elf;
pub mod extract;
pub mod hashes;
pub mod install;
#[cfg(test)]
mod live_tests;
pub mod managed;
pub mod metadata;
pub mod operations;
mod transaction;
pub mod updates;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Verified Gear Lever-style integration captured during a scan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppImageInstallation {
    /// The AppImage, a direct child of `~/AppImages`.
    pub path: PathBuf,
    /// The launcher whose direct `Exec` names `path`; rewritten on update.
    pub launcher: PathBuf,
    /// Every user launcher that runs `path`; all of them break on removal.
    pub launchers: Vec<PathBuf>,
    /// Icons attributable to this AppImage inside `~/AppImages/.icons`.
    pub icons: Vec<PathBuf>,
    /// File identity at scan time, used to reject stale plans.
    pub size: u64,
    pub modified_ns: u128,
    /// Raw `.upd_info` text embedded in the AppImage, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update_info: Option<String>,
    /// Resolved newer release, set only when a published digest differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<updates::AppImageUpdate>,
}

/// The directories Scope integrates into. Passed explicitly so tests never
/// touch the process-wide HOME.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    pub home: PathBuf,
    /// `~/AppImages`, Gear Lever's default destination.
    pub appimages: PathBuf,
    /// The user's applications directory for launchers.
    pub applications: PathBuf,
}

impl Roots {
    pub fn current() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .context("HOME is not set")?;
        let applications = match std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            Some(data) if data.is_absolute() => data.join("applications"),
            _ => home.join(".local/share/applications"),
        };
        Ok(Self::new(home, applications))
    }

    pub fn new(home: PathBuf, applications: PathBuf) -> Self {
        Self {
            appimages: home.join("AppImages"),
            home,
            applications,
        }
    }

    /// `~/AppImages/.icons`, where Gear Lever stores extracted icons.
    pub fn icons(&self) -> PathBuf {
        self.appimages.join(".icons")
    }

    /// True when `path` is lexically a visible direct child of `~/AppImages`.
    pub fn is_managed_location(&self, path: &Path) -> bool {
        path.is_absolute()
            && path.parent() == Some(self.appimages.as_path())
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.'))
            && path.components().all(|component| {
                matches!(
                    component,
                    std::path::Component::RootDir | std::path::Component::Normal(_)
                )
            })
    }

    /// Require a user-owned, regular, valid type 2 AppImage at a managed location.
    pub fn verify_owned_file(&self, path: &Path) -> Result<()> {
        if !self.is_managed_location(path) {
            bail!("Scope only manages AppImages kept directly in ~/AppImages");
        }
        if self.appimages.canonicalize()? != self.home.canonicalize()?.join("AppImages") {
            bail!("Scope cannot manage a symlinked AppImages directory");
        }
        if !self.is_owned_regular(path) {
            bail!("{} is not a regular file owned by you", path.display());
        }
        if elf::appimage_type(path) != Some(elf::AppImageType::Two) {
            bail!("{} is no longer a valid AppImage", path.display());
        }
        Ok(())
    }

    pub(super) fn is_owned_regular(&self, path: &Path) -> bool {
        use std::os::unix::fs::MetadataExt as _;
        let Ok(home) = std::fs::metadata(&self.home) else {
            return false;
        };
        std::fs::symlink_metadata(path)
            .is_ok_and(|metadata| metadata.file_type().is_file() && metadata.uid() == home.uid())
    }
}

/// File size and modification time, used as a cheap identity check.
pub fn identity(path: &Path) -> Result<(u64, u128)> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("{} is no longer present", path.display()))?;
    if !metadata.file_type().is_file() {
        bail!("{} is not a regular file", path.display());
    }
    let modified = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok((metadata.len(), modified))
}

/// Machine architecture label matching `elf::RuntimeInfo::architecture`.
pub fn host_architecture() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        "x86" => "i686",
        "arm" => "armhf",
        _ => "unknown",
    }
}

/// AppImages built on FUSE 2 fail to start without `libfuse.so.2`.
pub fn fuse2_available() -> bool {
    [
        "/usr/lib/x86_64-linux-gnu",
        "/lib/x86_64-linux-gnu",
        "/usr/lib/aarch64-linux-gnu",
        "/lib/aarch64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib",
    ]
    .iter()
    .any(|dir| Path::new(dir).join("libfuse.so.2").exists())
}

/// Ask the desktop to reread launchers. Failure only delays menu refresh.
pub async fn refresh_launchers(roots: &Roots, on_line: &(dyn Fn(&str) + Send + Sync)) {
    if !crate::domain::system::which("update-desktop-database") {
        return;
    }
    let dir = roots.applications.to_string_lossy().into_owned();
    let program = crate::domain::system::abs("update-desktop-database");
    if let Ok(out) = crate::domain::system::capture_output(
        &program,
        &["-q", &dir],
        std::time::Duration::from_secs(30),
    )
    .await
    {
        if !out.success {
            on_line(
                "[scope] update-desktop-database reported an error; the menu may refresh later",
            );
        }
    }
}

#[cfg(test)]
pub(crate) fn test_roots(dir: &Path) -> Roots {
    let roots = Roots::new(dir.to_path_buf(), dir.join(".local/share/applications"));
    std::fs::create_dir_all(roots.icons()).unwrap();
    std::fs::create_dir_all(&roots.applications).unwrap();
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_location_is_a_visible_direct_child_of_appimages() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let apps = &roots.appimages;
        assert!(roots.is_managed_location(&apps.join("app.appimage")));
        assert!(!roots.is_managed_location(&apps.join(".scope-update/app.appimage")));
        assert!(!roots.is_managed_location(&apps.join(".hidden.appimage")));
        assert!(!roots.is_managed_location(&apps.join("sub/app.appimage")));
        assert!(!roots.is_managed_location(&apps.join("../AppImages/app.appimage")));
        assert!(!roots.is_managed_location(&dir.path().join("Downloads/app.appimage")));
        assert!(!roots.is_managed_location(Path::new("AppImages/app.appimage")));
    }

    #[test]
    fn owned_file_must_be_a_regular_type_two_appimage() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let app = roots.appimages.join("app.appimage");
        std::fs::write(&app, elf::test_appimage("", b"hsqs")).unwrap();
        assert!(roots.verify_owned_file(&app).is_ok());

        let link = roots.appimages.join("link.appimage");
        std::os::unix::fs::symlink(&app, &link).unwrap();
        assert!(roots.verify_owned_file(&link).is_err());

        std::fs::write(&app, b"not an appimage").unwrap();
        assert!(roots.verify_owned_file(&app).is_err());
    }

    #[test]
    fn a_symlinked_appimages_directory_is_not_a_managed_root() {
        let dir = tempfile::tempdir().unwrap();
        let roots = test_roots(dir.path());
        let app = roots.appimages.join("demo.appimage");
        std::fs::write(&app, elf::test_appimage("", b"hsqs")).unwrap();
        let elsewhere = dir.path().join("elsewhere");
        std::fs::rename(&roots.appimages, &elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, &roots.appimages).unwrap();
        assert!(roots.verify_owned_file(&app).is_err());
    }
}
