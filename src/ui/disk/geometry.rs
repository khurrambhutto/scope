//! Sunburst geometry in design space: a 580 x 580 square whose centre is
//! `CENTRE`. Angles are radians, 0 at 12 o'clock and increasing clockwise,
//! as in the disk prototype. Everything here is pure and domain-agnostic.

use std::f32::consts::TAU;

/// Centre of the design square.
pub const CENTRE: f32 = 290.;
/// Side of the design square. Callers scale it to fit their bounds.
pub const DESIGN_SIZE: f32 = 580.;
/// Inner and outer radius of each ring, innermost first.
pub const RING_RADII: [(f32, f32); 4] = [(100., 160.), (164., 212.), (216., 250.), (254., 272.)];
/// Radius of the hub disc at the centre, which shows the zoom focus.
pub const HUB_RADIUS: f32 = 90.;
/// Visual gap between neighbouring segments, in design units.
pub const SEGMENT_GAP: f32 = 2.4;
/// Smallest angular span in radians that is laid out and drawn.
pub const MIN_SPAN: f32 = 0.003;

/// One node of the tree handed to [`layout`]. Index 0 is the zoom focus, drawn
/// as the hub rather than as a segment.
#[derive(Clone, Debug)]
pub struct RingNode {
    pub key: u64,
    /// Size of a leaf. A node with children is sized by the sum of its children.
    pub value: f64,
    /// Child indices, sorted by value descending by the caller. The tree must be acyclic.
    pub children: Vec<usize>,
}

/// Angular and radial extent of a segment before any gap or rounding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arc {
    pub a0: f32,
    pub a1: f32,
    pub r0: f32,
    pub r1: f32,
}

/// A laid-out segment. `node` indexes the slice given to [`layout`], so it is
/// only meaningful for the layout that produced it. Use `key` to look up data.
#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub key: u64,
    pub node: usize,
    /// Ring index, `0..RING_RADII.len()`.
    pub depth: usize,
    /// Position among the parent's children.
    pub sibling_index: usize,
    pub arc: Arc,
}

/// Centreline of a drawn segment. The visible shape is this sector dilated by
/// `corner`, so its corners are round with that radius. The gap is already applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawArc {
    pub a0: f32,
    pub a1: f32,
    pub r0: f32,
    pub r1: f32,
    pub corner: f32,
}

/// Lays out up to `RING_RADII.len()` levels below `nodes[0]`, proportional to
/// value. Children appear in the order given. Spans of `MIN_SPAN` or less are
/// skipped along with their descendants. Output is in depth-first order.
pub fn layout(nodes: &[RingNode]) -> Vec<Segment> {
    if nodes.is_empty() {
        return Vec::new();
    }
    let mut memo = vec![None; nodes.len()];
    let sizes: Vec<f64> = (0..nodes.len())
        .map(|i| subtree_size(nodes, i, &mut memo))
        .collect();
    let mut out = Vec::new();
    place(nodes, &sizes, 0, 0., TAU, 0, &mut out);
    out
}

fn subtree_size(nodes: &[RingNode], i: usize, memo: &mut [Option<f64>]) -> f64 {
    if let Some(size) = memo[i] {
        return size;
    }
    let node = &nodes[i];
    let size = if node.children.is_empty() {
        if node.value.is_finite() {
            node.value.max(0.)
        } else {
            0.
        }
    } else {
        node.children
            .iter()
            .map(|&child| subtree_size(nodes, child, memo))
            .sum()
    };
    memo[i] = Some(size);
    size
}

fn place(
    nodes: &[RingNode],
    sizes: &[f64],
    parent: usize,
    a0: f32,
    a1: f32,
    depth: usize,
    out: &mut Vec<Segment>,
) {
    if depth >= RING_RADII.len() || sizes[parent] <= 0. {
        return;
    }
    let (r0, r1) = RING_RADII[depth];
    let total = sizes[parent];
    let mut a = a0;
    for (sibling_index, &child) in nodes[parent].children.iter().enumerate() {
        let size = sizes[child];
        if size <= 0. {
            continue;
        }
        let b = a + (a1 - a0) * (size / total) as f32;
        if b - a > MIN_SPAN {
            out.push(Segment {
                key: nodes[child].key,
                node: child,
                depth,
                sibling_index,
                arc: Arc {
                    a0: a,
                    a1: b,
                    r0,
                    r1,
                },
            });
            place(nodes, sizes, child, a, b, depth + 1, out);
        }
        a = b;
    }
}

