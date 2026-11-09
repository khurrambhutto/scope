use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct Roots {
    pub home: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub processes: PathBuf,
}

impl Roots {
    pub fn current() -> Result<Self> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .context("HOME is unavailable")?;
        let root = |name: &str, fallback: &str| {
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        Ok(Self {
            config: root("XDG_CONFIG_HOME", ".config"),
            data: root("XDG_DATA_HOME", ".local/share"),
            cache: root("XDG_CACHE_HOME", ".cache"),
            state: root("XDG_STATE_HOME", ".local/state"),
            processes: PathBuf::from("/proc"),
            home,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct FileIdentity {
    device: u64,
    inode: u64,
    owner: u32,
    directory: bool,
}

pub(super) fn verify(path: &Path, roots: &Roots) -> Result<FileIdentity> {
    if !path.is_absolute()
        || !path.starts_with(&roots.home)
        || path == roots.home
        || !path
            .components()
            .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
    {
        bail!("Only a bounded app-owned path inside HOME is eligible");
    }
    if [
        &roots.config,
        &roots.data,
        &roots.cache,
        &roots.state,
        &roots.processes,
    ]
    .iter()
    .any(|base| base.starts_with(path))
    {
        bail!("App cleanup cannot remove a shared XDG/process root or its ancestor");
    }
    check_mounts(path, &roots.processes)?;
    let home = std::fs::symlink_metadata(&roots.home)?;
    let metadata = std::fs::symlink_metadata(path)?;
    if !(metadata.is_file() || metadata.is_dir())
        || metadata.uid() != home.uid()
        || metadata.dev() != home.dev()
    {
        bail!("Path is linked, not user-owned, or on another filesystem");
    }
    let mut ancestor = path.parent();
    while let Some(parent) = ancestor {
        let m = std::fs::symlink_metadata(parent)?;
        if !m.is_dir() || m.dev() != home.dev() {
            bail!("Linked ancestor or mount boundary");
        }
        if parent == roots.home {
            break;
        }
        ancestor = parent.parent();
    }
    // Moving a directory containing a mount or a foreign-owned tree is not a
    // reversible single-filesystem move. Bound the proof, just like measurement.
    if metadata.is_dir() {
        let started = Instant::now();
        for (count, entry) in walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .enumerate()
        {
            if count > 20_000 || started.elapsed() > Duration::from_secs(2) {
                bail!("Directory ownership inspection is incomplete");
            }
            let entry = entry?;
            let m = std::fs::symlink_metadata(entry.path())?;
            if m.dev() != home.dev() || m.uid() != home.uid() {
                bail!("Directory contains a mount or files owned by another user");
            }
            if !(m.is_dir() || m.is_file() || m.is_symlink()) {
                bail!("Directory contains a special file");
            }
        }
    }
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
        owner: metadata.uid(),
        directory: metadata.is_dir(),
    })
}

fn check_mounts(path: &Path, processes: &Path) -> Result<()> {
    use std::io::Read as _;
    let mut text = String::new();
    std::fs::File::open(processes.join("self/mountinfo"))?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 1024 * 1024 {
        bail!("Mount inventory exceeds its bound");
    }
    for line in text.lines() {
        let encoded = line
            .split_whitespace()
            .nth(4)
            .context("Mount inventory is malformed")?;
        let decoded = encoded
            .replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\012", "\n")
            .replace("\\134", "\\");
        let mount = Path::new(&decoded);
        if !mount.is_absolute() {
            bail!("Mount inventory is malformed");
        }
        if mount.starts_with(path) {
            bail!("App data contains a mount point; retained");
        }
    }
    Ok(())
}
