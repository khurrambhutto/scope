//! Disk page composition: the header strip, the ring stage with breadcrumbs and
//! legend, and the focus panel. Colours come from `theme`; formatting helpers
//! here are shared with `panel`.

use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, relative, AnyElement, App, FontWeight, Hsla, IntoElement, SharedString, WeakEntity,
    Window,
};

use super::geometry::Segment;
use super::rings::{rings, RingClick, RingsProps};
use super::view::DiskState;
use crate::domain::disk_usage::{DiskCategory, DiskTree, NodeId};
use crate::theme;
use crate::ui::app_view::{act, ScopeApp};
use crate::ui::widgets::{banner, loading_message, BannerKind};

/// Draws the Disk page below the title bar and banners. Advances the tween
/// and requests the next frame while one is running.
pub(in crate::ui) fn page(
    state: &mut DiskState,
    entity: &WeakEntity<ScopeApp>,
    window: &mut Window,
) -> AnyElement {
    let (frame, animating) = state.frame(Instant::now());
    if animating {
        window.request_animation_frame();
    }
    let body = match (&state.error, state.tree.clone()) {
        (Some(message), _) => div()
            .p(px(20.))
            .child(banner(message, BannerKind::Error))
            .into_any_element(),
        (None, None) => div()
            .flex()
            .flex_1()
            .child(loading_message("Measuring disk usage…"))
            .into_any_element(),
        (None, Some(tree)) => div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .child(stage(state, &tree, frame, entity))
            .child(super::panel::focus_panel(state, &tree, entity))
            .into_any_element(),
    };
    div()
        .flex_1()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .child(header(state, entity))
        .child(body)
        .into_any_element()
}

/// Bytes used for the percentage and bar denominators: statvfs when it works,
/// otherwise the tree total.
pub(super) fn disk_total(state: &DiskState) -> u64 {
    if state.usage.total > 0 {
        state.usage.total
    } else {
        state
            .tree
            .as_ref()
            .map_or(0, |tree| tree.node(tree.root).bytes)
    }
}

/// Size text, with a trailing `+` while the scan is still partial.
pub(super) fn size_text(bytes: u64, partial: bool) -> String {
    let text = theme::format_size(bytes);
    if partial && bytes > 0 {
        format!("{text}+")
    } else {
        text
    }
}

/// `part / whole` as a fraction clamped to `[0, 1]`. Zero when `whole` is zero.
pub(super) fn fraction(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.
    } else {
        (part as f32 / whole as f32).clamp(0., 1.)
    }
}

/// Whole-number share, with `<1%` for tiny non-zero values.
pub(super) fn percent(part: u64, whole: u64) -> String {
    if whole == 0 {
        return "0%".to_string();
    }
    let value = part as f64 / whole as f64 * 100.;
    if value >= 10. || value == 0. {
        format!("{value:.0}%")
    } else if value < 1. {
        "<1%".to_string()
    } else {
        format!("{value:.1}%")
    }
}

/// Shortens `text` to `max` characters, ending with an ellipsis when cut.
pub(super) fn clip(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn header(state: &DiskState, entity: &WeakEntity<ScopeApp>) -> impl IntoElement {
    let usage = state.usage;
    let used = if usage.total == 0 {
        "—".to_string()
    } else {
        size_text(usage.used, state.scanning)
    };
    div()
        .flex_none()
        .flex()
        .items_center()
        .gap(px(24.))
        .px(px(20.))
        .py(px(16.))
        .border_b_1()
        .border_color(theme::border())
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(8.))
                .whitespace_nowrap()
                .child(
                    div()
                        .text_size(px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(used),
                )
                .child(
                    div()
                        .text_color(theme::text_faint())
                        .child(format!("of {} used", theme::format_size(usage.total))),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(category_bar(&state.totals, disk_total(state))),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::text_faint())
                        .whitespace_nowrap()
                        .child(format!(
                            "{} free",
                            if usage.total == 0 {
                                "—".to_string()
                            } else {
                                size_text(usage.free, state.scanning)
                            }
                        )),
                )
                .child(
                    Button::new("disk-rescan")
                        .icon(IconName::RefreshCw)
                        .accessibility_label("Rescan disk usage")
                        .rounded(px(18.))
                        .size(px(34.))
                        .loading(state.scanning)
                        .on_click(act(entity, |this, cx| this.restart_disk_scan(cx))),
                ),
        )
}

