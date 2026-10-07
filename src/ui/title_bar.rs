//! Client-side title bar. GNOME on Wayland ships no server-side decorations, so
//! the app draws its own controls; GPUI drives them through `Window` methods.

use gpui_kit::prelude::*;
use gpui_kit::{
    div, img, px, rgb, svg, FontWeight, ImageSource, MouseButton, Pixels, Resource, Window,
    WindowControlArea,
};

use crate::theme::{danger, elev2, text, text_dim};

// Title-bar-sized copy of the brand mark: GPUI rasters SVGs at 2x their
// intrinsic size, so a 512px source would be crushed into the 28px slot.
const LOGO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/scope-logo.svg");
const ICON_MIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/win-min.svg");
const ICON_MAX: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/win-max.svg");
const ICON_CLOSE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/win-close.svg");

pub(super) fn title_bar(navigation: impl IntoElement, corner_radius: Pixels) -> impl IntoElement {
    div()
        .id("titlebar")
        .relative()
        .flex_none()
        .h(px(64.))
        .px(px(16.))
        .gap(px(16.))
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
        .child(
            div()
                .flex_1()
                .min_w(px(120.))
                .flex()
                .items_center()
                .gap(px(8.))
                // `Resource::Embedded` (not a bare `&str`): GPUI treats a plain
                // string path as a URI and tries an HTTP fetch, which fails
                // silently. Embedded goes through our `AssetSource`, which
                // serves the bytes compiled into the binary on any machine.
                // `svg()` is wrong here — it tints everything monochrome and
                // would flatten the multicolor mark into a white disc.
                .child(
                    img(ImageSource::Resource(Resource::Embedded(LOGO.into())))
                        .size(px(28.))
                        .flex_none(),
                )
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(text())
                        .child("Scope"),
                ),
        )
        .child(
            div()
                .id("titlebar-navigation")
                .flex_none()
                // Tabs are controls, so presses must not start a window drag.
                .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
                    cx.stop_propagation();
                })
                .child(navigation),
        )
        .child(
            // Balance the brand column so navigation stays centered while
            // window controls are anchored independently to the corner.
            div().flex_1().min_w(px(120.)),
        )
        .child(
            div()
                .absolute()
                .top_0()
                .right_0()
                .flex()
                .items_center()
                .child(window_control_button(
                    "win-min",
                    ICON_MIN,
                    WindowControlArea::Min,
                    elev2(),
                    px(0.),
                    |window| window.minimize_window(),
                ))
                .child(window_control_button(
                    "win-max",
                    ICON_MAX,
                    WindowControlArea::Max,
                    elev2(),
                    px(0.),
                    |window| window.zoom_window(),
                ))
                .child(window_control_button(
                    "win-close",
                    ICON_CLOSE,
                    WindowControlArea::Close,
                    danger(),
                    corner_radius,
                    |window| window.remove_window(),
                )),
        )
}

pub(super) fn window_control_button(
    id: &'static str,
    icon: &'static str,
    area: WindowControlArea,
    hover_bg: gpui_kit::Hsla,
    corner_radius: Pixels,
    action: impl Fn(&mut Window) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .group(id)
        .window_control_area(area)
        // Keep the press from bubbling to the title bar's window-move handler.
        .on_mouse_down(MouseButton::Left, |_event, _window, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, _cx| action(window))
        .w(px(40.))
        .h(px(36.))
        .rounded_tr(corner_radius)
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .hover(move |this| this.bg(hover_bg))
        .child(
            // `Svg` reads the colour from its own computed style, not the
            // cascaded text style, so it is set here and brightened through the
            // button's hover group.
            svg()
                .path(icon)
                .size(px(13.))
                .flex_none()
                .text_color(text_dim())
                .group_hover(id, |this| this.text_color(rgb(0xffffff))),
        )
}
