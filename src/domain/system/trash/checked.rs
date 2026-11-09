//! Bind a reversible move to the object that the user reviewed.

use std::path::Path;

use anyhow::{bail, Context, Result};

/// Recheck the reviewed identity before and after a private same-filesystem
/// staging move. The callback must compare against a previously captured identity.
pub async fn trash_checked(
    path: &Path,
    check: impl Fn(&Path) -> Result<()> + Send + 'static,
) -> Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || move_checked(&path, &super::home_trash()?, check))
        .await
        .context("Verified Trash worker stopped")?
}

#[cfg(test)]
pub(crate) async fn trash_checked_into(
    path: &Path,
    trash: &Path,
    check: impl Fn(&Path) -> Result<()> + Send + 'static,
) -> Result<()> {
    let path = path.to_path_buf();
    let trash = trash.to_path_buf();
    tokio::task::spawn_blocking(move || move_checked(&path, &trash, check)).await?
}

fn move_checked(path: &Path, trash: &Path, check: impl Fn(&Path) -> Result<()>) -> Result<()> {
    check(path)?;
    let parent = path.parent().context("Reviewed path has no parent")?;
    if parent.canonicalize()? != parent {
        bail!("The reviewed parent directory changed or is linked");
    }
    // Never let TempDir recursively delete a staged app/profile on an error.
    // Keep the directory immediately; only remove it explicitly when empty.
    let staging = tempfile::Builder::new()
        .prefix(".scope-trash-")
        .tempdir_in(parent)?
        .keep();
    let staged = staging.join(path.file_name().context("Reviewed path has no name")?);
    if let Err(error) = super::rename_noreplace(path, &staged) {
        let _ = std::fs::remove_dir(&staging);
        return Err(error).context("Could not stage the reviewed object");
    }
    let moved = check(&staged).and_then(|()| super::trash_manually_from(&staged, trash, path));
    if let Err(error) = moved {
        if let Err(restore) = super::rename_noreplace(&staged, path) {
            bail!(
                "{error:#}. Could not restore {}: {restore}. Recover the retained item from {}",
                path.display(),
                staged.display()
            );
        }
        let _ = std::fs::remove_dir(&staging);
        return Err(error).context("The object was retained at its original location");
    }
    let _ = std::fs::remove_dir(&staging);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::MetadataExt as _;

    #[test]
    fn trash_record_preserves_original_path_and_profile_contents() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        std::fs::create_dir(&profile).unwrap();
        std::fs::write(profile.join("settings"), b"saved").unwrap();
        let inode = profile.symlink_metadata().unwrap().ino();
        let trash = dir.path().join("Trash");
        move_checked(&profile, &trash, |path| {
            anyhow::ensure!(path.symlink_metadata()?.ino() == inode, "changed");
            Ok(())
        })
        .unwrap();
        assert!(!profile.exists());
        assert_eq!(
            std::fs::read(trash.join("files/profile/settings")).unwrap(),
            b"saved"
        );
        let record = std::fs::read_to_string(trash.join("info/profile.trashinfo")).unwrap();
        assert!(record.contains(&format!("Path={}\n", super::super::encode_path(&profile))));
        assert!(!record.contains(".scope-trash-"));
    }

    #[test]
    fn replacement_during_handoff_is_retained_and_never_trashed() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        std::fs::write(&profile, b"reviewed").unwrap();
        let inode = profile.symlink_metadata().unwrap().ino();
        let first = Cell::new(true);
        let trash = dir.path().join("Trash");
        let result = move_checked(&profile, &trash, |path| {
            anyhow::ensure!(path.symlink_metadata()?.ino() == inode, "changed");
            if first.replace(false) {
                std::fs::rename(path, dir.path().join("original"))?;
                std::fs::write(path, b"replacement")?;
            }
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(profile).unwrap(), b"replacement");
        assert_eq!(
            std::fs::read(dir.path().join("original")).unwrap(),
            b"reviewed"
        );
        assert!(!trash.exists());
    }

    #[test]
    fn failed_trash_restores_without_overwriting_a_recreated_original() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        std::fs::write(&profile, b"reviewed").unwrap();
        let first = Cell::new(true);
        let bad_trash = dir.path().join("not-a-directory");
        std::fs::write(&bad_trash, b"block").unwrap();
        let result = move_checked(&profile, &bad_trash, |_| {
            if !first.replace(false) {
                std::fs::write(&profile, b"new writer")?;
            }
            Ok(())
        });
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Recover the retained item"), "{error}");
        assert_eq!(std::fs::read(profile).unwrap(), b"new writer");
        let recovery = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".scope-trash-")
            })
            .unwrap();
        assert_eq!(
            std::fs::read(recovery.path().join("profile")).unwrap(),
            b"reviewed"
        );
    }

    #[test]
    fn failed_trash_restores_the_original_when_path_is_vacant() {
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("profile");
        std::fs::write(&profile, b"reviewed").unwrap();
        let bad_trash = dir.path().join("not-a-directory");
        std::fs::write(&bad_trash, b"block").unwrap();
        assert!(move_checked(&profile, &bad_trash, |_| Ok(())).is_err());
        assert_eq!(std::fs::read(profile).unwrap(), b"reviewed");
        assert!(!std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with(".scope-trash-")));
    }
}
