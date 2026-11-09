//! Move user files to the freedesktop Trash instead of deleting them.
//!
//! `gio trash` is preferred because it follows the desktop's own trash rules.
//! When it is missing or fails, the file is renamed into the home trash with a
//! matching `.trashinfo` record. Files on another filesystem are never copied
//! and deleted; that fails closed instead.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};

mod checked;
pub use checked::trash_checked;
#[cfg(test)]
pub(crate) use checked::trash_checked_into;

const GIO_TIMEOUT: Duration = Duration::from_secs(30);

/// Move one validated file, directory, or symlink to the user's Trash.
/// Callers must authorize the path and revalidate its ownership before calling.
pub async fn trash(path: &Path) -> Result<()> {
    trash_with(path, Some(&super::abs("gio")), home_trash).await
}

async fn trash_with(
    path: &Path,
    gio: Option<&str>,
    fallback_trash: impl FnOnce() -> Result<PathBuf> + Send + 'static,
) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("{} is no longer present", path.display()))?;
    validate_type(path, &metadata)?;
    // The command runner takes UTF-8 argv; never substitute a lossy filename.
    if let (Some(display), Some(gio)) = (path.to_str(), gio) {
        if let Ok(out) = super::capture_output(gio, &["trash", "--", display], GIO_TIMEOUT).await {
            if out.success
                && matches!(std::fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
            {
                return Ok(());
            }
        }
    }
    let current = std::fs::symlink_metadata(path)
        .with_context(|| format!("Cannot verify {} after attempting Trash", path.display()))?;
    use std::os::unix::fs::MetadataExt as _;
    if metadata.dev() != current.dev()
        || metadata.ino() != current.ino()
        || metadata.file_type() != current.file_type()
    {
        bail!("{} changed while attempting Trash", path.display());
    }
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || trash_manually(&path, &fallback_trash()?))
        .await
        .context("Trash worker stopped")?
}

fn validate_type(path: &Path, metadata: &std::fs::Metadata) -> Result<()> {
    let kind = metadata.file_type();
    if !kind.is_file() && !kind.is_dir() && !kind.is_symlink() {
        bail!("Refusing to trash a special file: {}", path.display());
    }
    Ok(())
}

fn home_trash() -> Result<PathBuf> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .context("HOME is not set")?;
    Ok(data.join("Trash"))
}

fn trash_manually(path: &Path, trash: &Path) -> Result<()> {
    trash_manually_from(path, trash, path)
}

fn trash_manually_from(path: &Path, trash: &Path, origin: &Path) -> Result<()> {
    let files = trash.join("files");
    let info = trash.join("info");
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let name = path
        .file_name()
        .context("Path has no file name")?
        .to_string_lossy()
        .into_owned();
    let absolute = std::path::absolute(origin)?;
    for attempt in 0..1000 {
        let candidate = if attempt == 0 {
            name.clone()
        } else {
            format!("{name}.{attempt}")
        };
        let info_path = info.join(format!("{candidate}.trashinfo"));
        // Reserving the info file first makes the chosen name unique.
        let mut record = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        };
        let destination = files.join(&candidate);
        match std::fs::symlink_metadata(&destination) {
            Ok(_) => {
                let _ = std::fs::remove_file(&info_path);
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                let _ = std::fs::remove_file(&info_path);
                return Err(error.into());
            }
        }
        let write_result = (|| -> std::io::Result<()> {
            write!(
                record,
                "[Trash Info]\nPath={}\nDeletionDate={}\n",
                encode_path(&absolute),
                deletion_date(std::time::SystemTime::now())
            )?;
            record.sync_all()
        })();
        if let Err(error) = write_result {
            let _ = std::fs::remove_file(&info_path);
            return Err(error.into());
        }
        if let Err(error) = rename_noreplace(path, &destination) {
            let _ = std::fs::remove_file(&info_path);
            bail!("Could not move {} to Trash: {error}", path.display());
        }
        return Ok(());
    }
    bail!("Could not reserve a Trash name for {}", path.display())
}

fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

