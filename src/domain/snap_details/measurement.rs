//! Bounded measurement of one managed directory, without following symlinks.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Allocated filesystem bytes, which are an estimate rather than freed space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub bytes: Option<u64>,
    pub complete: bool,
    pub note: Option<String>,
}

impl Measurement {
    pub(super) fn unknown(note: &str) -> Self {
        Self {
            bytes: None,
            complete: false,
            note: Some(note.into()),
        }
    }
}

pub(super) fn measure(path: &Path, cancel: &AtomicBool) -> Measurement {
    measure_with_limits(path, cancel, Duration::from_secs(2), 20_000)
}

fn measure_with_limits(
    path: &Path,
    cancel: &AtomicBool,
    limit: Duration,
    entries: usize,
) -> Measurement {
    let root = match path.symlink_metadata() {
        Ok(root) if root.is_dir() && !root.file_type().is_symlink() => root,
        Ok(_) => return Measurement::unknown("Not a regular managed directory"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Measurement::unknown("Directory not present")
        }
        Err(_) => return Measurement::unknown("Directory could not be read"),
    };
    let deadline = Instant::now() + limit;
    let mut seen = HashSet::new();
    let mut bytes = 0_u64;
    let mut complete = true;
    let mut note = Some("Disk usage estimate".into());
    for (count, entry) in WalkDir::new(path)
        .follow_links(false)
        .follow_root_links(false)
        .same_file_system(true)
        .into_iter()
        .enumerate()
    {
        if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline || count >= entries {
            complete = false;
            note = Some("Measurement stopped before all files were read".into());
            break;
        }
        let metadata = match entry.and_then(|entry| entry.metadata()) {
            Ok(metadata) => metadata,
            Err(_) => {
                complete = false;
                note = Some("Some files could not be read".into());
                continue;
            }
        };
        if metadata.dev() != root.dev() {
            complete = false;
            note = Some("Other filesystems were not measured".into());
            continue;
        }
        if metadata.file_type().is_symlink() {
            note = Some("Disk usage estimate; link targets excluded".into());
        }
        if seen.insert((metadata.dev(), metadata.ino())) {
            let Some(total) = metadata
                .blocks()
                .checked_mul(512)
                .and_then(|size| bytes.checked_add(size))
            else {
                return Measurement::unknown("Directory size exceeded the measurement limit");
            };
            bytes = total;
        }
    }
    Measurement {
        bytes: Some(bytes),
        complete,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    #[test]
    fn counts_hard_links_once() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("data");
        fs::write(&file, vec![1_u8; 8192]).unwrap();
        fs::hard_link(&file, directory.path().join("copy")).unwrap();
        let size = measure(directory.path(), &AtomicBool::new(false));
        let expected = (fs::metadata(&file).unwrap().blocks()
            + fs::metadata(directory.path()).unwrap().blocks())
            * 512;
        assert_eq!(size.bytes, Some(expected));
        assert!(size.complete);
    }

    #[test]
    fn does_not_follow_links_outside_the_directory() {
        let directory = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), vec![1_u8; 8192]).unwrap();
        symlink(outside.path(), directory.path().join("link")).unwrap();
        let size = measure(directory.path(), &AtomicBool::new(false));
        let expected = (fs::metadata(directory.path()).unwrap().blocks()
            + fs::symlink_metadata(directory.path().join("link"))
                .unwrap()
                .blocks())
            * 512;
        assert_eq!(size.bytes, Some(expected));
        assert!(size.complete);
    }

    #[test]
    fn root_symlink_is_unknown() {
        let directory = tempfile::tempdir().unwrap();
        symlink(directory.path(), directory.path().join("link")).unwrap();
        assert!(
            measure(&directory.path().join("link"), &AtomicBool::new(false))
                .bytes
                .is_none()
        );
    }

    #[test]
    fn missing_directory_is_not_reported_as_zero() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            measure(&directory.path().join("missing"), &AtomicBool::new(false))
                .bytes
                .is_none()
        );
    }

    #[test]
    fn entry_budget_and_cancellation_produce_partial_measurements() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            !measure_with_limits(
                directory.path(),
                &AtomicBool::new(false),
                Duration::from_secs(1),
                0
            )
            .complete
        );
        assert!(!measure(directory.path(), &AtomicBool::new(true)).complete);
    }
}
