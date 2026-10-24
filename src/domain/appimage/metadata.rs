//! Recover installed versions from the bundle, even after an app rewrites its
//! launcher. Only read the root desktop metadata; never execute the AppImage.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use futures::{stream, StreamExt as _};
use tokio::io::{AsyncRead, AsyncReadExt as _};

use super::{desktop::DesktopFile, elf};
use crate::domain::package::InstalledPackage;

const READ_TIMEOUT: Duration = Duration::from_secs(5);
const SCAN_BUDGET: Duration = Duration::from_secs(30);
const MAX_DESKTOP_BYTES: usize = 256 * 1024;
const MAX_CACHED_IMAGES: usize = 128;

#[derive(Clone)]
struct CachedVersion {
    identity: (u64, u128),
    version: Option<String>,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, CachedVersion>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedVersion>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// Prefer metadata in the actual installed file over stale launcher versions.
/// Failure leaves the existing launcher/filename fallback visible.
pub async fn enrich(packages: &mut [InstalledPackage]) {
    let pending: Vec<_> = packages
        .iter()
        .enumerate()
        .filter_map(|(index, package)| {
            let installation = package.appimage_installation.as_ref()?;
            Some((
                index,
                installation.path.clone(),
                (installation.size, installation.modified_ns),
            ))
        })
        .collect();
    let mut reads = stream::iter(pending)
        .map(|(index, path, identity)| async move {
            let version = read_version(&path).await;
            (index, path, identity, version)
        })
        .buffer_unordered(2);
    let deadline = tokio::time::Instant::now() + SCAN_BUDGET;
    while let Ok(Some((index, path, identity, result))) =
        tokio::time::timeout_at(deadline, reads.next()).await
    {
        if super::identity(&path).ok() != Some(identity) {
            continue;
        }
        if let Ok(Some(version)) = result {
            packages[index].version = version;
        }
    }
}

/// Cache only successful reads, keyed by the file identity rather than the
/// mutable desktop launcher. Replacing an image invalidates its metadata.
pub async fn read_version(path: &Path) -> Result<Option<String>> {
    let identity = super::identity(path)?;
    if let Some(cached) = cache().lock().ok().and_then(|cache| {
        cache
            .get(path)
            .filter(|item| item.identity == identity)
            .cloned()
    }) {
        return Ok(cached.version);
    }
    let runtime = elf::inspect(path)?;
    let reader = crate::domain::system::resolve("unsquashfs")
        .context("Reading AppImage versions needs unsquashfs (squashfs-tools)")?;
    let mut command = tokio::process::Command::new(reader);
    command
        .args([
            "-o",
            &runtime.payload_offset.to_string(),
            "-p",
            "1",
            "-mem",
            "8M",
            "-cat",
        ])
        .arg(path)
        .arg("*.desktop")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().context("Missing metadata output")?;
    let stderr = child.stderr.take().context("Missing metadata errors")?;
    let read = tokio::time::timeout(READ_TIMEOUT, async {
        tokio::try_join!(
            bounded_output(stdout, MAX_DESKTOP_BYTES),
            bounded_output(stderr, 16 * 1024),
            async { child.wait().await.map_err(anyhow::Error::from) },
        )
    })
    .await;
    let (output, _, status) = match read {
        Ok(Ok(result)) => result,
        other => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            match other {
                Ok(Err(error)) => return Err(error),
                _ => bail!("Reading AppImage version metadata timed out"),
            }
        }
    };
    if !status.success() {
        bail!("The AppImage has no readable root launcher metadata");
    }
    let version = launcher_version(std::str::from_utf8(&output)?)?;
    if super::identity(path)? != identity {
        bail!("The AppImage changed while reading its version");
    }
    if let Ok(mut cache) = cache().lock() {
        if cache.len() >= MAX_CACHED_IMAGES {
            cache.clear();
        }
        cache.insert(
            path.to_path_buf(),
            CachedVersion {
                identity,
                version: version.clone(),
            },
        );
    }
    Ok(version)
}

async fn bounded_output(reader: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > limit {
        bail!("AppImage metadata is too large");
    }
    Ok(bytes)
}

fn launcher_version(text: &str) -> Result<Option<String>> {
    // `-cat` can concatenate matches. Never pick another bundled app's version.
    if text
        .lines()
        .filter(|line| line.trim() == "[Desktop Entry]")
        .count()
        != 1
    {
        bail!("The AppImage has ambiguous root launcher metadata");
    }
    let launcher = DesktopFile::parse(text);
    if launcher.entry("Type") != Some("Application") {
        bail!("The AppImage launcher is not an application");
    }
    Ok(launcher
        .entry("X-AppImage-Version")
        .filter(|version| {
            !version.is_empty() && version.len() <= 128 && !version.chars().any(char::is_control)
        })
        .map(str::to_owned))
}

/// Build a small real SquashFS payload; every file stays in the test's private
/// directory. The ELF header is synthetic and cannot launch an application.
#[cfg(test)]
pub(crate) fn write_test_bundle(path: &Path, desktop: &str) -> bool {
    let Some(builder) = crate::domain::system::resolve("mksquashfs") else {
        eprintln!("Skipping bundle fixture: mksquashfs is not installed");
        return false;
    };
    if !crate::domain::system::which("unsquashfs") {
        eprintln!("Skipping bundle fixture: unsquashfs is not installed");
        return false;
    }
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("demo.desktop"), desktop).unwrap();
    let payload = temp.path().join("payload");
    let output = std::process::Command::new(builder)
        .arg(&root)
        .arg(&payload)
        .args(["-noappend", "-no-xattrs", "-processors", "1", "-quiet"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(
        path,
        elf::test_appimage("", &std::fs::read(payload).unwrap()),
    )
    .unwrap();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_shipped_by_zcode() {
        assert_eq!(
            launcher_version(
                "[Desktop Entry]\nName=ZCode\nType=Application\nX-AppImage-Version=3.16.3.8992\n"
            )
            .unwrap()
            .as_deref(),
            Some("3.16.3.8992")
        );
        assert!(launcher_version("[Desktop Entry]\nType=Application\n")
            .unwrap()
            .is_none());
        assert!(launcher_version("[Desktop Entry]\nType=Application\n[Desktop Entry]\nType=Application\nX-AppImage-Version=9\n").is_err());
    }

    #[tokio::test]
    async fn oversized_metadata_is_rejected() {
        assert!(bounded_output(&b"12345"[..], 4).await.is_err());
        assert_eq!(bounded_output(&b"1234"[..], 4).await.unwrap(), b"1234");
    }

    #[tokio::test]
    async fn replacing_the_image_invalidates_its_cached_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.appimage");
        if !write_test_bundle(
            &path,
            "[Desktop Entry]\nType=Application\nName=Demo\nX-AppImage-Version=1.2\n",
        ) {
            return;
        }
        assert_eq!(read_version(&path).await.unwrap().as_deref(), Some("1.2"));
        write_test_bundle(
            &path,
            "[Desktop Entry]\nType=Application\nName=Demo\nX-AppImage-Version=2.3.456\n",
        );
        assert_eq!(
            read_version(&path).await.unwrap().as_deref(),
            Some("2.3.456")
        );
    }
}
