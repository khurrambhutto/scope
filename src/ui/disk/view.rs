//! Disk tab state: the latest snapshot, zoom focus, selection, hover, and the
//! ring transition, plus the scan lifecycle that feeds them. Drawing lives in
//! `render` and `panel`.
//!
//! Node ids are not stable across snapshots (each pruned copy renumbers its
//! nodes), so the focus and selection are kept as name paths and re-resolved
//! whenever a snapshot arrives.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{Context, Window};

use super::geometry::{self, RingNode, Segment, RING_RADII};
use super::motion::Transition;
use crate::backend;
use crate::domain::disk_usage::{DiskCategory, DiskTree, FsUsage, NodeId, NodeKind, ScanProgress};
use crate::ui::app_view::ScopeApp;
use crate::ui::filters::ViewMode;

/// Length of a zoom tween.
const ZOOM: Duration = Duration::from_millis(620);
/// Sweep played the first time rings appear.
const INTRO: Duration = Duration::from_millis(900);

/// A running tween. `started` anchors `Transition::sample`, so an intro can be
/// retargeted to a newer layout without restarting its sweep.
struct Motion {
    transition: Transition,
    started: Instant,
    intro: bool,
}

/// Disk tab state, owned by `ScopeApp`.
#[derive(Default)]
pub(in crate::ui) struct DiskState {
    /// Latest snapshot, `None` until the first one arrives.
    pub(super) tree: Option<Arc<DiskTree>>,
    pub(super) usage: FsUsage,
    /// Folders the walk could not list, and entries it has visited so far.
    pub(super) unreadable: u64,
    pub(super) entries: u64,
    pub(super) scanning: bool,
    /// Set when the scan cannot start.
    pub(super) error: Option<String>,
    /// Bytes per category, largest first with System and Other last; zero totals omitted.
    pub(super) totals: Vec<(DiskCategory, u64)>,
    /// Node at the centre of the rings.
    pub(super) focus: NodeId,
    focus_names: Vec<String>,
    pub(super) selected: Option<NodeId>,
    selected_names: Option<Vec<String>>,
    pub(super) hovered: Option<NodeId>,
    /// Settled segments for the focus. A running tween draws `motion` instead.
    pub(super) layout: Vec<Segment>,
    motion: Option<Motion>,
    intro_played: bool,
    started: bool,
    /// Identifies the running scan. Snapshots from older scans are dropped.
    generation: u64,
    cancel: Option<Arc<AtomicBool>>,
}

impl DiskState {
    /// Segments to draw at `now`, and whether a tween is still running.
    pub(super) fn frame(&mut self, now: Instant) -> (Vec<(Segment, f32)>, bool) {
        if let Some(motion) = &self.motion {
            let elapsed = now.saturating_duration_since(motion.started);
            if !motion.transition.done(elapsed) {
                return (motion.transition.sample(elapsed), true);
            }
            self.motion = None;
        }
        (
            self.layout.iter().cloned().map(|s| (s, 1.)).collect(),
            false,
        )
    }

    /// Replaces the tree and re-lays out at once. Only a running intro survives:
    /// it retargets to the new layout and keeps its clock.
    fn apply_snapshot(&mut self, progress: ScanProgress, now: Instant, reduce_motion: bool) {
        let tree = progress.tree;
        self.usage = progress.usage;
        self.unreadable = progress.unreadable_dirs;
        self.entries = progress.entries_seen;
        self.scanning = !progress.complete;
        self.totals = category_totals(&tree);

        let (focus, matched) = resolve_prefix(&tree, &self.focus_names);
        self.focus_names.truncate(matched);
        self.focus = focus;
        self.selected = self
            .selected_names
            .as_ref()
            .and_then(|names| resolve(&tree, names));
        if self.selected.is_none() {
            self.selected_names = None;
        }
        self.hovered = None;
        self.layout = ring_layout(&tree, focus);
        self.tree = Some(tree);

        self.motion = self
            .motion
            .take()
            .filter(|m| m.intro && !m.transition.done(now.saturating_duration_since(m.started)))
            .map(|m| Motion {
                transition: Transition::new(&[], &self.layout, true, INTRO),
                started: m.started,
                intro: true,
            });

        if !self.intro_played && !self.layout.is_empty() {
            self.intro_played = true;
            if !reduce_motion {
                self.motion = Some(Motion {
                    transition: Transition::new(&[], &self.layout, true, INTRO),
                    started: now,
                    intro: true,
                });
            }
        }
    }

