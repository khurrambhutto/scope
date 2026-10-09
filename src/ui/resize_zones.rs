//! Invisible resize bands for the client-decorated window. GPUI resizes a
//! client-decorated window only when the app asks, so these thin zones along the
//! edges and corners call `start_window_resize`, the way the title bar calls
//! `start_window_move`. The top-right band is left open for the window controls.

use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, CursorStyle, Div, MouseButton, Pixels, ResizeEdge, Stateful};

const EDGE: Pixels = px(5.);
const CORNER: Pixels = px(12.);
/// Width of the min/max/close buttons in the title bar, which the top band must not cover.
const WINDOW_CONTROLS: Pixels = px(120.);
/// Height of the title bar's window-control buttons.
const WINDOW_CONTROLS_HEIGHT: Pixels = px(36.);

fn zone(id: &'static str, edge: ResizeEdge, cursor: CursorStyle) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .cursor(cursor)
        .occlude()
        .on_mouse_down(MouseButton::Left, move |_event, window, _cx| {
            window.start_window_resize(edge);
        })
}

/// The resize zones to lay over the window. Callers skip them when the window is
/// maximized, fullscreen, or tiled, because those states cannot be resized from an edge.
pub(super) fn resize_zones() -> Vec<AnyElement> {
    vec![
        zone("resize-top", ResizeEdge::Top, CursorStyle::ResizeUpDown)
            .top_0()
            .left(CORNER)
            .right(WINDOW_CONTROLS)
            .h(EDGE)
            .into_any_element(),
        zone(
            "resize-bottom",
            ResizeEdge::Bottom,
            CursorStyle::ResizeUpDown,
        )
        .bottom_0()
        .left(CORNER)
        .right(CORNER)
        .h(EDGE)
        .into_any_element(),
        zone(
            "resize-left",
            ResizeEdge::Left,
            CursorStyle::ResizeLeftRight,
        )
        .left_0()
        .top(CORNER)
        .bottom(CORNER)
        .w(EDGE)
        .into_any_element(),
        zone(
            "resize-right",
            ResizeEdge::Right,
            CursorStyle::ResizeLeftRight,
        )
        .right_0()
        .top(WINDOW_CONTROLS_HEIGHT)
        .bottom(CORNER)
        .w(EDGE)
        .into_any_element(),
        zone(
            "resize-top-left",
            ResizeEdge::TopLeft,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .top_0()
        .left_0()
        .w(CORNER)
        .h(CORNER)
        .into_any_element(),
        zone(
            "resize-bottom-right",
            ResizeEdge::BottomRight,
            CursorStyle::ResizeUpLeftDownRight,
        )
        .bottom_0()
        .right_0()
        .w(CORNER)
        .h(CORNER)
        .into_any_element(),
        zone(
            "resize-bottom-left",
            ResizeEdge::BottomLeft,
            CursorStyle::ResizeUpRightDownLeft,
        )
        .bottom_0()
        .left_0()
        .w(CORNER)
        .h(CORNER)
        .into_any_element(),
    ]
}