/// Percent-encode a path for `.trashinfo`, keeping `/` and unreserved bytes.
fn encode_path(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt as _;
    let mut out = String::new();
    for &byte in path.as_os_str().as_bytes() {
        if byte.is_ascii_alphanumeric() || b"/-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `YYYY-MM-DDThh:mm:ss` in UTC; the spec's local time is advisory only.
fn deletion_date(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Civil-from-days (Howard Hinnant), valid for the whole Unix era.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_trash_moves_the_file_and_records_its_origin() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("my app.AppImage");
        std::fs::write(&file, b"x").unwrap();
        let trash = dir.path().join("Trash");

        trash_manually(&file, &trash).unwrap();

        assert!(!file.exists());
        assert!(trash.join("files/my app.AppImage").exists());
        let info = std::fs::read_to_string(trash.join("info/my app.AppImage.trashinfo")).unwrap();
        assert!(info.contains("my%20app.AppImage"));
        assert!(info.contains("DeletionDate="));
    }

    #[test]
    fn manual_trash_never_overwrites_an_existing_trashed_file() {
        let dir = tempfile::tempdir().unwrap();
        let trash = dir.path().join("Trash");
        for content in [b"one", b"two"] {
            let file = dir.path().join("tool.AppImage");
            std::fs::write(&file, content).unwrap();
            trash_manually(&file, &trash).unwrap();
        }
        assert_eq!(
            std::fs::read(trash.join("files/tool.AppImage")).unwrap(),
            b"one"
        );
        assert_eq!(
            std::fs::read(trash.join("files/tool.AppImage.1")).unwrap(),
            b"two"
        );
    }

    #[tokio::test]
    async fn trash_accepts_nonempty_directory_without_following_nested_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("app data");
        std::fs::create_dir_all(data.join("profile")).unwrap();
        std::fs::write(data.join("profile/settings"), b"settings").unwrap();
        let retained = dir.path().join("retained");
        std::fs::write(&retained, b"keep").unwrap();
        std::os::unix::fs::symlink(&retained, data.join("external")).unwrap();
        let trash = dir.path().join("Trash");
        let fallback = trash.clone();

        trash_with(&data, None, move || Ok(fallback)).await.unwrap();

        assert!(!data.exists());
        assert_eq!(
            std::fs::read(trash.join("files/app data/profile/settings")).unwrap(),
            b"settings"
        );
        assert!(
            std::fs::symlink_metadata(trash.join("files/app data/external"))
                .unwrap()
                .is_symlink()
        );
        assert_eq!(std::fs::read(retained).unwrap(), b"keep");
        let info = std::fs::read_to_string(trash.join("info/app data.trashinfo")).unwrap();
        assert!(info.contains(&format!("Path={}\n", encode_path(&data))));
    }

    #[test]
    fn manual_trash_preserves_dangling_destination_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let trash = dir.path().join("Trash");
        std::fs::create_dir_all(trash.join("files")).unwrap();
        let occupied = trash.join("files/app");
        std::os::unix::fs::symlink("missing", &occupied).unwrap();
        let data = dir.path().join("app");
        std::fs::create_dir(&data).unwrap();

        trash_manually(&data, &trash).unwrap();

        assert_eq!(std::fs::read_link(occupied).unwrap(), Path::new("missing"));
        assert!(trash.join("files/app.1").is_dir());
    }

    #[test]
    fn failed_move_leaves_no_trash_record() {
        let dir = tempfile::tempdir().unwrap();
        let trash = dir.path().join("Trash");
        assert!(trash_manually(&dir.path().join("missing"), &trash).is_err());
        assert_eq!(std::fs::read_dir(trash.join("info")).unwrap().count(), 0);
    }

    #[test]
    fn manual_trash_moves_symlink_without_moving_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let trash = dir.path().join("Trash");

        trash_manually(&link, &trash).unwrap();

        assert!(target.is_dir());
        assert_eq!(
            std::fs::read_link(trash.join("files/link")).unwrap(),
            target
        );
        assert!(std::fs::symlink_metadata(link).is_err());
    }

    #[test]
    fn special_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("socket");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(validate_type(&socket, &std::fs::symlink_metadata(&socket).unwrap()).is_err());
    }

    #[test]
    fn deletion_date_formats_known_instants() {
        let instant = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(deletion_date(instant), "2023-11-14T22:13:20");
    }
}