    /// Starts a new scan, cancelling the previous one. The old tree stays on
    /// screen until the first snapshot of the new scan arrives.
    fn begin_scan(&mut self) -> (u64, Arc<AtomicBool>) {
        if let Some(previous) = self.cancel.take() {
            previous.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        self.started = true;
        self.scanning = true;
        self.error = None;
        (self.generation, cancel)
    }

    /// Zooms to `id`, tweening from what is on screen unless motion is reduced.
    fn zoom_to(&mut self, id: NodeId, select: Option<NodeId>, now: Instant, reduce_motion: bool) {
        let Some(tree) = self.tree.clone() else {
            return;
        };
        // A click can carry an id from a tree that a snapshot has since replaced.
        if tree.nodes.get(id as usize).is_none() {
            return;
        }
        let previous: Vec<Segment> = if reduce_motion {
            Vec::new()
        } else {
            self.frame(now).0.into_iter().map(|(s, _)| s).collect()
        };
        self.focus = id;
        self.focus_names = names_below_root(&tree, id);
        self.hovered = None;
        self.layout = ring_layout(&tree, id);
        self.select(select);
        if !reduce_motion {
            self.motion = Some(Motion {
                transition: Transition::new(&previous, &self.layout, false, ZOOM),
                started: now,
                intro: false,
            });
        }
    }

    /// Zooms to the parent of the focus and selects the node we came from.
    fn zoom_up(&mut self, now: Instant, reduce_motion: bool) {
        let Some(tree) = self.tree.clone() else {
            return;
        };
        let Some(parent) = tree.node(self.focus).parent else {
            return;
        };
        let came_from = self.focus;
        self.zoom_to(parent, Some(came_from), now, reduce_motion);
    }

    /// Escape clears the selection first, then zooms out one level.
    /// The node the panel card describes: a selected file inside the focus,
    /// otherwise the focus itself. A selected folder is only highlighted.
    pub(super) fn card_node(&self, tree: &DiskTree) -> NodeId {
        self.selected
            .filter(|&id| {
                tree.nodes
                    .get(id as usize)
                    .is_some_and(|node| node.children.is_empty() && node.parent == Some(self.focus))
            })
            .unwrap_or(self.focus)
    }

    fn escape(&mut self, now: Instant, reduce_motion: bool) {
        let card_shows_selection = self
            .tree
            .as_ref()
            .is_some_and(|tree| self.card_node(tree) != self.focus);
        if card_shows_selection {
            self.select(None);
        } else {
            self.zoom_up(now, reduce_motion);
        }
    }

    fn select(&mut self, id: Option<NodeId>) {
        self.selected = id;
        self.selected_names =
            id.and_then(|id| self.tree.as_ref().map(|tree| names_below_root(tree, id)));
    }

    fn toggle(&mut self, id: NodeId) {
        self.select((self.selected != Some(id)).then_some(id));
    }
}

impl ScopeApp {
    /// Starts the scan the first time the Disk tab is shown. Later visits keep
    /// the running or finished scan.
    pub(in crate::ui) fn ensure_disk_scan(&mut self, cx: &mut Context<Self>) {
        if !self.disk.started {
            self.restart_disk_scan(cx);
        }
    }