fn category_bar(totals: &[(DiskCategory, u64)], total: u64) -> impl IntoElement {
    div()
        .w_full()
        .h(px(8.))
        .rounded(px(4.))
        .bg(theme::hover_surface())
        .overflow_hidden()
        .flex()
        .gap(px(2.))
        .children(totals.iter().map(|&(category, bytes)| {
            div()
                .h_full()
                .flex_none()
                .rounded(px(4.))
                .w(relative(fraction(bytes, total)))
                .bg(theme::disk_category_color(category))
        }))
}

fn stage(
    state: &DiskState,
    tree: &Arc<DiskTree>,
    frame: Vec<(Segment, f32)>,
    entity: &WeakEntity<ScopeApp>,
) -> impl IntoElement {
    // Hit results are indices into this frame, so keep the keys it was built from.
    let keys: Rc<[u64]> = frame
        .iter()
        .map(|(segment, _)| segment.key)
        .collect::<Vec<_>>()
        .into();
    let hovered = state.hovered;
    let (up_label, hub_lines) = hub(state, tree);
    let props = RingsProps {
        frame,
        colors: segment_colors(tree.clone()),
        hovered: hovered.map(u64::from),
        lit: lit_keys(tree.clone(), hovered),
        selected: state.selected.map(u64::from),
        hub_lines,
        hub_up_label: up_label,
    };

    let hover_entity = entity.clone();
    let hover_keys = keys.clone();
    let on_hover = move |index: Option<usize>, _: &mut Window, cx: &mut App| {
        let id = index
            .and_then(|i| hover_keys.get(i))
            .map(|&key| key as NodeId);
        hover_entity
            .update(cx, |this, cx| this.disk_hover(id, cx))
            .ok();
    };
    let click_entity = entity.clone();
    let on_click = move |click: RingClick, _: &mut Window, cx: &mut App| {
        click_entity
            .update(cx, |this, cx| match click {
                RingClick::Hub => this.disk_zoom_up(cx),
                RingClick::Segment(index) => {
                    if let Some(&key) = keys.get(index) {
                        this.disk_activate(key as NodeId, cx);
                    }
                }
            })
            .ok();
    };

    div()
        .flex_1()
        .min_w(px(0.))
        .min_h(px(0.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .px(px(20.))
        .pt(px(14.))
        .pb(px(18.))
        .child(breadcrumbs(tree, state.focus, entity))
        .child(div().flex_1().min_h(px(0.)).relative().child(rings(
            "disk-rings",
            props,
            on_hover,
            on_click,
        )))
        .child(legend(state))
}

/// Hub text. Hovering a node shows its size and share of the focus. Otherwise
/// the hub shows the focus, with a link up one level when zoomed in.
fn hub(state: &DiskState, tree: &DiskTree) -> (Option<SharedString>, [SharedString; 3]) {
    let focus = tree.node(state.focus);
    let partial = state.scanning;
    let hovered = state
        .hovered
        .filter(|&id| id != state.focus)
        .and_then(|id| tree.nodes.get(id as usize).map(|node| (id, node)));
    match hovered {
        Some((_, node)) => (
            None,
            [
                size_text(node.bytes, partial).into(),
                clip(&node.name, 20).into(),
                format!(
                    "{} of {}",
                    percent(node.bytes, focus.bytes),
                    clip(&focus.name, 20)
                )
                .into(),
            ],
        ),
        None => {
            let up = focus.parent.map(|parent| {
                SharedString::from(format!("↑ {}", clip(&tree.node(parent).name, 20)))
            });
            let total = disk_total(state);
            let sub = if state.focus == tree.root {
                format!("of {}", theme::format_size(total))
            } else {
                format!("{} of disk", percent(focus.bytes, total))
            };
            (
                up,
                [
                    size_text(focus.bytes, partial).into(),
                    clip(&focus.name, 20).into(),
                    sub.into(),
                ],
            )
        }
    }
}

fn segment_colors(tree: Arc<DiskTree>) -> Rc<dyn Fn(&Segment) -> Hsla> {
    Rc::new(move |segment: &Segment| {
        let category = tree
            .nodes
            .get(segment.key as usize)
            .map_or(DiskCategory::Other, |node| node.category);
        theme::disk_segment_color(category, segment.depth, segment.sibling_index)
    })
}

/// Keys that stay lit while `hovered` is set: the node, its ancestors and its descendants.
fn lit_keys(tree: Arc<DiskTree>, hovered: Option<NodeId>) -> Rc<dyn Fn(u64) -> bool> {
    Rc::new(move |key: u64| {
        let Some(hot) = hovered else {
            return false;
        };
        let id = key as NodeId;
        id == hot || tree.ancestors(id).contains(&hot) || tree.ancestors(hot).contains(&id)
    })
}

/// Ancestors of the focus, root first. Each one but the last zooms when clicked.
fn breadcrumbs(tree: &DiskTree, focus: NodeId, entity: &WeakEntity<ScopeApp>) -> impl IntoElement {
    let chain = tree.ancestors(focus);
    let last = chain.len().saturating_sub(1);
    let mut row = div()
        .flex()
        .flex_wrap()
        .items_center()
        .min_h(px(30.))
        .text_size(px(13.))
        .text_color(theme::text_faint());
    for (index, &id) in chain.iter().enumerate() {
        let name = tree.node(id).name.clone();
        if index == last {
            row = row.child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme::text())
                    .child(name),
            );
            continue;
        }
        let entity = entity.clone();
        row = row
            .child(
                div()
                    .id(("disk-crumb", index))
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme::hover_surface()).text_color(theme::text()))
                    .on_click(move |_, _, cx| {
                        entity.update(cx, |this, cx| this.disk_zoom_to(id, cx)).ok();
                    })
                    .child(name),
            )
            .child(div().px(px(2.)).text_color(theme::border()).child("/"));
    }
    row
}

