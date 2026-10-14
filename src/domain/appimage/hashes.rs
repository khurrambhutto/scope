//! SHA-1 and SHA-256 digests of AppImages, cached by file identity.
//!
//! zsync manifests publish SHA-1 and GitHub release assets publish SHA-256,
//! so both are computed in one pass. Rescans reuse the result while the file's
//! size and modification time are unchanged.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use sha1::Digest as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digests {
    pub sha1: String,
    pub sha256: String,
}

type Key = (PathBuf, u64, u128);

fn cache() -> &'static Mutex<HashMap<Key, Digests>> {
    static CACHE: std::sync::OnceLock<Mutex<HashMap<Key, Digests>>> = std::sync::OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Digests of a file, reusing a cached result for an unchanged file.
pub fn digests(path: &Path) -> Result<Digests> {
    let (size, modified) = super::identity(path)?;
    let key = (path.to_path_buf(), size, modified);
    if let Some(found) = cache().lock().unwrap_or_else(|p| p.into_inner()).get(&key) {
        return Ok(found.clone());
    }
    let computed = compute(path)?;
    let mut guard = cache().lock().unwrap_or_else(|p| p.into_inner());
    guard.retain(|(cached, _, _), _| cached != path);
    guard.insert(key, computed.clone());
    Ok(computed)
}

/// Hash a file without consulting the cache.
pub fn compute(path: &Path) -> Result<Digests> {
    let mut file = std::fs::File::open(path)?;
    let mut sha1 = sha1::Sha1::new();
    let mut sha256 = sha2::Sha256::new();
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        sha1.update(&buffer[..read]);
        sha2::Digest::update(&mut sha256, &buffer[..read]);
    }
    Ok(Digests {
        sha1: hex(&sha1.finalize()),
        sha256: hex(&sha2::Digest::finalize(sha256)),
    })
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_known_digests() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        std::fs::write(&path, b"abc").unwrap();
        let digests = digests(&path).unwrap();
        assert_eq!(digests.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            digests.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
