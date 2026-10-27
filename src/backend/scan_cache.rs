//! Persistent scan cache. The UI only sends complete, accepted snapshots.
use super::Scan;
use std::path::PathBuf;

pub(super) fn store(scan: Scan) {
    static WRITER: std::sync::OnceLock<std::sync::mpsc::Sender<Scan>> = std::sync::OnceLock::new();
    let writer = WRITER.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<Scan>();
        std::thread::spawn(move || {
            for scan in rx {
                if let Some(path) = cache_path() {
                    write(&scan, &path);
                }
            }
        });
        tx
    });
    let _ = writer.send(scan);
}

/// Path of the persisted scan inside the XDG data directory.
fn cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })?;
    Some(base.join("scope").join("scan-cache.json"))
}

/// Load the previous session's scan, if any, so a cold start paints instantly.
pub(super) fn read() -> Option<Scan> {
    let bytes = std::fs::read(cache_path()?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Persist a scan for the next launch.
fn write(scan: &Scan, path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec(scan) {
        if let Some(parent) = path.parent() {
            if let Ok(mut temp) = tempfile::Builder::new()
                .prefix(".scan-cache-")
                .tempfile_in(parent)
            {
                use std::io::Write as _;
                if temp.write_all(&bytes).is_ok() && temp.as_file().sync_all().is_ok() {
                    let _ = temp.persist(path);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        package::{InstalledPackage, PackageSource},
        scanner::ScanAvailability,
    };

    #[test]
    fn atomic_cache_replacement_keeps_only_the_latest_complete_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scope/scan-cache.json");
        let mut scan = Scan {
            packages: vec![InstalledPackage::new(PackageSource::Cargo, "fixture")],
            availability: ScanAvailability::default(),
            scanned_at_ms: 1,
        };
        write(&scan, &path);
        scan.scanned_at_ms = 2;
        scan.packages[0].version = "2.0.0".into();
        write(&scan, &path);
        let saved: Scan = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.scanned_at_ms, 2);
        assert_eq!(saved.packages[0].version, "2.0.0");
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            1
        );
    }
}