/// Segments indexed for hit testing: per ring, sorted by start angle.
#[derive(Clone, Debug, Default)]
pub struct HitIndex {
    rings: [Vec<Span>; RING_RADII.len()],
}

#[derive(Clone, Copy, Debug)]
struct Span {
    a0: f32,
    a1: f32,
    id: usize,
}

impl HitIndex {
    /// Builds the index from `(id, segment)` pairs. `hit_test` returns the ids.
    /// Overlapping arcs can occur only mid-tween. Then a point resolves to the
    /// arc that starts latest before it.
    pub fn new<'a>(segments: impl IntoIterator<Item = (usize, &'a Segment)>) -> Self {
        let mut rings: [Vec<Span>; RING_RADII.len()] = Default::default();
        for (id, segment) in segments {
            if let Some(ring) = rings.get_mut(segment.depth) {
                ring.push(Span {
                    a0: segment.arc.a0,
                    a1: segment.arc.a1,
                    id,
                });
            }
        }
        for ring in &mut rings {
            ring.sort_by(|a, b| a.a0.total_cmp(&b.a0));
        }
        Self { rings }
    }
}

/// Returns the id of the segment under design-space point `(x, y)`, relative to
/// the centre. Returns `None` for the hub, radial gaps and outside the rings.
/// Cost is O(log n) per ring, a binary search by angle after the radius picks the ring.
pub fn hit_test(index: &HitIndex, x: f32, y: f32) -> Option<usize> {
    let radius = x.hypot(y);
    let depth = RING_RADII
        .iter()
        .position(|&(r0, r1)| radius >= r0 && radius <= r1)?;
    // Angle from 12 o'clock, clockwise, in [0, TAU).
    let mut angle = x.atan2(-y);
    if angle < 0. {
        angle += TAU;
    }
    let spans = &index.rings[depth];
    let next = spans.partition_point(|span| span.a0 <= angle);
    let span = spans.get(next.checked_sub(1)?)?;
    (angle < span.a1).then_some(span.id)
}

/// True when the point lies inside the hub disc.
pub fn in_hub(x: f32, y: f32) -> bool {
    x * x + y * y < HUB_RADIUS * HUB_RADIUS
}

/// Design-space offset from the centre for radius `r` at angle `a`.
pub fn polar(r: f32, a: f32) -> (f32, f32) {
    (r * a.sin(), -r * a.cos())
}

/// Inset centreline for drawing. `None` when the arc is too small to show.
/// Mirrors the prototype's `segShape`: it insets by half the gap plus the corner
/// radius, so the rounded outline ends a half gap from its neighbours.
pub fn drawable(arc: Arc) -> Option<DrawArc> {
    let span = arc.a1 - arc.a0;
    let mid = (arc.r0 + arc.r1) / 2.;
    let thickness = arc.r1 - arc.r0;
    if span <= 0. || mid <= 0. {
        return None;
    }
    let corner = 4.0_f32
        .min((span * mid - SEGMENT_GAP) / 2. - 0.3)
        .min((thickness - SEGMENT_GAP) / 2. - 0.3);
    if corner <= 0.15 {
        return None;
    }
    let inset = corner + SEGMENT_GAP / 2.;
    let pad = (inset / mid).min(span / 2.01);
    Some(DrawArc {
        a0: arc.a0 + pad,
        a1: arc.a1 - pad,
        r0: arc.r0 + inset,
        r1: arc.r1 - inset,
        corner,
    })
}

