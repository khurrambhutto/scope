//! Stage an integration before replacing any installed files. Keep hard-linked
//! originals until every replacement succeeds, so write failures can roll back.

use std::io::{Read as _, Write as _};
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::Digest as _;

pub(super) struct PreparedFile {
    path: PathBuf,
    staged: tempfile::NamedTempFile,
    previous: Option<PreviousFile>,
}

struct PreviousFile {
    directory: tempfile::TempDir,
    identity: (u64, u64, u64, i64, i64),
}

fn file_identity(path: &Path) -> Result<(u64, u64, u64, i64, i64)> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        bail!("{} is no longer a regular file", path.display());
    }
    Ok((
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
    ))
}

impl PreparedFile {
    fn new(path: &Path, replace: bool) -> Result<Self> {
        let parent = path.parent().context("Installation path has no parent")?;
        std::fs::create_dir_all(parent)?;
        let previous = match std::fs::symlink_metadata(path) {
            Ok(metadata) if replace && metadata.file_type().is_file() => {
                let backup = tempfile::Builder::new()
                    .prefix(".scope-backup-")
                    .tempdir_in(parent)?;
                std::fs::hard_link(path, backup.path().join("previous"))?;
                let identity = file_identity(&backup.path().join("previous"))?;
                Some(PreviousFile {
                    directory: backup,
                    identity,
                })
            }
            Ok(_) => bail!("{} already exists or is not a regular file", path.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && !replace => None,
            Err(error) => return Err(error).with_context(|| format!("{} changed", path.display())),
        };
        Ok(Self {
            path: path.to_path_buf(),
            staged: tempfile::Builder::new()
                .prefix(".scope-install-")
                .tempfile_in(parent)?,
            previous,
        })
    }

    pub(super) fn bytes(path: &Path, bytes: &[u8], replace: bool) -> Result<Self> {
        let mut prepared = Self::new(path, replace)?;
        prepared.staged.write_all(bytes)?;
        prepared.finish(0o644)?;
        Ok(prepared)
    }

    pub(super) fn appimage(
        source: &Path,
        destination: &Path,
        expected_sha256: &str,
        replace: bool,
    ) -> Result<Self> {
        let mut prepared = Self::new(destination, replace)?;
        let mut input = std::fs::File::open(source)?;
        let mut hasher = sha2::Sha256::new();
        let mut buffer = vec![0u8; 1 << 16];
        loop {
            let read = input.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            prepared.staged.write_all(&buffer[..read])?;
        }
        if super::hashes::hex(&hasher.finalize()) != expected_sha256 {
            bail!("The AppImage changed since it was checked. Nothing was installed.");
        }
        prepared.finish(0o755)?;
        Ok(prepared)
    }

    fn finish(&self, mode: u32) -> Result<()> {
        self.staged
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(mode))?;
        self.staged.as_file().sync_all()?;
        Ok(())
    }
}

/// Commit metadata before the executable. If any rename fails, restore the
/// completed replacements and remove only files this transaction created.
pub(super) fn commit(files: Vec<PreparedFile>) -> Result<()> {
    let mut completed = Vec::new();
    for file in files {
        let PreparedFile {
            path,
            staged,
            previous,
        } = file;
        let result: Result<()> = (|| {
            if let Some(previous) = &previous {
                if file_identity(&path)? != previous.identity {
                    bail!("{} changed during installation. Try again.", path.display());
                }
                staged.persist(&path)?;
            } else {
                staged.persist_noclobber(&path)?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => completed.push((path, previous)),
            Err(error) => {
                let mut problems = vec![format!("Could not install {}: {}", path.display(), error)];
                for (path, previous) in completed.into_iter().rev() {
                    let restore = match previous.as_ref() {
                        Some(backup) => {
                            std::fs::rename(backup.directory.path().join("previous"), &path)
                        }
                        None => std::fs::remove_file(&path),
                    };
                    if let Err(error) = restore {
                        problems.push(format!("Could not restore {}: {error}", path.display()));
                        if let Some(backup) = previous {
                            problems.push(format!(
                                "Original saved in {}",
                                backup.directory.keep().display()
                            ));
                        }
                    }
                }
                bail!("{}", problems.join(". "));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_commit_restores_replaced_files_and_removes_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.desktop");
        let new = dir.path().join("icon.png");
        let blocked = dir.path().join("app.appimage");
        std::fs::write(&old, b"old launcher").unwrap();
        let files = vec![
            PreparedFile::bytes(&old, b"new launcher", true).unwrap(),
            PreparedFile::bytes(&new, b"new icon", false).unwrap(),
            PreparedFile::bytes(&blocked, b"new app", false).unwrap(),
        ];
        // A concurrent installation takes the final destination after staging.
        std::fs::write(&blocked, b"other app").unwrap();
        assert!(commit(files).is_err());
        assert_eq!(std::fs::read(&old).unwrap(), b"old launcher");
        assert!(!new.exists());
        assert_eq!(std::fs::read(&blocked).unwrap(), b"other app");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn a_file_changed_after_staging_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("demo.desktop");
        std::fs::write(&path, b"old").unwrap();
        let staged = PreparedFile::bytes(&path, b"new", true).unwrap();
        std::fs::write(&path, b"rewritten by the app").unwrap();
        assert!(commit(vec![staged]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"rewritten by the app");
    }
}
