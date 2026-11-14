//! Mount table parsing and the rules for which mounts the disk walk descends into.
//!
//! Same-filesystem is decided by mount source and filesystem type, not by
//! `st_dev`: Btrfs subvolumes such as `/home` report different device numbers
//! while living on the same filesystem.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// One line of `/proc/self/mountinfo`, reduced to what the walker needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_point: PathBuf,
    pub fs_type: String,
    pub source: String,
}

/// Filesystem types that are never descended into.
const VIRTUAL_FS: &[&str] = &[
    "proc",
    "sysfs",
    "devtmpfs",
    "devpts",
    "tmpfs",
    "cgroup",
    "cgroup2",
    "securityfs",
    "debugfs",
    "tracefs",
    "pstore",
    "bpf",
    "mqueue",
    "hugetlbfs",
    "autofs",
    "configfs",
    "fusectl",
    "binfmt_misc",
    "overlay",
    "squashfs",
    "nsfs",
    "efivarfs",
    "ramfs",
    "fuse",
    "sshfs",
    "cifs",
    "smb3",
    "nfs",
    "nfs4",
];

/// Reads and parses the current process's mount table. Empty on failure.
pub fn read_mounts() -> Vec<MountEntry> {
    std::fs::read_to_string("/proc/self/mountinfo")
        .map(|text| parse_mountinfo(&text))
        .unwrap_or_default()
}

/// Parses `mountinfo` text. Malformed lines are skipped.
pub fn parse_mountinfo(text: &str) -> Vec<MountEntry> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<MountEntry> {
    let (left, right) = line.split_once(" - ")?;
    let mount_point = left.split_whitespace().nth(4)?;
    let mut right = right.split_whitespace();
    let fs_type = right.next()?;
    let source = right.next().unwrap_or_default();
    Some(MountEntry {
        mount_point: PathBuf::from(unescape_octal(mount_point)),
        fs_type: fs_type.to_string(),
        source: unescape_octal(source),
    })
}

/// Decodes the `\NNN` octal escapes mountinfo uses for spaces, tabs and backslashes.
fn unescape_octal(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if let Some(byte) = octal_escape(bytes.get(i..i + 4)) {
            out.push(byte);
            i += 4;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn octal_escape(window: Option<&[u8]>) -> Option<u8> {
    let &[b'\\', a, b, c] = window? else {
        return None;
    };
    if ![a, b, c].iter().all(|d| (b'0'..=b'7').contains(d)) {
        return None;
    }
    let value = u16::from(a - b'0') * 64 + u16::from(b - b'0') * 8 + u16::from(c - b'0');
    u8::try_from(value).ok()
}

/// True for filesystems that are never walked (virtual, pseudo, network, FUSE).
pub fn is_virtual_fs(fs_type: &str) -> bool {
    VIRTUAL_FS.contains(&fs_type) || fs_type.starts_with("fuse.")
}

/// Decides whether a mount point is skipped.
///
/// Skipped when the filesystem is virtual or network-backed, or when its source
/// differs from the source of the filesystem holding the scan root. With no known
/// root source only the filesystem type is checked.
pub fn should_skip_mount(entry: &MountEntry, root_source: Option<&str>) -> bool {
    is_virtual_fs(&entry.fs_type) || root_source.is_some_and(|source| source != entry.source)
}

/// The mount that holds `path`: the longest mount point that prefixes it.
/// Later entries win ties, matching overmounts.
pub fn mount_for<'a>(mounts: &'a [MountEntry], path: &Path) -> Option<&'a MountEntry> {
    let mut best: Option<&MountEntry> = None;
    for entry in mounts {
        if !path.starts_with(&entry.mount_point) {
            continue;
        }
        let deeper_or_equal = best.is_none_or(|current| {
            entry.mount_point.components().count() >= current.mount_point.components().count()
        });
        if deeper_or_equal {
            best = Some(entry);
        }
    }
    best
}

/// Whether the block device under `path` reports itself as rotational.
/// Any failure counts as solid state. Partitions fall back to their parent device.
pub fn device_is_rotational(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let dev = meta.dev();
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    let base = format!("/sys/dev/block/{major}:{minor}");
    [format!("{base}/queue/rotational"), format!("{base}/../queue/rotational")]
        .iter()
        .find_map(|file| std::fs::read_to_string(file).ok())
        .is_some_and(|value| value.trim() == "1")
}

/// Mount points indexed for the walker, with the root filesystem source of each scan.
#[derive(Clone, Debug, Default)]
pub struct MountTable {
    by_point: HashMap<PathBuf, MountEntry>,
    home_source: Option<String>,
    system_source: Option<String>,
}

impl MountTable {
    /// Indexes `entries`, recording the sources of the filesystems holding `home` and `/`.
    pub fn new(entries: Vec<MountEntry>, home: &Path) -> Self {
        let home_source = mount_for(&entries, home).map(|entry| entry.source.clone());
        let system_source = mount_for(&entries, Path::new("/")).map(|entry| entry.source.clone());
        let by_point = entries
            .into_iter()
            .map(|entry| (entry.mount_point.clone(), entry))
            .collect();
        Self {
            by_point,
            home_source,
            system_source,
        }
    }

