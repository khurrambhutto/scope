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

const GIO_TIMEOUT: Duration = Duration::from_secs(30);

/// Move one regular file or symlink to the user's Trash.
pub async fn trash(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("{} is no longer present", path.display()))?;
    if metadata.is_dir() {
        bail!("Refusing to trash a directory: {}", path.display());
    }
    let display = path.to_string_lossy().into_owned();
    let gio = super::abs("gio");
    if let Ok(out) = super::capture_output(&gio, &["trash", "--", &display], GIO_TIMEOUT).await {
        if out.success && std::fs::symlink_metadata(path).is_err() {
            return Ok(());
        }
    }
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || trash_manually(&path, &home_trash()?))
        .await
        .context("Trash worker stopped")?
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
    let files = trash.join("files");
    let info = trash.join("info");
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;
    let name = path
        .file_name()
        .context("Path has no file name")?
        .to_string_lossy()
        .into_owned();
    let absolute = std::path::absolute(path)?;
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
        if destination.exists() {
            let _ = std::fs::remove_file(&info_path);
            continue;
        }
        write!(
            record,
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            encode_path(&absolute),
            deletion_date(std::time::SystemTime::now())
        )?;
        record.sync_all()?;
        if let Err(error) = std::fs::rename(path, &destination) {
            let _ = std::fs::remove_file(&info_path);
            bail!("Could not move {} to Trash: {error}", path.display());
        }
        return Ok(());
    }
    bail!("Could not reserve a Trash name for {}", path.display())
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

    #[test]
    fn deletion_date_formats_known_instants() {
        let instant = std::time::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(deletion_date(instant), "2023-11-14T22:13:20");
    }
}
