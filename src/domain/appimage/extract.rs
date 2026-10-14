//! Read an AppImage's launcher and icon without running it.
//!
//! `unsquashfs` reads the SquashFS payload after the ELF runtime and extracts
//! only the root launcher, icons, and the files their in-image symlinks name.
//! Links are resolved by reading their text and extracting the target path
//! again, so nothing outside the private temporary directory is ever read.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::desktop::DesktopFile;
use super::elf::{self, AppImageType, RuntimeInfo};

const EXTRACT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_ICON_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LINK_HOPS: usize = 6;

/// Everything integration needs from an AppImage.
#[derive(Debug, Clone)]
pub struct Metadata {
    pub runtime: RuntimeInfo,
    pub launcher: DesktopFile,
    pub name: String,
    pub version: Option<String>,
    pub icon: Option<Icon>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Icon {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Inspect an AppImage's runtime and extract its launcher and icon.
pub async fn read(path: &Path) -> Result<Metadata> {
    match elf::appimage_type(path) {
        Some(AppImageType::Two) => {}
        Some(AppImageType::One) => bail!("Type 1 AppImages are not supported"),
        None => bail!("This file is not an AppImage"),
    }
    let runtime = elf::inspect(path)?;
    let unsquashfs = crate::domain::system::resolve("unsquashfs")
        .context("Reading AppImage launchers needs unsquashfs (the squashfs-tools package)")?;
    let temp = tempfile::Builder::new()
        .prefix("scope-appimage-")
        .tempdir()?;
    let image = Image {
        unsquashfs,
        file: path.to_path_buf(),
        offset: runtime.payload_offset,
        root: temp.path().join("root"),
    };
    image
        .extract(&["*.desktop", ".DirIcon", "*.png", "*.svg"])
        .await?;

    let mut desktops: Vec<PathBuf> = std::fs::read_dir(&image.root)
        .context("The AppImage has no readable root directory")?
        .flatten()
        .map(|entry| PathBuf::from(entry.file_name()))
        .filter(|name| name.extension().is_some_and(|ext| ext == "desktop"))
        .collect();
    desktops.sort();
    let desktop = desktops
        .first()
        .context("The AppImage does not ship a launcher (.desktop file)")?;
    let desktop = image
        .resolve(desktop)
        .await?
        .context("The AppImage launcher could not be read")?;
    let launcher = DesktopFile::read(&desktop)?;
    let name = launcher
        .entry("Name")
        .filter(|name| !name.is_empty())
        .context("The AppImage launcher has no Name")?
        .to_string();
    let version = launcher
        .entry("X-AppImage-Version")
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    let icon = image.icon(launcher.entry("Icon")).await;
    drop(temp);
    Ok(Metadata {
        runtime,
        launcher,
        name,
        version,
        icon,
    })
}

struct Image {
    unsquashfs: PathBuf,
    file: PathBuf,
    offset: u64,
    root: PathBuf,
}

impl Image {
    async fn extract(&self, patterns: &[&str]) -> Result<()> {
        let offset = self.offset.to_string();
        let root = self.root.to_string_lossy().into_owned();
        let file = self.file.to_string_lossy().into_owned();
        let mut args = vec![
            "-o",
            offset.as_str(),
            "-n",
            "-no-xattrs",
            "-f",
            "-d",
            root.as_str(),
            file.as_str(),
        ];
        args.extend_from_slice(patterns);
        let out = crate::domain::system::capture_output(
            &self.unsquashfs.to_string_lossy(),
            &args,
            EXTRACT_TIMEOUT,
        )
        .await?;
        if !out.success {
            bail!(
                "unsquashfs could not read the AppImage: {}",
                out.stderr.lines().last().unwrap_or("").trim()
            );
        }
        Ok(())
    }

    /// Resolve an in-image path to an extracted regular file, following
    /// in-image symlinks by extracting their targets.
    async fn resolve(&self, relative: &Path) -> Result<Option<PathBuf>> {
        let mut current = relative.to_path_buf();
        for _ in 0..MAX_LINK_HOPS {
            let on_disk = self.root.join(&current);
            if std::fs::symlink_metadata(&on_disk).is_err() {
                self.extract(&[&current.to_string_lossy()]).await?;
            }
            let Ok(metadata) = std::fs::symlink_metadata(&on_disk) else {
                return Ok(None);
            };
            if metadata.file_type().is_file() {
                return Ok(Some(on_disk));
            }
            if !metadata.file_type().is_symlink() {
                return Ok(None);
            }
            let target = std::fs::read_link(&on_disk)?;
            let base = current.parent().unwrap_or(Path::new(""));
            let Some(next) = normalize(base, &target) else {
                return Ok(None);
            };
            current = next;
        }
        Ok(None)
    }

    /// Find the icon the way Gear Lever does: `.DirIcon`, then the launcher's
    /// icon at the image root, then the hicolor theme sizes.
    async fn icon(&self, name: Option<&str>) -> Option<Icon> {
        let mut candidates = vec![PathBuf::from(".DirIcon")];
        if let Some(name) = name.and_then(icon_stem) {
            for ext in ["svg", "png"] {
                candidates.push(PathBuf::from(format!("{name}.{ext}")));
            }
            let theme = "usr/share/icons/hicolor";
            candidates.push(PathBuf::from(format!("{theme}/scalable/apps/{name}.svg")));
            for size in [512, 256, 128, 96] {
                candidates.push(PathBuf::from(format!(
                    "{theme}/{size}x{size}/apps/{name}.png"
                )));
            }
        }
        for candidate in candidates {
            let Ok(Some(file)) = self.resolve(&candidate).await else {
                continue;
            };
            if let Some(icon) = read_icon(&file) {
                return Some(icon);
            }
        }
        None
    }
}

/// A theme icon name from `Icon=`, without directories or image extensions.
fn icon_stem(icon: &str) -> Option<String> {
    let base = Path::new(icon).file_name()?.to_str()?;
    let stem = base
        .strip_suffix(".png")
        .or_else(|| base.strip_suffix(".svg"))
        .unwrap_or(base);
    (!stem.is_empty() && !stem.starts_with('.') && !stem.contains(['*', '?', '[']))
        .then(|| stem.to_string())
}

/// Join a symlink target onto its directory, refusing to leave the image.
fn normalize(base: &Path, target: &Path) -> Option<PathBuf> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        base.join(target)
    };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Prefix(_) => return None,
        }
    }
    (!out.as_os_str().is_empty()).then_some(out)
}