    /// True when the directory at `path` is a mount point the walk must not enter.
    /// `system` selects the root filesystem source for the System scan.
    pub fn skips(&self, path: &Path, system: bool) -> bool {
        let Some(entry) = self.by_point.get(path) else {
            return false;
        };
        let root_source = if system {
            self.system_source.as_deref()
        } else {
            self.home_source.as_deref()
        };
        should_skip_mount(entry, root_source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT_LINE: &str = "44 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw";

    fn entry(mount_point: &str, fs_type: &str, source: &str) -> MountEntry {
        MountEntry {
            mount_point: PathBuf::from(mount_point),
            fs_type: fs_type.to_string(),
            source: source.to_string(),
        }
    }

    #[test]
    fn parses_fields_and_optional_tags() {
        let text = format!(
            "{ROOT_LINE}\n34 44 0:29 / /run rw,nosuid shared:13 master:1 - tmpfs tmpfs rw,size=1k\n"
        );
        let entries = parse_mountinfo(&text);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], entry("/", "ext4", "/dev/nvme0n1p2"));
        assert_eq!(entries[1], entry("/run", "tmpfs", "tmpfs"));
    }

    #[test]
    fn unescapes_octal_in_mount_point_and_source() {
        let line = "50 44 8:17 / /media/me/My\\040Disk\\011x\\134y rw - vfat /dev/sdb\\040\\101 rw";
        let entries = parse_mountinfo(line);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mount_point, PathBuf::from("/media/me/My Disk\tx\\y"));
        assert_eq!(entries[0].source, "/dev/sdb A");
    }

    #[test]
    fn non_escape_backslashes_and_bad_octal_are_kept() {
        assert_eq!(unescape_octal("a\\12b"), "a\\12b");
        assert_eq!(unescape_octal("a\\999"), "a\\999");
        assert_eq!(unescape_octal("\\040"), " ");
    }

    #[test]
    fn malformed_lines_are_ignored() {
        assert!(parse_mountinfo("garbage without separator").is_empty());
        assert!(parse_mountinfo("1 2 3 - ext4").is_empty());
    }

    #[test]
    fn virtual_and_network_filesystems_are_skipped() {
        for fs in ["proc", "sysfs", "tmpfs", "squashfs", "fuse.sshfs", "fuse", "nfs4", "cifs", "overlay"] {
            let mount = entry("/x", fs, "whatever");
            assert!(should_skip_mount(&mount, Some("/dev/nvme0n1p2")), "{fs}");
            assert!(should_skip_mount(&mount, None), "{fs}");
        }
    }

    #[test]
    fn different_source_is_skipped_but_same_source_is_descended() {
        let usb = entry("/media/usb", "ext4", "/dev/sdb1");
        assert!(should_skip_mount(&usb, Some("/dev/nvme0n1p2")));
        assert!(!should_skip_mount(&usb, None));
    }

    #[test]
    fn btrfs_subvolume_on_same_source_is_descended() {
        let lines = "36 44 0:31 /@home /home rw,relatime shared:20 - btrfs /dev/nvme0n1p2 rw,subvol=/@home\n\
            44 1 259:2 /@ / rw,relatime shared:1 - btrfs /dev/nvme0n1p2 rw,subvol=/@";
        let entries = parse_mountinfo(lines);
        let home = entries.iter().find(|e| e.mount_point == Path::new("/home")).cloned();
        let home = home.unwrap_or_else(|| entry("/home", "", ""));
        assert_eq!(home.source, "/dev/nvme0n1p2");
        assert!(!should_skip_mount(&home, Some("/dev/nvme0n1p2")));

        let table = MountTable::new(entries, Path::new("/home/tester"));
        assert!(!table.skips(Path::new("/home"), true));
        assert!(!table.skips(Path::new("/home"), false));
    }

    #[test]
    fn mount_table_uses_root_source_per_scan_and_skips_virtual() {
        let entries = vec![
            entry("/", "ext4", "/dev/nvme0n1p2"),
            entry("/proc", "proc", "proc"),
            entry("/media/usb", "ext4", "/dev/sdb1"),
            entry("/home/tester/net", "nfs4", "server:/share"),
        ];
        let table = MountTable::new(entries, Path::new("/home/tester"));
        assert!(table.skips(Path::new("/proc"), true));
        assert!(table.skips(Path::new("/media/usb"), true));
        assert!(table.skips(Path::new("/home/tester/net"), false));
        assert!(!table.skips(Path::new("/srv"), true));
    }

    #[test]
    fn mount_for_picks_longest_prefix() {
        let entries = vec![
            entry("/", "ext4", "/dev/root"),
            entry("/home", "btrfs", "/dev/home"),
        ];
        let found = mount_for(&entries, Path::new("/home/tester/docs"));
        assert_eq!(found.map(|e| e.source.as_str()), Some("/dev/home"));
        let found = mount_for(&entries, Path::new("/homework"));
        assert_eq!(found.map(|e| e.source.as_str()), Some("/dev/root"));
    }
}
