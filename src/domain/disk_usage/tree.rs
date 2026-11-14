//! Arena-backed disk usage tree and the builder the walker writes into.

use std::cmp::Reverse;
use std::path::{Path, PathBuf};

use super::classify::DiskCategory;

/// Index of a node inside [`DiskTree::nodes`].
pub type NodeId = u32;

/// What a node represents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// A directory whose children are listed.
    Directory,
    /// A single large file kept as its own node.
    File,
    /// All remaining small files of one directory, folded together.
    OtherFiles { count: u64 },
    /// A directory that could not be listed.
    Unreadable,
}

/// One node of the disk tree.
#[derive(Clone, Debug)]
pub struct DiskNode {
    /// Last path segment; synthetic roots use "Disk", "Home", "System".
    pub name: String,
    pub parent: Option<NodeId>,
    /// Sorted by bytes, largest first, once the tree is finalized or snapshotted.
    pub children: Vec<NodeId>,
    /// Allocated bytes of the whole subtree.
    pub bytes: u64,
    pub kind: NodeKind,
    pub category: DiskCategory,
    /// True for cache roots, build outputs and their descendants.
    pub rebuildable: bool,
    /// Owning app or game, when a rule identifies one.
    pub owner: Option<String>,
    /// True for everything outside HOME.
    pub read_only: bool,
}

/// A finished or partial disk usage tree. Node 0 is the synthetic root.
#[derive(Clone, Debug, Default)]
pub struct DiskTree {
    pub nodes: Vec<DiskNode>,
    /// The synthetic "Disk" node; its children are `home` and `system`.
    pub root: NodeId,
    pub home: Option<NodeId>,
    pub system: Option<NodeId>,
    pub home_path: PathBuf,
}

impl DiskTree {
    /// The node with `id`. Panics on an id that did not come from this tree.
    pub fn node(&self, id: NodeId) -> &DiskNode {
        &self.nodes[id as usize]
    }

    /// Real filesystem path of a node. `None` for the synthetic root and folded file groups.
    pub fn path(&self, id: NodeId) -> Option<PathBuf> {
        let node = self.nodes.get(id as usize)?;
        if id == self.root || matches!(node.kind, NodeKind::OtherFiles { .. }) {
            return None;
        }
        let chain = self.ancestors(id);
        let anchor = chain
            .iter()
            .rposition(|&c| Some(c) == self.home || Some(c) == self.system)?;
        let mut path = if Some(chain[anchor]) == self.home {
            self.home_path.clone()
        } else {
            PathBuf::from("/")
        };
        for &c in &chain[anchor + 1..] {
            path.push(&self.node(c).name);
        }
        Some(path)
    }

    /// Chain from the root down to `id`, both included.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(c) = current {
            chain.push(c);
            current = self.nodes.get(c as usize).and_then(|n| n.parent);
        }
        chain.reverse();
        chain
    }
}

/// Mutable arena used by the walker. Callers hold the lock around every call.
pub(super) struct Builder {
    nodes: Vec<DiskNode>,
    home: Option<NodeId>,
    system: Option<NodeId>,
}

impl Builder {
    /// A builder holding only the synthetic "Disk" root.
    pub(super) fn new() -> Self {
        Self {
            nodes: vec![DiskNode {
                name: "Disk".to_string(),
                parent: None,
                children: Vec::new(),
                bytes: 0,
                kind: NodeKind::Directory,
                category: DiskCategory::Other,
                rebuildable: false,
                owner: None,
                read_only: false,
            }],
            home: None,
            system: None,
        }
    }

    /// Adds a node and returns its id. The caller links it to its parent.
    pub(super) fn push(&mut self, node: DiskNode) -> NodeId {
        let id = self.nodes.len() as NodeId;
        self.nodes.push(node);
        id
    }

    pub(super) fn node_mut(&mut self, id: NodeId) -> &mut DiskNode {
        &mut self.nodes[id as usize]
    }

    pub(super) fn set_home(&mut self, id: NodeId) {
        self.home = Some(id);
    }

    pub(super) fn set_system(&mut self, id: NodeId) {
        self.system = Some(id);
    }

    /// Adds `bytes` to `id` and every ancestor, keeping parent totals exact.
    pub(super) fn add_bytes(&mut self, id: NodeId, bytes: u64) {
        let mut current = Some(id);
        while let Some(c) = current {
            let node = &mut self.nodes[c as usize];
            node.bytes = node.bytes.saturating_add(bytes);
            current = node.parent;
        }
    }

    /// Copy of the tree limited to nodes at depth `max_depth` or less, children sorted by size.
    /// Nodes at the limit keep their totals but no children.
    pub(super) fn pruned(&self, max_depth: usize, home_path: &Path) -> DiskTree {
        // `source[i]` is the arena id behind output node `i`; BFS order makes ids dense.
        let mut source: Vec<NodeId> = vec![0];
        let mut depths: Vec<usize> = vec![0];
        let mut out: Vec<DiskNode> = vec![shallow_copy(&self.nodes[0], None)];
        let mut home = None;
        let mut system = None;
        let mut i = 0;
        while i < source.len() {
            if depths[i] < max_depth {
                let old = source[i];
                for &child in &self.nodes[old as usize].children {
                    let new = out.len() as NodeId;
                    if Some(child) == self.home {
                        home = Some(new);
                    }
                    if Some(child) == self.system {
                        system = Some(new);
                    }
                    out.push(shallow_copy(&self.nodes[child as usize], Some(i as NodeId)));
                    out[i].children.push(new);
                    source.push(child);
                    depths.push(depths[i] + 1);
                }
            }
            i += 1;
        }
        sort_children(&mut out);
        DiskTree {
            nodes: out,
            root: 0,
            home,
            system,
            home_path: home_path.to_path_buf(),
        }
    }

