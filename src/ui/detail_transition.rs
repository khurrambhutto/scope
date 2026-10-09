//! Measured inline-panel heights, eased without resetting the virtual list.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::prelude::*;
use gpui_kit::{canvas, div, px, AnyElement, ListState, Pixels};

use crate::domain::package::InstalledPackage;

#[derive(Clone, Copy)]
struct Panel {
    open: bool,
    measured: Pixels,
    height: Pixels,
    last_frame: Instant,
}

#[derive(Clone, Default)]
pub(super) struct DetailTransition {
    panels: Rc<RefCell<HashMap<String, Rc<Cell<Panel>>>>>,
}

impl DetailTransition {
    pub(super) fn select(&self, previous: Option<&str>, next: Option<&str>) {
        let mut panels = self.panels.borrow_mut();
        if let Some(panel) = previous.and_then(|key| panels.get(key)) {
            let mut state = panel.get();
            state.open = false;
            state.last_frame = Instant::now();
            panel.set(state);
        }
        if let Some(key) = next {
            let panel = panels.entry(key.to_owned()).or_insert_with(|| {
                Rc::new(Cell::new(Panel {
                    open: true,
                    measured: px(0.),
                    height: px(0.),
                    last_frame: Instant::now(),
                }))
            });
            let mut state = panel.get();
            state.open = true;
            state.last_frame = Instant::now();
            panel.set(state);
        }
    }

    pub(super) fn clear(&self) {
        self.panels.borrow_mut().clear();
    }

    pub(super) fn visible(&self, key: &str) -> bool {
        self.panels
            .borrow()
            .get(key)
            .is_some_and(|panel| panel.get().open || panel.get().height > px(0.))
    }

    pub(super) fn wrap(&self, key: &str, content: AnyElement) -> AnyElement {
        let Some(panel) = self.panels.borrow().get(key).cloned() else {
            return content;
        };
        div()
            .h(panel.get().height)
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .w_full()
                    .child(content)
                    .on_children_prepainted(move |bounds, window, _cx| {
                        if let Some(bounds) = bounds.first() {
                            let mut state = panel.get();
                            if state.measured != bounds.size.height {
                                state.measured = bounds.size.height;
                                panel.set(state);
                                window.request_animation_frame();
                            }
                        }
                    }),
            )
            .into_any_element()
    }

    /// Runs before the list prepaints, invalidating only rows whose height moved.
    pub(super) fn layer(&self, list: ListState, entries: &[InstalledPackage]) -> AnyElement {
        let panels = self.panels.clone();
        let indices: HashMap<_, _> = panels
            .borrow()
            .keys()
            .filter_map(|key| {
                entries
                    .iter()
                    .position(|entry| &entry.key == key)
                    .map(|index| (key.clone(), index))
            })
            .collect();
        canvas(
            move |_bounds, window, cx| {
                let now = Instant::now();
                panels.borrow_mut().retain(|key, panel| {
                    let Some(&index) = indices.get(key) else {
                        return false;
                    };
                    let mut state = panel.get();
                    let target = if state.open { state.measured } else { px(0.) };
                    let remaining = target - state.height;
                    if remaining != px(0.) {
                        let elapsed = now.duration_since(state.last_frame).as_secs_f32();
                        state.height = if cx.reduce_motion() || remaining.abs() < px(0.5) {
                            target
                        } else {
                            state.height + remaining * (1. - (-elapsed / 0.055).exp())
                        };
                        list.splice(index..index + 1, 1);
                        window.request_animation_frame();
                    }
                    state.last_frame = now;
                    panel.set(state);
                    state.open || state.height > px(0.)
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}
