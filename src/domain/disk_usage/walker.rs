//! Parallel directory walk that streams partial snapshots of the disk usage tree.
//!
//! Each directory is one task: it lists its entries, classifies itself, writes
//! its node and totals under one arena lock, then spawns its subdirectories.
//! Totals propagate to every ancestor as each directory completes, so parent
//! sizes are exact at the end and roughly right in between.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};
use std::fs::{self, DirEntry};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::classify::{classify_dir, DiskCategory, XdgDirs};
use super::mounts::{device_is_rotational, mount_for, read_mounts, MountEntry, MountTable};
use super::tree::{Builder, DiskNode, DiskTree, NodeId, NodeKind};

/// Filesystem space numbers in bytes, from `statvfs`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FsUsage {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

/// What to scan and how often to report.
#[derive(Clone, Debug)]
pub struct ScanOptions {
    pub home: PathBuf,
    /// Also walk `/` excluding HOME and the skipped mounts.
    pub include_system: bool,
    /// Depth limit for partial snapshots; the final snapshot is never pruned.
    pub snapshot_depth: usize,
    /// Minimum time between partial snapshots.
    pub snapshot_interval: Duration,
    /// Worker threads. `None` picks a count from the CPU count and the disk type.
    pub threads: Option<usize>,
}

/// One progress report from [`scan`].
#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub tree: Arc<DiskTree>,
    pub usage: FsUsage,
    pub complete: bool,
    pub unreadable_dirs: u64,
    pub entries_seen: u64,
}

/// Folding thresholds. Tests lower them to exercise the large-file path cheaply.
#[derive(Clone, Copy, Debug)]
struct Tuning {
    large_file_min: u64,
    max_large_files: usize,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            large_file_min: 32 * 1024 * 1024,
            max_large_files: 12,
        }
    }
}

/// Top-level system paths that are never walked.
const SYSTEM_SKIP: [&str; 4] = ["/proc", "/sys", "/dev", "/run"];
const SEEN_SHARDS: usize = 64;
const MAX_THREADS: usize = 16;
const ROTATIONAL_THREADS: usize = 2;
const WORKER_STACK_BYTES: usize = 4 * 1024 * 1024;
const BLOCK_BYTES: u64 = 512;

/// Scans disk usage. Blocking: call from a background thread.
///
/// Sends partial snapshots at most every `snapshot_interval`, then exactly one
/// final snapshot with `complete == true`. A cancelled scan returns early
/// without a final snapshot.
pub fn scan(
    options: &ScanOptions,
    cancel: &AtomicBool,
    on_progress: impl FnMut(ScanProgress) + Send,
) {
    run(options, cancel, Tuning::default(), on_progress);
}

/// Statvfs numbers for the filesystem holding `home`, plus `/` when it is a different
/// filesystem. Zeroed when `home` cannot be queried.
pub fn filesystem_usage(home: &Path, mounts: &[MountEntry]) -> FsUsage {
    let mut usage = statvfs_usage(home).unwrap_or_default();
    let home_source = mount_for(mounts, home).map(|m| m.source.as_str());
    let root_source = mount_for(mounts, Path::new("/")).map(|m| m.source.as_str());
    if matches!((home_source, root_source), (Some(h), Some(r)) if h != r) {
        if let Some(root) = statvfs_usage(Path::new("/")) {
            usage.total = usage.total.saturating_add(root.total);
            usage.used = usage.used.saturating_add(root.used);
            usage.free = usage.free.saturating_add(root.free);
        }
    }
    usage
}

fn statvfs_usage(path: &Path) -> Option<FsUsage> {
    let stat = rustix::fs::statvfs(path).ok()?;
    let unit = if stat.f_frsize > 0 {
        stat.f_frsize
    } else {
        stat.f_bsize
    };
    Some(FsUsage {
        total: stat.f_blocks.saturating_mul(unit),
        free: stat.f_bavail.saturating_mul(unit),
        used: stat
            .f_blocks
            .saturating_sub(stat.f_bfree)
            .saturating_mul(unit),
    })
}