fn read_icon(path: &Path) -> Option<Icon> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_ICON_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let extension = sniff_icon(&bytes)?;
    Some(Icon { bytes, extension })
}

fn sniff_icon(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
    let head = head.trim_start_matches('\u{feff}').trim_start();
    (head.starts_with('<') && head.contains("<svg")).then_some("svg")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symlink_targets_stay_inside_the_image() {
        assert_eq!(
            normalize(Path::new(""), Path::new("usr/share/applications/a.desktop")),
            Some(PathBuf::from("usr/share/applications/a.desktop"))
        );
        assert_eq!(
            normalize(Path::new("usr/bin"), Path::new("../share/a.png")),
            Some(PathBuf::from("usr/share/a.png"))
        );
        assert_eq!(
            normalize(Path::new(""), Path::new("/usr/share/a.png")),
            Some(PathBuf::from("usr/share/a.png"))
        );
        assert_eq!(
            normalize(Path::new(""), Path::new("../../etc/passwd")),
            None
        );
    }

    #[test]
    fn icons_are_sniffed_by_content() {
        assert_eq!(sniff_icon(b"\x89PNG\r\n\x1a\nrest"), Some("png"));
        assert_eq!(
            sniff_icon(b"<?xml version=\"1.0\"?>\n<svg xmlns=\"x\"/>"),
            Some("svg")
        );
        assert_eq!(sniff_icon(b"GIF89a"), None);
    }

    #[test]
    fn icon_names_drop_paths_and_extensions() {
        assert_eq!(icon_stem("demo").as_deref(), Some("demo"));
        assert_eq!(icon_stem("demo.png").as_deref(), Some("demo"));
        assert_eq!(icon_stem("/abs/path/demo.svg").as_deref(), Some("demo"));
        assert_eq!(icon_stem("*"), None);
    }

    /// Reads a real AppImage when one is provided; ignored by default.
    #[tokio::test]
    #[ignore = "needs SCOPE_TEST_APPIMAGE pointing at a real AppImage"]
    async fn reads_a_real_appimage() {
        let path = std::env::var_os("SCOPE_TEST_APPIMAGE").expect("SCOPE_TEST_APPIMAGE");
        let metadata = read(Path::new(&path)).await.unwrap();
        println!(
            "name={} version={:?} icon={:?} update={:?}",
            metadata.name,
            metadata.version,
            metadata.icon.as_ref().map(|i| (i.extension, i.bytes.len())),
            metadata.runtime.update_info
        );
    }
}
