//! Discovery-only local files. Never launch programs or infer data ownership.

use anyhow::{bail, Result};
use std::path::Path;

use super::{
    targets::{Discovery, Target},
    DetailEffect,
};
use crate::domain::{
    desktop_entries,
    package::{InstalledPackage, PackageSource},
    scanner::appimage,
};

pub(super) fn discover(pkg: &InstalledPackage) -> Result<Discovery> {
    let path = Path::new(&pkg.package_id);
    if !path.is_absolute() {
        bail!("Local app location is not an absolute path");
    }
    let mut discovery = Discovery::default();
    match pkg.source {
        PackageSource::AppImage => {
            if !appimage::is_appimage(path) {
                bail!("The detected file is no longer a valid AppImage");
            }
            // Scope/Gear Lever integrations own their file, launchers, and
            // icons; anything else is only discovered.
            let installation = pkg.appimage_installation.as_deref();
            let (owned, sidecar_effect) = match installation {
                Some(_) => (DetailEffect::Removed, DetailEffect::Kept),
                None => (DetailEffect::Informational, DetailEffect::Informational),
            };
            discovery.add(
                "AppImage",
                path.display().to_string(),
                owned,
                Target::File(path.to_path_buf()),
            );
            if let Some(installation) = installation {
                for launcher in &installation.launchers {
                    discovery.add(
                        "Launcher",
                        launcher.display().to_string(),
                        DetailEffect::Removed,
                        Target::File(launcher.clone()),
                    );
                }
                for icon in &installation.icons {
                    discovery.add(
                        "Icon",
                        icon.display().to_string(),
                        DetailEffect::Removed,
                        Target::File(icon.clone()),
                    );
                }
            }
            // AppImageKit's documented portable mode uses these exact sidecars:
            // https://docs.appimage.org/user-guide/portable-mode.html
            for (suffix, label) in [(".home", "Portable data"), (".config", "Portable config")] {
                let mut sidecar = path.as_os_str().to_os_string();
                sidecar.push(suffix);
                let sidecar = std::path::PathBuf::from(sidecar);
                if !sidecar
                    .symlink_metadata()
                    .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
                {
                    discovery.add(
                        label,
                        sidecar.display().to_string(),
                        sidecar_effect,
                        Target::Directory(sidecar),
                    );
                }
            }
            discovery.unknown(
                "User data",
                "AppImage files do not identify external app data",
            );
        }
        PackageSource::Desktop => {
            discovery.add(
                "Launcher",
                path.display().to_string(),
                DetailEffect::Informational,
                Target::File(path.to_path_buf()),
            );
            let app = desktop_entries::parse_launcher(path)
                .ok_or_else(|| anyhow::anyhow!("Desktop launcher could not be read"))?;
            if let Some(executable) = app.executable.and_then(|path| path.canonicalize().ok()) {
                discovery.add(
                    "Executable",
                    executable.display().to_string(),
                    DetailEffect::Informational,
                    Target::File(executable),
                );
            } else {
                discovery.unknown("Executable", "Launcher executable could not be resolved");
            }
            discovery.unknown(
                "User data",
                "This launcher does not identify app-created files",
            );
        }
        _ => bail!("Not a local discovery source"),
    }
    Ok(discovery)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_rows_are_informational_and_do_not_execute_the_launcher() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("app");
        let marker = dir.path().join("ran");
        std::fs::write(&script, format!("#!/bin/sh\ntouch {}\n", marker.display())).unwrap();
        let launcher = dir.path().join("app.desktop");
        std::fs::write(
            &launcher,
            format!(
                "[Desktop Entry]\nType=Application\nName=Test\nExec={}\n",
                script.display()
            ),
        )
        .unwrap();
        let pkg = InstalledPackage::new(PackageSource::Desktop, launcher.display().to_string());
        let result = discover(&pkg).unwrap();
        assert!(result
            .details
            .items
            .iter()
            .all(|i| i.effect == DetailEffect::Informational));
        assert_eq!(result.details.items[1].label, "Executable");
        assert!(!marker.exists());
    }
    #[test]
    fn valid_appimage_is_informational_and_invalid_images_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("test.AppImage");
        std::fs::write(&image, b"\x7fELF\0\0\0\0AI\x02").unwrap();
        let pkg = InstalledPackage::new(PackageSource::AppImage, image.display().to_string());
        std::fs::create_dir(dir.path().join("test.AppImage.home")).unwrap();
        assert_eq!(
            discover(&pkg).unwrap().details.items[0].effect,
            DetailEffect::Informational
        );
        assert!(discover(&pkg)
            .unwrap()
            .details
            .items
            .iter()
            .any(|i| i.label == "Portable data" && i.effect == DetailEffect::Informational));
        std::fs::write(image, b"not an AppImage").unwrap();
        assert!(discover(&pkg).is_err());
    }

    #[test]
    fn integrated_appimages_report_removed_files_and_kept_portable_data() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("demo.appimage");
        std::fs::write(&image, b"\x7fELF\0\0\0\0AI\x02").unwrap();
        std::fs::create_dir(dir.path().join("demo.appimage.config")).unwrap();
        let launcher = dir.path().join("demo.desktop");
        std::fs::write(&launcher, b"[Desktop Entry]\n").unwrap();
        let mut pkg = InstalledPackage::new(PackageSource::AppImage, image.display().to_string());
        pkg.appimage_installation = Some(Box::new(crate::domain::appimage::AppImageInstallation {
            path: image,
            launcher: launcher.clone(),
            launchers: vec![launcher],
            icons: vec![],
            size: 0,
            modified_ns: 0,
            update_info: None,
            update: None,
        }));
        let items = discover(&pkg).unwrap().details.items;
        assert_eq!(items[0].effect, DetailEffect::Removed);
        assert!(items
            .iter()
            .any(|i| i.label == "Launcher" && i.effect == DetailEffect::Removed));
        assert!(items
            .iter()
            .any(|i| i.label == "Portable config" && i.effect == DetailEffect::Kept));
    }
}
