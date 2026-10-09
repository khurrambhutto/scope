//! Eased mouse-wheel scrolling for the package list. The list moves by the raw
//! wheel delta, so each notch lands in one jump. This layer takes the wheel
//! events first and eases the list toward a target one frame at a time. It
//! stays out of the way while idle, and scrollbar drags use the same list state.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    canvas, point, px, AnyElement, DispatchPhase, HitboxBehavior, IntoElement, ListState, Pixels,
    ScrollDelta, ScrollWheelEvent, Styled as _, Window,
};

/// Share of the remaining distance covered each frame. At 60 fps this settles in about 0.3 s.
const EASE: f32 = 0.2;
/// Distance under which the animation snaps onto its target.
const SETTLED: Pixels = px(0.5);
/// How long after the last wheel event the list still counts as scrolling.
const IDLE: Duration = Duration::from_millis(200);
/// Pixels per wheel line. Trackpad deltas already arrive in pixels.
const LINE_HEIGHT: Pixels = px(40.);

#[derive(Clone)]
pub(super) struct SmoothScroll {
    list: ListState,
    /// Where the wheel is taking the list; `None` when no animation is running.
    target: Rc<Cell<Option<Pixels>>>,
    /// When the wheel last moved the list.
    last_wheel: Rc<Cell<Option<Instant>>>,
}

impl SmoothScroll {
    pub(super) fn new(list: ListState) -> Self {
        Self {
            list,
            target: Rc::new(Cell::new(None)),
            last_wheel: Rc::new(Cell::new(None)),
        }
    }

    /// An invisible layer that covers the list viewport. Place it before the
    /// list in the children so the step runs before the list lays out.
    pub(super) fn layer(&self) -> AnyElement {
        let prepaint_state = self.clone();
        let paint_state = self.clone();
        canvas(
            move |bounds, window, _cx| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                prepaint_state.step(window);
                hitbox
            },
            move |_bounds, hitbox, window, _cx| {
                window.on_mouse_event(move |event: &ScrollWheelEvent, phase, window, cx| {
                    if phase == DispatchPhase::Capture && hitbox.should_handle_scroll(window) {
                        paint_state.wheel(event.delta, window);
                        cx.stop_propagation();
                    }
                });
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }

    /// True while the wheel is moving the list, so rows can skip hover styling
    /// and stop flickering as they slide under the pointer.
    pub(super) fn is_scrolling(&self) -> bool {
        self.target.get().is_some() || self.last_wheel.get().is_some_and(|at| at.elapsed() < IDLE)
    }

    /// Current top of the list in pixels.
    fn position(&self) -> Pixels {
        -self.list.scroll_px_offset_for_scrollbar().y
    }

    fn max_position(&self) -> Pixels {
        self.list.max_offset_for_scrollbar().y
    }

    fn wheel(&self, delta: ScrollDelta, window: &mut Window) {
        let dy = delta.pixel_delta(LINE_HEIGHT).y;
        let base = self.target.get().unwrap_or_else(|| self.position());
        let target = (base - dy).max(px(0.)).min(self.max_position());
        self.target.set(Some(target));
        self.last_wheel.set(Some(Instant::now()));
        window.refresh();
    }

    fn step(&self, window: &mut Window) {
        if let Some(target) = self.target.get() {
            if self.list.is_scrollbar_dragging() {
                self.target.set(None);
            } else {
                let current = self.position();
                let target = target.max(px(0.)).min(self.max_position());
                let remaining = target - current;
                if remaining.abs() <= SETTLED {
                    self.list.set_offset_from_scrollbar(point(px(0.), -target));
                    self.target.set(None);
                } else {
                    self.list
                        .set_offset_from_scrollbar(point(px(0.), -(current + remaining * EASE)));
                }
            }
        }
        // Keep frames coming through the idle window so rows re-enable hover once it ends.
        if self.is_scrolling() {
            window.request_animation_frame();
        }
    }
}