/// Inset sector for the selection outline, as in the prototype.
pub fn outline(arc: Arc) -> Option<Arc> {
    let span = arc.a1 - arc.a0;
    let mid = (arc.r0 + arc.r1) / 2.;
    if span <= 0. || mid <= 0. {
        return None;
    }
    let inset = SEGMENT_GAP / 2. + 0.2;
    let pad = (inset / mid).min(span / 2.2);
    Some(Arc {
        a0: arc.a0 + pad,
        a1: arc.a1 - pad,
        r0: arc.r0 + inset,
        r1: arc.r1 - inset,
    })
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    fn node(key: u64, value: f64, children: Vec<usize>) -> RingNode {
        RingNode {
            key,
            value,
            children,
        }
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-4
    }

    /// focus(0) -> A(1, children 3 and 4) and B(2, leaf). A has size 3, B has size 1.
    fn sample_tree() -> Vec<RingNode> {
        vec![
            node(0, 0., vec![1, 2]),
            node(1, 0., vec![3, 4]),
            node(2, 1., vec![]),
            node(3, 2., vec![]),
            node(4, 1., vec![]),
        ]
    }

    fn by_key(segments: &[Segment], key: u64) -> &Segment {
        segments.iter().find(|s| s.key == key).expect("segment")
    }

    #[test]
    fn layout_is_proportional_to_value() {
        let segments = layout(&sample_tree());
        let a = by_key(&segments, 1).arc;
        let b = by_key(&segments, 2).arc;
        assert!(near(a.a1 - a.a0, TAU * 0.75), "A spans 3/4 of the ring");
        assert!(near(b.a1 - b.a0, TAU * 0.25), "B spans 1/4 of the ring");
        assert!(
            near(a.a0, 0.) && near(b.a1, TAU),
            "root children fill the ring"
        );
        assert_eq!((a.r0, a.r1), RING_RADII[0]);
    }

    #[test]
    fn children_nest_inside_parent_arc() {
        let segments = layout(&sample_tree());
        let parent = by_key(&segments, 1).arc;
        let big = by_key(&segments, 3);
        let small = by_key(&segments, 4);
        assert_eq!(big.depth, 1);
        assert_eq!((big.arc.r0, big.arc.r1), RING_RADII[1]);
        assert!(near(big.arc.a1 - big.arc.a0, TAU * 0.5));
        assert!(near(small.arc.a1 - small.arc.a0, TAU * 0.25));
        assert!(big.arc.a0 >= parent.a0 - 1e-6 && small.arc.a1 <= parent.a1 + 1e-6);
        assert!(near(small.arc.a0, big.arc.a1), "siblings are adjacent");
        assert_eq!(small.sibling_index, 1);
    }

    #[test]
    fn zero_value_children_are_skipped() {
        let tree = vec![
            node(0, 0., vec![1, 2]),
            node(1, 0., vec![]),
            node(2, 5., vec![]),
        ];
        let segments = layout(&tree);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].key, 2);
        assert!(near(segments[0].arc.a1 - segments[0].arc.a0, TAU));
    }

    #[test]
    fn spans_at_or_below_min_span_are_not_drawn() {
        let count = |n: usize| {
            let mut tree = vec![node(0, 0., (1..=n).collect())];
            tree.extend((1..=n).map(|i| node(i as u64, 1., vec![])));
            layout(&tree).len()
        };
        // TAU / 1500 is about 0.0042, above MIN_SPAN. TAU / 3000 is about 0.0021, below it.
        assert_eq!(count(1500), 1500);
        assert_eq!(count(3000), 0);
    }

    #[test]
    fn depth_is_capped_at_four_rings() {
        let mut tree: Vec<RingNode> = (0..6).map(|i| node(i as u64, 1., vec![i + 1])).collect();
        tree[5].children.clear();
        let segments = layout(&tree);
        assert_eq!(segments.len(), RING_RADII.len());
        assert!(segments.iter().all(|s| s.depth < RING_RADII.len()));
        assert_eq!(segments.last().map(|s| s.depth), Some(3));
    }

    #[test]
    fn empty_input_lays_out_nothing() {
        assert!(layout(&[]).is_empty());
        assert!(layout(&[node(0, 0., vec![])]).is_empty());
    }

    fn sample_index() -> (Vec<Segment>, HitIndex) {
        let segments = layout(&sample_tree());
        let index = HitIndex::new(segments.iter().enumerate());
        (segments, index)
    }

    #[test]
    fn hit_test_finds_each_ring() {
        let (segments, index) = sample_index();
        let id_of = |key: u64| segments.iter().position(|s| s.key == key);
        // Outer-ring point at 45 degrees lies in A (0 to 3/4 turn).
        let (x, y) = polar(130., PI / 4.);
        assert_eq!(hit_test(&index, x, y), id_of(1));
        // 0.9 of a turn on ring 0 lies in B.
        let (x, y) = polar(130., TAU * 0.9);
        assert_eq!(hit_test(&index, x, y), id_of(2));
        // Ring 1 at 45 degrees lies in child 3 (0 to half a turn).
        let (x, y) = polar(190., PI / 4.);
        assert_eq!(hit_test(&index, x, y), id_of(3));
        // Ring 1 at 0.6 of a turn lies in child 4 (half to 3/4).
        let (x, y) = polar(190., TAU * 0.6);
        assert_eq!(hit_test(&index, x, y), id_of(4));
    }

    #[test]
    fn hit_test_rejects_hub_gaps_and_outside() {
        let (_, index) = sample_index();
        assert!(in_hub(0., 0.));
        assert_eq!(hit_test(&index, 0., 0.), None, "hub");
        let (x, y) = polar(95., FRAC_PI_2);
        assert_eq!(hit_test(&index, x, y), None, "between hub and ring 0");
        let (x, y) = polar(162., FRAC_PI_2);
        assert_eq!(hit_test(&index, x, y), None, "radial gap between rings");
        let (x, y) = polar(300., FRAC_PI_2);
        assert_eq!(hit_test(&index, x, y), None, "outside the rings");
        assert!(!in_hub(polar(95., 0.).0, polar(95., 0.).1));
    }

    #[test]
    fn hit_test_angles_wrap_from_twelve_oclock() {
        let (segments, index) = sample_index();
        let b = segments.iter().position(|s| s.key == 2);
        // Just before 12 o'clock, on the anticlockwise side, belongs to B.
        let (x, y) = polar(130., TAU - 0.01);
        assert_eq!(hit_test(&index, x, y), b);
        // Exactly 12 o'clock starts the first root child, A.
        let (x, y) = polar(130., 0.);
        assert_eq!(
            hit_test(&index, x, y),
            segments.iter().position(|s| s.key == 1)
        );
    }

    #[test]
    fn drawable_insets_and_rounds_real_segments() {
        let arc = Arc {
            a0: 0.,
            a1: FRAC_PI_2,
            r0: 100.,
            r1: 160.,
        };
        let d = drawable(arc).expect("large enough to draw");
        assert!(d.corner > 0.15 && d.corner <= 4.);
        assert!(d.a0 > arc.a0 && d.a1 < arc.a1);
        assert!(d.r0 > arc.r0 && d.r1 < arc.r1);
        // The rounded outline ends half a gap inside each neighbour's edge.
        let outer_edge = d.r1 + d.corner;
        assert!(near(outer_edge, arc.r1 - SEGMENT_GAP / 2.));
        let inner_edge = d.r0 - d.corner;
        assert!(near(inner_edge, arc.r0 + SEGMENT_GAP / 2.));
    }

    #[test]
    fn drawable_is_none_for_tiny_arcs() {
        let tiny_span = Arc {
            a0: 0.,
            a1: 0.001,
            r0: 100.,
            r1: 160.,
        };
        assert_eq!(drawable(tiny_span), None);
        let thin = Arc {
            a0: 0.,
            a1: FRAC_PI_2,
            r0: 100.,
            r1: 103.,
        };
        assert_eq!(drawable(thin), None);
        let empty = Arc {
            a0: 1.,
            a1: 1.,
            r0: 100.,
            r1: 160.,
        };
        assert_eq!(drawable(empty), None);
    }

    #[test]
    fn outline_is_inside_the_segment() {
        let arc = Arc {
            a0: 0.,
            a1: 1.,
            r0: 164.,
            r1: 212.,
        };
        let o = outline(arc).expect("outline");
        assert!(o.a0 > arc.a0 && o.a1 < arc.a1);
        assert!(o.r0 > arc.r0 && o.r1 < arc.r1);
    }
}
