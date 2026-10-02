//! Client-side title bar. GNOME on Wayland ships no server-side decorations, so
//! the app draws its own controls; GPUI drives them through `Window` methods.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{div, img, px, rgb, FontWeight, MouseButton, Window, WindowControlArea};

use crate::theme::{danger, text_dim};

const LOGO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../public/scope-logo.svg");

pub(super) fn title_bar() -> impl IntoElement {
    div()
        .id("titlebar")
        .flex_none()
        .h(px(40.))
        .flex()
        .items_center()
        .window_control_area(WindowControlArea::Drag)
        .on_mouse_down(MouseButton::Left, |event, window, _cx| {
            if event.click_count == 2 {
                window.zoom_window();
            } else {
                window.start_window_move();
            }
        })
        .child(div().flex_1())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(img(PathBuf::from(LOGO)).size(px(28.)).flex_none())
                .child(
                    div()
                        .text_size(px(18.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("Scope"),
                ),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .items_center()
                .justify_end()
                .gap(px(6.))
                .pr(px(8.))
                .child(window_control_button(
                    "win-min",
                    "–",
                    WindowControlArea::Min,
                    |window| window.minimize_window(),
                ))
                .child(window_control_button(
                    "win-max",
                    "▢",
                    WindowControlArea::Max,
                    |window| window.zoom_window(),
                ))
                .child(window_control_button(
                    "win-close",
                    "✕",
                    WindowControlArea::Close,
                    |window| window.remove_window(),
                )),
        )
}

pub(super) fn window_control_button(
    id: &'static str,
    glyph: &'static str,
    area: WindowControlArea,
    action: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    let hover_bg = danger();
    div()
        .id(id)
        .window_control_area(area)
        // Keep the press from bubbling to the title bar's window-move handler.
        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, _cx| action(window))
        .size(px(28.))
        .rounded(px(4.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(14.))
        .text_color(text_dim())
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg).text_color(rgb(0xffffff)))
        .child(glyph)
}