/// Category chips, centred, with scan status on its own line below: entries visited
/// while scanning, then folders that could not be read.
fn legend(state: &DiskState) -> impl IntoElement {
    let mut status = Vec::new();
    if state.scanning {
        status.push(format!("{} entries scanned", state.entries));
    }
    if state.unreadable > 0 {
        let noun = if state.unreadable == 1 { "folder" } else { "folders" };
        status.push(format!("{} {noun} unreadable", state.unreadable));
    }
    let chips = state.totals.iter().map(|&(category, bytes)| {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .py(px(5.))
            .rounded_full()
            .text_size(px(12.5))
            .text_color(theme::text_faint())
            .child(
                div()
                    .flex_none()
                    .size(px(9.))
                    .rounded(px(3.))
                    .bg(theme::disk_category_color(category)),
            )
            .child(category.label())
            .child(
                div()
                    .text_color(theme::text_dim())
                    .child(size_text(bytes, state.scanning)),
            )
    });
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(6.))
        .child(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .justify_center()
                .gap(px(4.))
                .children(chips),
        )
        .when(!status.is_empty(), |this| {
            this.child(
                div()
                    .text_size(px(12.))
                    .text_color(theme::text_faint())
                    .child(status.join(" · ")),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_rounds_large_shares_and_marks_tiny_ones() {
        assert_eq!(percent(0, 100), "0%");
        assert_eq!(percent(5, 1000), "<1%");
        assert_eq!(percent(25, 100), "25%");
        assert_eq!(percent(15, 100), "15%");
        assert_eq!(percent(1, 0), "0%");
    }

    #[test]
    fn clip_keeps_short_names_and_ellipsises_long_ones() {
        assert_eq!(clip("Home", 20), "Home");
        assert_eq!(clip("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn fraction_is_clamped_and_zero_without_a_whole() {
        assert_eq!(fraction(1, 0), 0.);
        assert_eq!(fraction(3, 2), 1.);
        assert_eq!(fraction(1, 4), 0.25);
    }

    #[test]
    fn partial_sizes_carry_a_plus_only_when_nonzero() {
        assert_eq!(size_text(0, true), "—");
        assert_eq!(size_text(1024, true), "1.0 KB+");
        assert_eq!(size_text(1024, false), "1.0 KB");
    }
}