/// Class inherited by a directory's children, or the fixed class of a root.
#[derive(Clone, Debug)]
struct Class {
    category: DiskCategory,
    rebuildable: bool,
    owner: Option<String>,
    read_only: bool,
}

/// Shared state for one scan.
struct Ctx<'a> {
    home: PathBuf,
    xdg: XdgDirs,
    mounts: MountTable,
    cancel: &'a AtomicBool,
    tuning: Tuning,
    usage: FsUsage,
    arena: Mutex<Builder>,
    seen: SeenSet,
    entries_seen: AtomicU64,
    unreadable_dirs: AtomicU64,
}

fn run(
    options: &ScanOptions,
    cancel: &AtomicBool,
    tuning: Tuning,
    mut on_progress: impl FnMut(ScanProgress) + Send,
) {
    let mounts = read_mounts();
    let usage = filesystem_usage(&options.home, &mounts);
    let threads = thread_count(options);
    let ctx = Ctx {
        home: options.home.clone(),
        xdg: XdgDirs::current(&options.home),
        mounts: MountTable::new(mounts, &options.home),
        cancel,
        tuning,
        usage,
        arena: Mutex::new(Builder::new()),
        seen: SeenSet::new(),
        entries_seen: AtomicU64::new(0),
        unreadable_dirs: AtomicU64::new(0),
    };
    let (home_id, system_id) = ctx.create_roots(options.include_system);
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(WORKER_STACK_BYTES)
        .build()
        .ok();

    let (done_tx, done_rx) = mpsc::channel::<()>();
    std::thread::scope(|scope| {
        let ctx = &ctx;
        scope.spawn(move || {
            let walk = || walk_roots(ctx, home_id, system_id);
            match &pool {
                Some(pool) => pool.install(walk),
                None => walk(),
            }
            // Dropping the sender tells the reporter the walk is over.
            drop(done_tx);
        });
        while let Err(mpsc::RecvTimeoutError::Timeout) =
            done_rx.recv_timeout(options.snapshot_interval)
        {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let tree = ctx.snapshot(options.snapshot_depth);
            on_progress(ctx.progress(tree, false));
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return;
    }
    let tree = ctx.finish();
    on_progress(ctx.progress(tree, true));
}

fn thread_count(options: &ScanOptions) -> usize {
    if let Some(threads) = options.threads {
        return threads.max(1);
    }
    if device_is_rotational(&options.home) {
        return ROTATIONAL_THREADS;
    }
    std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .min(MAX_THREADS)
}

fn walk_roots(ctx: &Ctx, home_id: NodeId, system_id: Option<NodeId>) {
    let home_class = Class {
        category: DiskCategory::Other,
        rebuildable: false,
        owner: None,
        read_only: false,
    };
    walk_dir(ctx, &ctx.home, home_id, "Home", &home_class, true);
    if let Some(system_id) = system_id {
        let system_class = Class {
            category: DiskCategory::System,
            rebuildable: false,
            owner: None,
            read_only: true,
        };
        walk_dir(
            ctx,
            Path::new("/"),
            system_id,
            "System",
            &system_class,
            true,
        );
    }
}

/// Lists one directory, writes its node, then walks its subdirectories in parallel.
///
/// `parent` is the class the directory inherits from. `is_root` means `parent`
/// is already this directory's final class.
fn walk_dir(ctx: &Ctx, path: &Path, id: NodeId, name: &str, parent: &Class, is_root: bool) {
    if ctx.is_cancelled() {
        return;
    }
    let system = parent.read_only;
    let Ok(read) = fs::read_dir(path) else {
        let own = resolve(ctx, path, name, parent, is_root, false);
        ctx.mark_unreadable(id, &own);
        return;
    };
    let mut collected = Collected::default();
    for entry in read {
        if ctx.is_cancelled() {
            return;
        }
        if let Ok(entry) = entry {
            collected.visit(ctx, &entry, system);
        }
    }
    let own = resolve(ctx, path, name, parent, is_root, collected.has_tag);
    let child_ids = ctx.attach(id, &own, &mut collected);
    let subdirs = std::mem::take(&mut collected.subdirs);
    if subdirs.is_empty() {
        return;
    }
    let own_ref = &own;
    rayon::scope(|s| {
        for ((child_name, child_path), child_id) in subdirs.into_iter().zip(child_ids) {
            s.spawn(move |_| walk_dir(ctx, &child_path, child_id, &child_name, own_ref, false));
        }
    });
}

fn resolve(
    ctx: &Ctx,
    path: &Path,
    name: &str,
    parent: &Class,
    is_root: bool,
    has_tag: bool,
) -> Class {
    if is_root {
        return parent.clone();
    }
    let (category, rebuildable, owner) = classify_dir(
        &ctx.home,
        &ctx.xdg,
        (parent.category, parent.rebuildable),
        path,
        name,
        has_tag,
    );
    Class {
        category,
        rebuildable,
        owner: owner.or_else(|| parent.owner.clone()),
        read_only: parent.read_only,
    }
}

/// What one directory listing produced before it is written to the arena.
#[derive(Default)]
struct Collected {
    subdirs: Vec<(String, PathBuf)>,
    local: Local,
    has_tag: bool,
}

impl Collected {
    fn visit(&mut self, ctx: &Ctx, entry: &DirEntry, system: bool) {
        ctx.entries_seen.fetch_add(1, Ordering::Relaxed);
        let Ok(file_type) = entry.file_type() else {
            return;
        };
        let name_os = entry.file_name();
        let name = name_os.to_string_lossy();
        if file_type.is_dir() {
            let path = entry.path();
            if ctx.skip_dir(&path, system) {
                return;
            }
            let Ok(meta) = entry.metadata() else {
                return;
            };
            if ctx.seen.insert(meta.dev(), meta.ino()) {
                self.subdirs.push((name.into_owned(), path));
            }
            return;
        }
        if name == "CACHEDIR.TAG" && file_type.is_file() {
            self.has_tag = true;
        }
        // Symlinks are never followed: `entry.metadata()` describes the link itself.
        let Ok(meta) = entry.metadata() else {
            return;
        };
        if meta.nlink() > 1 && !ctx.seen.insert(meta.dev(), meta.ino()) {
            return;
        }
        let size = meta.blocks().saturating_mul(BLOCK_BYTES);
        self.local.add_file(&name, size, ctx.tuning);
    }
}

/// File sizes of one directory: the largest few kept, the rest folded.
#[derive(Default)]
struct Local {
    large: BinaryHeap<Reverse<(u64, String)>>,
    other_bytes: u64,
    other_count: u64,
    total: u64,
}

impl Local {
    fn add_file(&mut self, name: &str, size: u64, tuning: Tuning) {
        self.total = self.total.saturating_add(size);
        if size < tuning.large_file_min {
            self.fold(size);
            return;
        }
        self.large.push(Reverse((size, name.to_string())));
        if self.large.len() > tuning.max_large_files {
            if let Some(Reverse((smallest, _))) = self.large.pop() {
                self.fold(smallest);
            }
        }
    }

    fn fold(&mut self, size: u64) {
        self.other_bytes = self.other_bytes.saturating_add(size);
        self.other_count += 1;
    }

    /// The kept large files, largest first.
    fn take_large(&mut self) -> Vec<(u64, String)> {
        let mut files: Vec<(u64, String)> = std::mem::take(&mut self.large)
            .into_iter()
            .map(|Reverse(file)| file)
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        files
    }
}

impl Ctx<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Directories the walk must not enter: unwanted mounts, and HOME and the
    /// virtual top-level paths while walking the system root.
    fn skip_dir(&self, path: &Path, system: bool) -> bool {
        if system && (path == self.home || SYSTEM_SKIP.iter().any(|skip| path == Path::new(skip))) {
            return true;
        }
        self.mounts.skips(path, system)
    }

    fn create_roots(&self, include_system: bool) -> (NodeId, Option<NodeId>) {
        let mut arena = lock(&self.arena);
        let home = arena.push(root_node("Home", DiskCategory::Other, false));
        arena.node_mut(0).children.push(home);
        arena.set_home(home);
        let system = include_system.then(|| {
            let system = arena.push(root_node("System", DiskCategory::System, true));
            arena.node_mut(0).children.push(system);
            arena.set_system(system);
            system
        });
        (home, system)
    }

    /// Writes a directory's class, child nodes and file totals, and propagates the totals.
    /// Returns the ids of the subdirectory nodes, in the order of `collected.subdirs`.
    fn attach(&self, id: NodeId, own: &Class, collected: &mut Collected) -> Vec<NodeId> {
        let mut arena = lock(&self.arena);
        set_class(arena.node_mut(id), own);
        let child_ids: Vec<NodeId> = collected
            .subdirs
            .iter()
            .map(|(name, _)| arena.push(new_node(name, id, NodeKind::Directory, 0, own)))
            .collect();
        let mut children = child_ids.clone();
        for (size, name) in collected.local.take_large() {
            children.push(arena.push(new_node(&name, id, NodeKind::File, size, own)));
        }
        let count = collected.local.other_count;
        if count > 0 {
            let kind = NodeKind::OtherFiles { count };
            let other = new_node(
                &other_label(count),
                id,
                kind,
                collected.local.other_bytes,
                own,
            );
            children.push(arena.push(other));
        }
        arena.node_mut(id).children = children;
        arena.add_bytes(id, collected.local.total);
        child_ids
    }

    fn mark_unreadable(&self, id: NodeId, own: &Class) {
        let mut arena = lock(&self.arena);
        let node = arena.node_mut(id);
        set_class(node, own);
        node.kind = NodeKind::Unreadable;
        self.unreadable_dirs.fetch_add(1, Ordering::Relaxed);
    }

    /// Pruned copy of the arena for a partial snapshot.
    fn snapshot(&self, depth: usize) -> DiskTree {
        lock(&self.arena).pruned(depth, &self.home)
    }

    fn finish(&self) -> DiskTree {
        let builder = std::mem::replace(&mut *lock(&self.arena), Builder::new());
        builder.finish(&self.home)
    }

    fn progress(&self, tree: DiskTree, complete: bool) -> ScanProgress {
        ScanProgress {
            tree: Arc::new(tree),
            usage: self.usage,
            complete,
            unreadable_dirs: self.unreadable_dirs.load(Ordering::Relaxed),
            entries_seen: self.entries_seen.load(Ordering::Relaxed),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn root_node(name: &str, category: DiskCategory, read_only: bool) -> DiskNode {
    DiskNode {
        name: name.to_string(),
        parent: Some(0),
        children: Vec::new(),
        bytes: 0,
        kind: NodeKind::Directory,
        category,
        rebuildable: false,
        owner: None,
        read_only,
    }
}

fn set_class(node: &mut DiskNode, class: &Class) {
    node.category = class.category;
    node.rebuildable = class.rebuildable;
    node.owner = class.owner.clone();
    node.read_only = class.read_only;
}

fn new_node(name: &str, parent: NodeId, kind: NodeKind, bytes: u64, class: &Class) -> DiskNode {
    DiskNode {
        name: name.to_string(),
        parent: Some(parent),
        children: Vec::new(),
        bytes,
        kind,
        category: class.category,
        rebuildable: class.rebuildable,
        owner: class.owner.clone(),
        read_only: class.read_only,
    }
}

fn other_label(count: u64) -> String {
    if count == 1 {
        "1 other file".to_string()
    } else {
        format!("{count} other files")
    }
}

/// Inodes already counted: hard-linked files and directories reached twice (bind mounts).
struct SeenSet {
    shards: Vec<Mutex<HashSet<(u64, u64)>>>,
}

impl SeenSet {
    fn new() -> Self {
        Self {
            shards: (0..SEEN_SHARDS)
                .map(|_| Mutex::new(HashSet::new()))
                .collect(),
        }
    }

    /// Records the inode. Returns false when it was already recorded.
    fn insert(&self, dev: u64, ino: u64) -> bool {
        let shard = ((dev ^ ino) as usize) % SEEN_SHARDS;
        lock(&self.shards[shard]).insert((dev, ino))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::os::unix::fs::PermissionsExt;

    fn options(home: &Path, include_system: bool, interval: Duration) -> ScanOptions {
        ScanOptions {
            home: home.to_path_buf(),
            include_system,
            snapshot_depth: 6,
            snapshot_interval: interval,
            threads: Some(4),
        }
    }

    /// All progress reports of a scan over `home` with `tuning`.
    fn run_scan(home: &Path, tuning: Tuning, interval: Duration) -> Vec<ScanProgress> {
        let cancel = AtomicBool::new(false);
        let mut out = Vec::new();
        run(&options(home, false, interval), &cancel, tuning, |p| {
            out.push(p)
        });
        out
    }

    /// The final, complete tree of a scan with no partial snapshots.
    fn final_tree(home: &Path, tuning: Tuning) -> Arc<DiskTree> {
        let mut all = run_scan(home, tuning, Duration::from_secs(3600));
        let last = all.pop().expect("final snapshot");
        assert!(last.complete);
        last.tree
    }

    fn write_file(path: &Path, len: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        // Pseudo-random data so compressing filesystems cannot shrink the allocation.
        let mut state = len as u64 * 2_654_435_761 + 1;
        let data: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        fs::write(path, data).expect("write");
    }

    fn allocated(path: &Path) -> u64 {
        fs::symlink_metadata(path).expect("stat").blocks() * BLOCK_BYTES
    }

    /// Allocated bytes of every non-directory under `root`, hard links counted once.
    fn expected_bytes(root: &Path) -> u64 {
        let mut seen = HashSet::new();
        let mut total = 0;
        for entry in walkdir::WalkDir::new(root).follow_links(false).min_depth(1) {
            let meta = entry.expect("walk").metadata().expect("meta");
            if meta.is_dir() {
                continue;
            }
            if meta.nlink() > 1 && !seen.insert((meta.dev(), meta.ino())) {
                continue;
            }
            total += meta.blocks() * BLOCK_BYTES;
        }
        total
    }

    fn find(tree: &DiskTree, name: &str) -> NodeId {
        (0..tree.nodes.len() as NodeId)
            .find(|&id| tree.node(id).name == name)
            .expect("node present")
    }

    fn assert_directory_totals(tree: &DiskTree) {
        for (id, node) in tree.nodes.iter().enumerate() {
            if node.kind != NodeKind::Directory {
                continue;
            }
            let sum: u64 = node.children.iter().map(|&c| tree.node(c).bytes).sum();
            assert_eq!(node.bytes, sum, "node {id} ({})", node.name);
        }
    }

    #[test]
    fn subtree_totals_match_children_and_expected_allocation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("a/x.bin"), 100_000);
        write_file(&home.join("a/b/y.bin"), 300_000);
        write_file(&home.join("c/z.txt"), 10);
        write_file(&home.join("top.bin"), 50_000);
        let tuning = Tuning {
            large_file_min: 64 * 1024,
            max_large_files: 12,
        };

        let tree = final_tree(home, tuning);
        let home_id = tree.home.expect("home node");
        assert_eq!(tree.node(0).name, "Disk");
        assert_eq!(tree.node(home_id).name, "Home");
        assert_eq!(tree.node(home_id).bytes, expected_bytes(home));
        assert_eq!(tree.node(0).bytes, tree.node(home_id).bytes);
        assert_directory_totals(&tree);

        let x = tree.node(find(&tree, "x.bin"));
        assert_eq!(x.kind, NodeKind::File);
        assert_eq!(x.bytes, allocated(&home.join("a/x.bin")));
    }

    #[test]
    fn hard_links_are_counted_once() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("one.bin"), 200_000);
        fs::create_dir_all(home.join("sub")).expect("mkdir");
        fs::hard_link(home.join("one.bin"), home.join("sub/two.bin")).expect("link");
        fs::hard_link(home.join("one.bin"), home.join("three.bin")).expect("link");

        let tree = final_tree(home, Tuning::default());
        let home_id = tree.home.expect("home node");
        assert_eq!(tree.node(home_id).bytes, allocated(&home.join("one.bin")));
        assert_eq!(tree.node(home_id).bytes, expected_bytes(home));
    }

    #[test]
    fn symlinks_are_never_followed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("real/big.bin"), 200_000);
        std::os::unix::fs::symlink(home.join("real"), home.join("link")).expect("symlink");

        let tree = final_tree(home, Tuning::default());
        let home_id = tree.home.expect("home node");
        assert_eq!(tree.node(home_id).bytes, expected_bytes(home));
        assert!(tree
            .nodes
            .iter()
            .all(|n| !(n.name == "link" && n.kind == NodeKind::Directory)));
    }

    #[test]
    fn small_files_fold_into_one_other_node() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let mut expected = 0;
        for i in 0..5 {
            let path = home.join(format!("many/f{i}.txt"));
            write_file(&path, 2_000 + i);
            expected += allocated(&path);
        }
        write_file(&home.join("single/one.txt"), 1_000);
        fs::create_dir_all(home.join("empty")).expect("mkdir");

        let tree = final_tree(home, Tuning::default());
        let many = tree.node(find(&tree, "many"));
        assert_eq!(many.children.len(), 1);
        let other = tree.node(many.children[0]);
        assert_eq!(other.kind, NodeKind::OtherFiles { count: 5 });
        assert_eq!(other.name, "5 other files");
        assert_eq!(other.bytes, expected);

        let single = tree.node(tree.node(find(&tree, "single")).children[0]);
        assert_eq!(single.name, "1 other file");
        assert_eq!(single.kind, NodeKind::OtherFiles { count: 1 });
        assert!(tree.node(find(&tree, "empty")).children.is_empty());
    }

    #[test]
    fn large_files_are_capped_per_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("big/a300.bin"), 300_000);
        write_file(&home.join("big/b250.bin"), 250_000);
        write_file(&home.join("big/c200.bin"), 200_000);
        write_file(&home.join("big/d150.bin"), 150_000);
        let tuning = Tuning {
            large_file_min: 100_000,
            max_large_files: 2,
        };

        let tree = final_tree(home, tuning);
        let big = tree.node(find(&tree, "big"));
        // Children are sorted by size, so the folded group (about 350 KB) comes first.
        let folded = allocated(&home.join("big/c200.bin")) + allocated(&home.join("big/d150.bin"));
        let other = big
            .children
            .iter()
            .map(|&c| tree.node(c))
            .find(|n| matches!(n.kind, NodeKind::OtherFiles { .. }))
            .expect("folded group");
        assert_eq!(other.kind, NodeKind::OtherFiles { count: 2 });
        assert_eq!(other.bytes, folded);
        let files: Vec<&str> = big
            .children
            .iter()
            .map(|&c| tree.node(c))
            .filter(|n| n.kind == NodeKind::File)
            .map(|n| n.name.as_str())
            .collect();
        assert_eq!(files, vec!["a300.bin", "b250.bin"]);
        assert_eq!(big.children.len(), 3);
        assert_directory_totals(&tree);
    }

    #[test]
    fn classification_flows_into_nodes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("project/node_modules/pkg/index.js"), 100);
        write_file(&home.join(".cache/app/blob.bin"), 100);
        write_file(&home.join("build_out/CACHEDIR.TAG"), 40);
        write_file(&home.join("build_out/obj.bin"), 100);

        let tree = final_tree(home, Tuning::default());
        let modules = tree.node(find(&tree, "node_modules"));
        assert_eq!(modules.category, DiskCategory::Developer);
        assert!(modules.rebuildable);
        let pkg = tree.node(find(&tree, "pkg"));
        assert!(pkg.rebuildable);
        assert_eq!(pkg.category, DiskCategory::Developer);
        let cache = tree.node(find(&tree, ".cache"));
        assert_eq!(cache.category, DiskCategory::Cache);
        assert!(cache.rebuildable);
        let build = tree.node(find(&tree, "build_out"));
        assert_eq!(build.category, DiskCategory::Cache);
        assert!(build.rebuildable);
        assert!(!tree.node(find(&tree, "project")).rebuildable);
    }

    #[test]
    fn unreadable_directory_is_marked_and_counted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let locked = home.join("locked");
        write_file(&locked.join("secret.bin"), 1_000);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
        if fs::read_dir(&locked).is_ok() {
            // Running as root: permissions do not restrict reads.
            fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("chmod");
            return;
        }

        let progress = run_scan(home, Tuning::default(), Duration::from_secs(3600));
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).expect("chmod");
        let last = progress.last().expect("final snapshot");
        let node = last.tree.node(find(&last.tree, "locked"));
        assert_eq!(node.kind, NodeKind::Unreadable);
        assert_eq!(node.bytes, 0);
        assert!(node.children.is_empty());
        assert_eq!(last.unreadable_dirs, 1);
    }

    #[test]
    fn cancelled_scan_sends_no_snapshots() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_file(&dir.path().join("a.bin"), 1_000);
        let cancel = AtomicBool::new(true);
        let mut count = 0;
        scan(&options(dir.path(), false, Duration::ZERO), &cancel, |_| {
            count += 1
        });
        assert_eq!(count, 0);
    }

    #[test]
    fn exactly_one_final_snapshot_and_partials_are_pruned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        write_file(&home.join("a/b/c/d/deep.bin"), 5_000);
        for i in 0..40 {
            write_file(&home.join(format!("d{}/f{i}.txt", i % 4)), 3_000);
        }
        let cancel = AtomicBool::new(false);
        let mut seen = Vec::new();
        let scan_options = ScanOptions {
            snapshot_depth: 2,
            snapshot_interval: Duration::ZERO,
            ..options(home, false, Duration::ZERO)
        };
        run(&scan_options, &cancel, Tuning::default(), |p| seen.push(p));

        let (last, partial) = seen.split_last().expect("final snapshot");
        assert!(last.complete);
        assert!(partial.iter().all(|p| !p.complete));
        assert_eq!(seen.iter().filter(|p| p.complete).count(), 1);
        for progress in partial {
            for id in 0..progress.tree.nodes.len() as NodeId {
                assert!(progress.tree.ancestors(id).len() <= 3, "depth limit of 2");
            }
        }
        let deepest = find(&last.tree, "d");
        assert!(last.tree.ancestors(deepest).len() > 3);
        assert_eq!(
            last.tree.node(last.tree.home.expect("home")).bytes,
            expected_bytes(home)
        );
        assert_directory_totals(&last.tree);
    }

    #[test]
    fn filesystem_usage_reports_space_for_a_temp_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let usage = filesystem_usage(dir.path(), &read_mounts());
        assert!(usage.total > 0);
        assert!(usage.used + usage.free <= usage.total);
    }
}
