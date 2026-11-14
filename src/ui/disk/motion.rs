//! Tweens between two ring layouts, matched by segment key.
//!
//! Kept keys move between their arcs. Fresh keys grow out from their outer
//! radius and fade in. Removed keys collapse onto their inner radius and fade
//! out. An intro starts every arc at zero angle and ignores the previous layout.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use super::geometry::{Arc, Segment};

/// Opacity of a fresh segment ramps to full a little before the motion ends.
const FRESH_FADE: f32 = 1.6;

/// Cubic ease-in-out over `t` in `[0, 1]`, clamped outside that range.
pub fn ease_in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    if t < 0.5 {
        4. * t * t * t
    } else {
        1. - (-2. * t + 2.).powi(3) / 2.
    }
}

/// A timed transition between two laid-out frames.
#[derive(Clone, Debug)]
pub struct Transition {
    entries: Vec<Entry>,
    duration: Duration,
}

#[derive(Clone, Debug)]
struct Entry {
    /// Non-geometric fields. Its `arc` is unused, because `from` and `to` set the geometry.
    segment: Segment,
    from: Arc,
    to: Arc,
    fade: Fade,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Fade {
    Keep,
    In,
    Out,
}

impl Transition {
    /// `previous` is what is displayed now, possibly mid-tween. `intro` ignores
    /// it and grows every arc from zero angle.
    pub fn new(previous: &[Segment], next: &[Segment], intro: bool, duration: Duration) -> Self {
        let before: HashMap<u64, &Segment> = if intro {
            HashMap::new()
        } else {
            previous.iter().map(|s| (s.key, s)).collect()
        };
        let after: HashSet<u64> = next.iter().map(|s| s.key).collect();
        let mut entries = Vec::with_capacity(previous.len() + next.len());

        for segment in next {
            let to = segment.arc;
            let (from, fade) = match before.get(&segment.key) {
                Some(prev) if !intro => (prev.arc, Fade::Keep),
                _ if intro => (
                    Arc {
                        a0: 0.,
                        a1: 0.,
                        r0: to.r0,
                        r1: to.r1,
                    },
                    Fade::In,
                ),
                _ => (
                    Arc {
                        a0: to.a0,
                        a1: to.a1,
                        r0: to.r1,
                        r1: to.r1,
                    },
                    Fade::In,
                ),
            };
            entries.push(Entry {
                segment: segment.clone(),
                from,
                to,
                fade,
            });
        }

        if !intro {
            for prev in previous.iter().filter(|s| !after.contains(&s.key)) {
                let from = prev.arc;
                let to = Arc {
                    r1: from.r0,
                    ..from
                };
                entries.push(Entry {
                    segment: prev.clone(),
                    from,
                    to,
                    fade: Fade::Out,
                });
            }
        }

        Self { entries, duration }
    }

    /// The displayed segments at `elapsed`, each with its opacity. Output
    /// includes collapsing removed segments until the transition is done.
    /// Once done, it is exactly the next layout at full opacity.
    pub fn sample(&self, elapsed: Duration) -> Vec<(Segment, f32)> {
        if self.done(elapsed) {
            return self
                .entries
                .iter()
                .filter(|e| e.fade != Fade::Out)
                .map(|e| {
                    (
                        Segment {
                            arc: e.to,
                            ..e.segment.clone()
                        },
                        1.,
                    )
                })
                .collect();
        }
        let k = ease_in_out_cubic(self.progress(elapsed));
        self.entries
            .iter()
            .map(|e| {
                let opacity = match e.fade {
                    Fade::Keep => 1.,
                    Fade::In => (k * FRESH_FADE).min(1.),
                    Fade::Out => 1. - k,
                };
                (
                    Segment {
                        arc: lerp_arc(e.from, e.to, k),
                        ..e.segment.clone()
                    },
                    opacity,
                )
            })
            .collect()
    }

    /// True once `elapsed` reaches the duration. A zero duration is done at once.
    pub fn done(&self, elapsed: Duration) -> bool {
        elapsed >= self.duration
    }

