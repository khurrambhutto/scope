use super::*;
use std::io::Read as _;

pub(super) fn collect_launchers(
    roots: &Roots,
    root: &Path,
    executable: &Path,
    owner: u32,
) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let start = Instant::now();
    let mut visited = 0;
    for base in std::iter::once(&roots.applications).chain(&roots.other_applications) {
        let mut pending = vec![base.clone()];
        while let Some(dir) = pending.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    return Err(error).context("Cannot establish complete launcher ownership")
                }
            };
            for entry in entries {
                visited += 1;
                if visited > ENTRY_LIMIT || start.elapsed() > WALK_TIMEOUT {
                    bail!("Launcher ownership search is incomplete");
                }
                let entry = entry?;
                let path = entry.path();
                let meta = path.symlink_metadata()?;
                if meta.is_symlink() && path.is_dir() {
                    bail!("Cannot establish launcher ownership through a linked directory");
                }
                if meta.is_dir() {
                    if path
                        .components()
                        .count()
                        .saturating_sub(base.components().count())
                        > 8
                    {
                        bail!("Launcher ownership search exceeded its depth bound");
                    }
                    pending.push(path);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "desktop") {
                    continue;
                }
                let file = DesktopFile::read(&path)
                    .context("Cannot read a launcher while checking shared ownership")?;
                let candidate = file
                    .entry("Exec")
                    .and_then(|exec| exec_program(&unescape(exec)))
                    .map(PathBuf::from);
                // An external icon is retained. An unrelated launcher using an
                // icon inside the installation would break when its root moves.
                let icon_claim = file
                    .entry("Icon")
                    .map(|icon| PathBuf::from(unescape(icon)))
                    .is_some_and(|icon| icon.is_absolute() && icon.starts_with(root));
                let action_claim = file.to_text().lines().any(|line| {
                    line.trim_start()
                        .strip_prefix("Exec=")
                        .is_some_and(|value| {
                            root.to_str()
                                .is_some_and(|root| unescape(value).contains(root))
                        })
                });
                let exec_claim = action_claim
                    || candidate
                        .as_ref()
                        .is_some_and(|path| path.starts_with(root));
                if !exec_claim && !icon_claim {
                    continue;
                }
                if path.parent() != Some(roots.applications.as_path())
                    || meta.is_symlink()
                    || meta.uid() != owner
                    || target(&file).as_deref() != Some(executable)
                {
                    bail!("{} also claims the extracted installation", path.display());
                }
                found.push(path);
            }
        }
    }
    found.sort();
    found.dedup();
    Ok(found)
}

pub(super) fn check_mounts(roots: &Roots, root: &Path) -> Result<()> {
    let mut mountinfo = String::new();
    std::fs::File::open(roots.processes.join("self/mountinfo"))
        .context("Cannot establish installation mount boundaries")?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut mountinfo)?;
    if mountinfo.len() > 1024 * 1024 {
        bail!("Installation mount inspection is incomplete");
    }
    for line in mountinfo.lines() {
        let raw = line
            .split_whitespace()
            .nth(4)
            .context("Malformed mount metadata")?;
        let decoded = raw
            .replace("\\040", " ")
            .replace("\\011", "\t")
            .replace("\\012", "\n")
            .replace("\\134", "\\");
        let mount = Path::new(&decoded);
        if !mount.is_absolute() {
            bail!("Malformed mount point");
        }
        if mount.starts_with(root) {
            bail!("{} contains a mounted filesystem", root.display());
        }
    }
    Ok(())
}

pub(super) fn validate_tree(root: &Path, owner: u32, device: u64) -> Result<()> {
    let mut pending = vec![root.to_path_buf()];
    let start = Instant::now();
    let mut count = 0;
    while let Some(path) = pending.pop() {
        count += 1;
        if count > ENTRY_LIMIT || start.elapsed() > WALK_TIMEOUT {
            bail!("Installation ownership inspection is incomplete");
        }
        let m = path.symlink_metadata()?;
        if m.uid() != owner || m.dev() != device || !(m.is_dir() || m.is_file() || m.is_symlink()) {
            bail!(
                "{} has foreign ownership, a mount boundary or unsupported file type",
                path.display()
            );
        }
        if m.is_dir() {
            for child in std::fs::read_dir(path)? {
                pending.push(child?.path());
            }
        }
    }
    Ok(())
}