    /// Consumes the builder into a complete, sorted tree.
    pub(super) fn finish(mut self, home_path: &Path) -> DiskTree {
        sort_children(&mut self.nodes);
        DiskTree {
            nodes: self.nodes,
            root: 0,
            home: self.home,
            system: self.system,
            home_path: home_path.to_path_buf(),
        }
    }
}

/// Copies a node's fields without its child list.
fn shallow_copy(node: &DiskNode, parent: Option<NodeId>) -> DiskNode {
    DiskNode {
        name: node.name.clone(),
        parent,
        children: Vec::new(),
        bytes: node.bytes,
        kind: node.kind,
        category: node.category,
        rebuildable: node.rebuildable,
        owner: node.owner.clone(),
        read_only: node.read_only,
    }
}

fn sort_children(nodes: &mut [DiskNode]) {
    let bytes: Vec<u64> = nodes.iter().map(|n| n.bytes).collect();
    for node in nodes.iter_mut() {
        node.children.sort_by_key(|&c| Reverse(bytes[c as usize]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, parent: Option<NodeId>, kind: NodeKind, bytes: u64) -> DiskNode {
        DiskNode {
            name: name.to_string(),
            parent,
            children: Vec::new(),
            bytes,
            kind,
            category: DiskCategory::Other,
            rebuildable: false,
            owner: None,
            read_only: false,
        }
    }

    /// Disk(0) -> Home(1) -> {small(2, 10 bytes), big(3, 50 bytes) -> deep(4, 40)}
    ///         -> System(5) -> usr(6, 7 bytes)
    fn sample() -> Builder {
        let mut b = Builder::new();
        let home = b.push(node("Home", Some(0), NodeKind::Directory, 0));
        let small = b.push(node("small", Some(home), NodeKind::File, 10));
        let big = b.push(node("big", Some(home), NodeKind::Directory, 0));
        let deep = b.push(node("deep", Some(big), NodeKind::File, 40));
        let system = b.push(node("System", Some(0), NodeKind::Directory, 0));
        let usr = b.push(node("usr", Some(system), NodeKind::Directory, 7));
        b.node_mut(0).children = vec![home, system];
        b.node_mut(home).children = vec![small, big];
        b.node_mut(big).children = vec![deep];
        b.node_mut(system).children = vec![usr];
        b.set_home(home);
        b.set_system(system);
        b.add_bytes(small, 10);
        b.add_bytes(deep, 40);
        b.add_bytes(usr, 7);
        b
    }

    #[test]
    fn path_resolves_home_and_system_anchors() {
        let tree = sample().finish(Path::new("/home/tester"));
        assert_eq!(tree.path(0), None);
        assert_eq!(tree.path(1), Some(PathBuf::from("/home/tester")));
        assert_eq!(tree.path(4), Some(PathBuf::from("/home/tester/big/deep")));
        assert_eq!(tree.path(6), Some(PathBuf::from("/usr")));
    }

    #[test]
    fn path_is_none_for_other_files_group() {
        let mut b = sample();
        let other = b.push(node("2 other files", Some(1), NodeKind::OtherFiles { count: 2 }, 5));
        let tree = b.finish(Path::new("/home/tester"));
        assert_eq!(tree.path(other), None);
    }

    #[test]
    fn ancestors_run_root_first_and_end_with_id() {
        let tree = sample().finish(Path::new("/home/tester"));
        assert_eq!(tree.ancestors(4), vec![0, 1, 3, 4]);
        assert_eq!(tree.ancestors(0), vec![0]);
    }

    #[test]
    fn add_bytes_propagates_to_every_ancestor() {
        let tree = sample().finish(Path::new("/home/tester"));
        assert_eq!(tree.node(0).bytes, 10 + 40 + 7);
        assert_eq!(tree.node(1).bytes, 50);
        assert_eq!(tree.node(3).bytes, 40);
    }

    #[test]
    fn finish_sorts_children_by_bytes_descending() {
        let tree = sample().finish(Path::new("/home/tester"));
        assert_eq!(tree.node(1).children, vec![3, 2]);
        assert_eq!(tree.node(0).children, vec![1, 5]);
    }

    #[test]
    fn pruned_keeps_depth_and_remaps_anchors() {
        let b = sample();
        let tree = b.pruned(1, Path::new("/home/tester"));
        assert_eq!(tree.nodes.len(), 3, "root, home and system only");
        let home = tree.home.map(|id| tree.node(id));
        assert_eq!(home.map(|n| n.name.as_str()), Some("Home"));
        assert_eq!(home.map(|n| n.bytes), Some(50));
        assert!(tree.node(tree.home.unwrap_or(0)).children.is_empty());
        assert_eq!(tree.node(tree.system.unwrap_or(0)).bytes, 7);
        assert_eq!(tree.path(tree.home.unwrap_or(0)), Some(PathBuf::from("/home/tester")));
    }

    #[test]
    fn pruned_children_are_sorted_and_parents_are_remapped() {
        let tree = sample().pruned(3, Path::new("/home/tester"));
        let home = tree.home.unwrap_or(0);
        let names: Vec<&str> = tree
            .node(home)
            .children
            .iter()
            .map(|&c| tree.node(c).name.as_str())
            .collect();
        assert_eq!(names, vec!["big", "small"]);
        let deep = tree.node(home).children[0];
        let deep_leaf = tree.node(deep).children[0];
        assert_eq!(tree.node(deep_leaf).name, "deep");
        assert_eq!(tree.node(deep_leaf).parent, Some(deep));
        assert_eq!(tree.path(deep_leaf), Some(PathBuf::from("/home/tester/big/deep")));
    }
}