    /// Cancels any running disk scan and starts a fresh one.
    pub(in crate::ui) fn restart_disk_scan(&mut self, cx: &mut Context<Self>) {
        if std::env::var_os("HOME").is_none() {
            self.disk.error = Some("Disk usage needs a HOME folder.".to_string());
            cx.notify();
            return;
        }
        let (generation, cancel) = self.disk.begin_scan();
        let (tx, mut rx) = mpsc::unbounded::<ScanProgress>();
        backend::spawn_disk_scan(cancel, tx);
        cx.spawn(async move |this, cx| {
            while let Some(progress) = rx.next().await {
                let accepted = this
                    .update(cx, |this, cx| {
                        if this.disk.generation != generation {
                            return false;
                        }
                        let reduce_motion = cx.reduce_motion();
                        this.disk
                            .apply_snapshot(progress, Instant::now(), reduce_motion);
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !accepted {
                    return;
                }
            }
            // The worker ended: a complete scan, a cancelled one, or a panic.
            let _ = this.update(cx, |this, cx| {
                if this.disk.generation == generation {
                    this.disk.scanning = false;
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    /// Escape on the Disk tab, after dialogs and menus have had their turn.
    pub(in crate::ui) fn disk_escape(&mut self, cx: &mut Context<Self>) {
        let reduce_motion = cx.reduce_motion();
        self.disk.escape(Instant::now(), reduce_motion);
        cx.notify();
    }

    /// Zooms to a node that has children, or toggles selection on a leaf. Other
    /// folded and unreadable entries cannot be selected.
    pub(super) fn disk_activate(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let Some(tree) = self.disk.tree.clone() else {
            return;
        };
        let Some(node) = tree.nodes.get(id as usize) else {
            return;
        };
        let (now, reduce_motion) = (Instant::now(), cx.reduce_motion());
        match node.kind {
            NodeKind::OtherFiles { .. } | NodeKind::Unreadable => return,
            _ if !node.children.is_empty() => self.disk.zoom_to(id, None, now, reduce_motion),
            _ => self.disk.toggle(id),
        }
        cx.notify();
    }

    /// Zooms to a breadcrumb or any other ancestor.
    pub(super) fn disk_zoom_to(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let reduce_motion = cx.reduce_motion();
        self.disk.zoom_to(id, None, Instant::now(), reduce_motion);
        cx.notify();
    }

    /// Clicking the hub zooms out one level.
    pub(super) fn disk_zoom_up(&mut self, cx: &mut Context<Self>) {
        let reduce_motion = cx.reduce_motion();
        self.disk.zoom_up(Instant::now(), reduce_motion);
        cx.notify();
    }

    pub(super) fn disk_hover(&mut self, id: Option<NodeId>, cx: &mut Context<Self>) {
        if self.disk.hovered != id {
            self.disk.hovered = id;
            cx.notify();
        }
    }

    /// Opens the Apps tab searching for `owner`, the name a rule attached to the node.
    pub(super) fn open_in_apps(
        &mut self,
        owner: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = ViewMode::Uninstall;
        self.open_select = None;
        self.search_input
            .update(cx, |input, cx| input.set_value(owner, window, cx));
        cx.notify();
    }
}

/// Sum of each node's own bytes (its total minus its children's), grouped by
/// category. Summing own bytes never counts a byte twice, even when a category
/// changes partway down a branch.
fn category_totals(tree: &DiskTree) -> Vec<(DiskCategory, u64)> {
    let mut sums = [0u64; DiskCategory::ALL.len()];
    for node in &tree.nodes {
        let children = node.children.iter().fold(0u64, |sum, &child| {
            sum.saturating_add(tree.node(child).bytes)
        });
        let own = node.bytes.saturating_sub(children);
        if let Some(index) = DiskCategory::ALL.iter().position(|&c| c == node.category) {
            sums[index] = sums[index].saturating_add(own);
        }
    }
    let mut totals: Vec<(DiskCategory, u64)> = DiskCategory::ALL
        .iter()
        .copied()
        .zip(sums)
        .filter(|&(_, bytes)| bytes > 0)
        .collect();
    // Largest first; System and Other always close the list.
    totals.sort_by_key(|&(category, bytes)| {
        let tail = matches!(category, DiskCategory::System | DiskCategory::Other);
        (
            tail,
            category == DiskCategory::Other,
            std::cmp::Reverse(bytes),
        )
    });
    totals
}

/// Follows `names` from the root. Returns the deepest node reached and how many
/// names matched.
fn resolve_prefix(tree: &DiskTree, names: &[String]) -> (NodeId, usize) {
    let mut id = tree.root;
    let mut matched = 0;
    for name in names {
        let child = tree
            .node(id)
            .children
            .iter()
            .copied()
            .find(|&c| tree.node(c).name == *name);
        match child {
            Some(child) => {
                id = child;
                matched += 1;
            }
            None => break,
        }
    }
    (id, matched)
}

/// The node at the end of `names`, or `None` when any name is missing.
fn resolve(tree: &DiskTree, names: &[String]) -> Option<NodeId> {
    let (id, matched) = resolve_prefix(tree, names);
    (matched == names.len()).then_some(id)
}

/// Names from the root's children down to `id`. The root itself is omitted.
fn names_below_root(tree: &DiskTree, id: NodeId) -> Vec<String> {
    tree.ancestors(id)
        .into_iter()
        .skip(1)
        .map(|c| tree.node(c).name.clone())
        .collect()
}

/// Ring nodes for `focus` and its descendants down to the last ring, then the
/// settled layout. Nodes past the last ring keep their totals as leaves.
fn ring_layout(tree: &DiskTree, focus: NodeId) -> Vec<Segment> {
    let mut nodes = Vec::new();
    push_ring_node(tree, focus, 0, &mut nodes);
    geometry::layout(&nodes)
}

/// Adds `id`, and its children while the ring depth allows. `level` is the
/// distance from the focus, so a child at level `l + 1` is drawn on ring `l`.
fn push_ring_node(tree: &DiskTree, id: NodeId, level: usize, out: &mut Vec<RingNode>) -> usize {
    let index = out.len();
    let node = tree.node(id);
    out.push(RingNode {
        key: u64::from(id),
        value: node.bytes as f64,
        children: Vec::new(),
    });
    if level < RING_RADII.len() {
        let children = node
            .children
            .iter()
            .map(|&child| push_ring_node(tree, child, level + 1, out))
            .collect();
        out[index].children = children;
    }
    index
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::disk_usage::tree::DiskNode;

    fn node(
        name: &str,
        parent: Option<NodeId>,
        children: Vec<NodeId>,
        bytes: u64,
        kind: NodeKind,
        category: DiskCategory,
    ) -> DiskNode {
        DiskNode {
            name: name.to_string(),
            parent,
            children,
            bytes,
            kind,
            category,
            rebuildable: false,
            owner: None,
            read_only: false,
        }
    }

    /// Disk(0) -> Home(1, Other) -> code(2, Developer) -> target(3, Developer, 30 bytes)
    ///                            -> Music(4, Music, 5 bytes)
    fn sample() -> DiskTree {
        use DiskCategory::{Developer, Music, Other};
        use NodeKind::{Directory, File};
        DiskTree {
            nodes: vec![
                node("Disk", None, vec![1], 35, Directory, Other),
                node("Home", Some(0), vec![2, 4], 35, Directory, Other),
                node("code", Some(1), vec![3], 30, Directory, Developer),
                node("target", Some(2), vec![], 30, File, Developer),
                node("Music", Some(1), vec![], 5, File, Music),
            ],
            root: 0,
            home: Some(1),
            system: None,
            home_path: PathBuf::from("/home/tester"),
        }
    }

    #[test]
    fn category_totals_count_each_byte_once() {
        let tree = sample();
        assert_eq!(
            category_totals(&tree),
            vec![(DiskCategory::Developer, 30), (DiskCategory::Music, 5)]
        );
    }

    #[test]
    fn names_resolve_back_to_the_same_node() {
        let tree = sample();
        let names = names_below_root(&tree, 3);
        assert_eq!(names, vec!["Home", "code", "target"]);
        assert_eq!(resolve(&tree, &names), Some(3));
        assert_eq!(
            resolve(&tree, &["Home".to_string(), "gone".to_string()]),
            None
        );
    }

    #[test]
    fn resolve_prefix_stops_at_the_deepest_match() {
        let tree = sample();
        let names = vec![
            "Home".to_string(),
            "code".to_string(),
            "missing".to_string(),
        ];
        assert_eq!(resolve_prefix(&tree, &names), (2, 2));
    }

    #[test]
    fn ring_layout_draws_the_focus_children_first() {
        let tree = sample();
        let segments = ring_layout(&tree, 1);
        let keys: Vec<u64> = segments.iter().map(|s| s.key).collect();
        assert!(
            keys.contains(&2) && keys.contains(&4),
            "home children on ring 0"
        );
        assert!(keys.contains(&3), "grandchild on ring 1");
        assert!(
            segments.iter().all(|s| s.key != 1),
            "the focus is the hub, not a segment"
        );
    }

    #[test]
    fn toggle_selects_then_clears() {
        let mut state = DiskState {
            tree: Some(Arc::new(sample())),
            ..DiskState::default()
        };
        state.toggle(4);
        assert_eq!(state.selected, Some(4));
        state.toggle(4);
        assert_eq!(state.selected, None);
    }

    #[test]
    fn escape_clears_selection_before_zooming_out() {
        let now = Instant::now();
        let mut state = DiskState {
            tree: Some(Arc::new(sample())),
            focus: 2,
            focus_names: vec!["Home".to_string(), "code".to_string()],
            ..DiskState::default()
        };
        state.select(Some(3));
        state.escape(now, true);
        assert_eq!(state.selected, None);
        assert_eq!(state.focus, 2, "first escape only clears the selection");
        state.escape(now, true);
        assert_eq!(state.focus, 1, "second escape zooms to the parent");
        assert_eq!(state.selected, Some(2), "the node we came from is selected");
    }

    #[test]
    fn frame_without_motion_is_the_settled_layout() {
        let tree = Arc::new(sample());
        let layout = ring_layout(&tree, 0);
        let mut state = DiskState {
            tree: Some(tree),
            layout,
            ..DiskState::default()
        };
        let (frame, animating) = state.frame(Instant::now());
        assert!(!animating);
        assert_eq!(frame.len(), state.layout.len());
        assert!(frame.iter().all(|(_, opacity)| *opacity == 1.));
    }

    #[test]
    fn panel_stays_at_the_rings_level_after_zooming_out() {
        let now = Instant::now();
        let mut state = DiskState {
            tree: Some(Arc::new(sample())),
            focus: 1,
            focus_names: vec!["Home".to_string()],
            ..DiskState::default()
        };
        let tree = sample();
        state.zoom_up(now, true);
        assert_eq!(state.focus, 0);
        // The folder we came from is highlighted, but the card shows the focus.
        assert_eq!(state.selected, Some(1));
        assert_eq!(state.card_node(&tree), 0);
        // Escape keeps going up instead of clearing a highlight first.
        state.zoom_to(2, None, now, true);
        state.select(Some(3));
        assert_eq!(state.card_node(&tree), 3);
        state.escape(now, true);
        assert_eq!(state.card_node(&tree), 2);
        state.escape(now, true);
        assert_eq!(state.focus, 1);
    }
}