    fn progress(&self, elapsed: Duration) -> f32 {
        if self.duration.is_zero() {
            1.
        } else {
            (elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0., 1.)
        }
    }
}

fn lerp_arc(from: Arc, to: Arc, k: f32) -> Arc {
    let lerp = |a: f32, b: f32| a + (b - a) * k;
    Arc {
        a0: lerp(from.a0, to.a0),
        a1: lerp(from.a1, to.a1),
        r0: lerp(from.r0, to.r0),
        r1: lerp(from.r1, to.r1),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    const D: Duration = Duration::from_millis(600);

    fn seg(key: u64, a0: f32, a1: f32) -> Segment {
        Segment {
            key,
            node: key as usize,
            depth: 0,
            sibling_index: 0,
            arc: Arc {
                a0,
                a1,
                r0: 100.,
                r1: 160.,
            },
        }
    }

    fn find(frame: &[(Segment, f32)], key: u64) -> Option<(Segment, f32)> {
        frame.iter().find(|(s, _)| s.key == key).cloned()
    }

    /// Key 1 moves, key 2 is removed, key 3 is fresh.
    fn scene() -> (Vec<Segment>, Vec<Segment>) {
        let previous = vec![seg(1, 0., 1.), seg(2, 1., 2.)];
        let next = vec![seg(1, 0., 2.), seg(3, 2., 3.)];
        (previous, next)
    }

    #[test]
    fn easing_has_fixed_endpoints_and_midpoint() {
        assert_eq!(ease_in_out_cubic(0.), 0.);
        assert_eq!(ease_in_out_cubic(1.), 1.);
        assert!((ease_in_out_cubic(0.5) - 0.5).abs() < 1e-6);
        assert_eq!(ease_in_out_cubic(-1.), 0.);
        assert_eq!(ease_in_out_cubic(2.), 1.);
        assert!(ease_in_out_cubic(0.25) < ease_in_out_cubic(0.75));
    }

    #[test]
    fn start_matches_previous_layout() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, D);
        let frame = t.sample(Duration::ZERO);
        let (kept, kept_op) = find(&frame, 1).expect("kept key present");
        assert_eq!(kept.arc, previous[0].arc);
        assert_eq!(kept_op, 1.);
        let (removed, removed_op) = find(&frame, 2).expect("removed key still drawn at start");
        assert_eq!(removed.arc, previous[1].arc);
        assert_eq!(removed_op, 1.);
        let (_, fresh_op) = find(&frame, 3).expect("fresh key present");
        assert_eq!(fresh_op, 0.);
    }

    #[test]
    fn end_matches_next_layout_exactly() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, D);
        assert!(t.done(D));
        let frame = t.sample(D);
        assert!(frame.iter().all(|(_, op)| *op == 1.));
        let arcs: Vec<Segment> = frame.into_iter().map(|(s, _)| s).collect();
        assert_eq!(arcs, next);
    }

    #[test]
    fn fresh_grows_from_outer_radius_and_fades_in() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, D);
        let early = t.sample(D / 10);
        let (fresh, op) = find(&early, 3).expect("fresh key");
        assert!(op > 0. && op < 1., "fades in: {op}");
        assert!(
            fresh.arc.r0 > 100. && fresh.arc.r0 < 160.,
            "grows inward from r1"
        );
        assert_eq!(fresh.arc.r1, 160.);
        // The fade reaches full opacity before the motion ends (k * 1.6 >= 1 from k = 0.625).
        let late = t.sample(D * 3 / 4);
        assert_eq!(find(&late, 3).map(|(_, op)| op), Some(1.));
    }

    #[test]
    fn removed_collapses_and_fades_out() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, D);
        let mut last = 1.;
        for step in 1u32..10 {
            let frame = t.sample(D * step / 10);
            if let Some((seg, op)) = find(&frame, 2) {
                assert!(op <= last, "opacity falls monotonically");
                assert!(seg.arc.r1 <= 160. && seg.arc.r1 >= 100.);
                last = op;
            }
        }
        assert!(
            find(&t.sample(D), 2).is_none(),
            "removed keys are gone at the end"
        );
    }

    #[test]
    fn midway_arcs_sit_between_endpoints() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, D);
        let (mid, _) = find(&t.sample(D / 2), 1).expect("kept key");
        assert!(mid.arc.a1 > 1. && mid.arc.a1 < 2.);
        assert!((mid.arc.a1 - 1.5).abs() < 1e-6, "symmetric ease at halfway");
    }

    #[test]
    fn intro_grows_every_arc_from_zero_angle() {
        let (_, next) = scene();
        let t = Transition::new(&[], &next, true, D);
        let start = t.sample(Duration::ZERO);
        assert_eq!(start.len(), next.len());
        for (seg, op) in &start {
            assert_eq!((seg.arc.a0, seg.arc.a1), (0., 0.));
            assert_eq!(*op, 0.);
        }
        let end: Vec<Segment> = t.sample(D).into_iter().map(|(s, _)| s).collect();
        assert_eq!(end, next);
    }

    #[test]
    fn intro_ignores_previous_layout() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, true, D);
        let frame = t.sample(Duration::ZERO);
        assert!(
            find(&frame, 2).is_none(),
            "removed keys are not drawn during an intro"
        );
    }

    #[test]
    fn zero_duration_jumps_to_next() {
        let (previous, next) = scene();
        let t = Transition::new(&previous, &next, false, Duration::ZERO);
        assert!(t.done(Duration::ZERO));
        let arcs: Vec<Segment> = t
            .sample(Duration::ZERO)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        assert_eq!(arcs, next);
        let intro = Transition::new(&[], &next, true, Duration::ZERO);
        let arcs: Vec<Segment> = intro
            .sample(Duration::ZERO)
            .into_iter()
            .map(|(s, _)| s)
            .collect();
        assert_eq!(arcs, next);
    }
}
