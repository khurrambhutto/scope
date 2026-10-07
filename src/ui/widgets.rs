//! Small reusable building blocks shared by the list, filters, and dialog.

use gpui_kit::prelude::*;
use gpui_kit::{
    deferred, div, img, linear_color_stop, linear_gradient, px, rgb, rgba, AnyElement, App,
    ClickEvent, FontWeight, Hsla, ImageSource, IntoElement, Resource, SharedString, Window,
};

use crate::theme::{accent, border, danger, elev, elev2, text, text_dim, text_faint, update_green};

pub(super) fn view_toggle_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(18.))
        .py(px(5.))
        .rounded_full()
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(active, |this| {
            this.bg(linear_gradient(
                135.,
                linear_color_stop(rgb(0xd4504a), 0.),
                linear_color_stop(rgb(0xe0605a), 1.),
            ))
            .text_color(rgb(0xffffff))
        })
        .when(!active, |this| {
            this.text_color(text_dim())
                .hover(|this| this.text_color(text()).bg(rgba(0xffffff08)))
        })
        .on_click(on_click)
        .child(label)
}

/// A pill select trigger plus its deferred dropdown menu. The trigger shows
/// either a text label or an icon.
pub(super) fn select_widget(
    trigger_id: &'static str,
    value_label: Option<&'static str>,
    icon: Option<&'static str>,
    open: bool,
    on_trigger: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    menu: impl Fn() -> Vec<AnyElement> + 'static,
) -> impl IntoElement {
    let trigger = div()
        .id(trigger_id)
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded_full()
        .border_1()
        .border_color(if open { accent() } else { border() })
        .cursor_pointer()
        .hover(|this| this.bg(rgba(0xffffff08)).border_color(accent()))
        .on_click(on_trigger)
        .when_some(value_label, |this, label| {
            this.px(px(18.))
                .py(px(7.))
                .text_size(px(13.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(text_dim())
                .child(label)
        })
        .when_some(icon, |this, icon| {
            this.px(px(12.)).py(px(8.)).child(
                img(ImageSource::Resource(Resource::Embedded(icon.into())))
                    .size(px(16.))
                    .flex_none(),
            )
        })
        .child(
            div()
                .text_color(text_dim())
                .child(if open { "▴" } else { "▾" }),
        );

    div()
        .relative()
        .flex_none()
        .child(trigger)
        .when(open, |this| this.child(deferred(menu_popup(menu()))))
}

pub(super) fn menu_popup(items: Vec<AnyElement>) -> AnyElement {
    div()
        .absolute()
        .top(px(38.))
        // Right-align to the trigger: the source select sits at the window's
        // right edge, so left-alignment pushes the menu out of the window.
        .right_0()
        .min_w(px(160.))
        .p(px(6.))
        .rounded(px(14.))
        .border_1()
        .border_color(border())
        .bg(elev())
        .shadow_lg()
        // Block clicks on the menu from falling through to the package list
        // rows rendered underneath the deferred overlay.
        .occlude()
        .flex()
        .flex_col()
        .children(items)
        .into_any_element()
}

pub(super) fn menu_item(
    id: gpui_kit::ElementId,
    label: &'static str,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px(px(12.))
        .py(px(8.))
        .rounded(px(8.))
        .text_size(px(14.))
        .cursor_pointer()
        .when(selected, |this| this.text_color(accent()))
        .when(!selected, |this| this.text_color(text()))
        .hover(|this| this.bg(elev2()))
        .on_click(on_click)
        .child(label)
}

pub(super) fn tag(label: &'static str, color: Hsla) -> impl IntoElement {
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .border_1()
        .border_color(color)
        .text_color(color)
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}

pub(super) fn empty_state() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .py(px(48.))
        .text_color(text_faint())
        .text_size(px(14.))
        .child("No installed apps match your search.")
}

pub(super) fn loading_state() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .py(px(48.))
        .text_color(text_faint())
        .text_size(px(14.))
        .child("Scanning installed apps across APT, Snap, Flatpak, and AppImage…")
}

pub(super) enum BannerKind {
    Error,
    Warn,
    Ok,
    Muted,
}

pub(super) fn banner(message: &str, kind: BannerKind) -> impl IntoElement {
    let (bg, border_c, fg) = match kind {
        BannerKind::Error => (rgba(0xd4504a1a), rgba(0xd4504a40), rgb(0xf0c4be)),
        BannerKind::Warn => (rgba(0xd9a4411a), rgba(0xd9a44140), rgb(0xecd9a8)),
        BannerKind::Ok => (rgba(0x0b8a4f1f), rgba(0x0b8a4f4d), rgb(0xbfe9c8)),
        BannerKind::Muted => (rgba(0xefe6e40a), rgb(0x2d2325), rgb(0xa69692)),
    };
    div()
        .mx(px(18.))
        .mt(px(10.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(border_c)
        .bg(bg)
        .text_size(px(13.))
        .text_color(fg)
        .child(message.to_string())
}

#[derive(Clone, Copy)]
pub(super) enum ButtonStyle {
    Neutral,
    Danger,
    Update,
}

pub(super) fn button(
    id: &'static str,
    label: &'static str,
    style: ButtonStyle,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let (bg, fg) = match style {
        ButtonStyle::Neutral => (elev2(), text()),
        ButtonStyle::Danger => (danger(), rgb(0xffffff).into()),
        ButtonStyle::Update => (update_green(), rgb(0xffffff).into()),
    };
    let solid = !matches!(style, ButtonStyle::Neutral);
    div()
        .id(id)
        .px(px(16.))
        .py(px(8.))
        .rounded(px(12.))
        .border_1()
        .border_color(if solid { bg } else { border() })
        .bg(bg)
        .text_color(fg)
        .text_size(px(14.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|this| this.opacity(0.9))
        .on_click(on_click)
        .child(label)
}

/// One label/value line in the inline detail panel or a plan summary.
pub(super) fn kv_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    let (label, value): (SharedString, SharedString) = (label.into(), value.into());
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(12.))
        .py(px(8.))
        .border_b_1()
        .border_color(border())
        .child(
            div()
                .flex_none()
                .w(px(120.))
                .text_size(px(13.))
                .text_color(text_faint())
                .child(label),
        )
        .child(div().flex_1().text_size(px(14.)).child(value))
}

pub(super) fn plan_rows(rows: Vec<(&'static str, String)>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .mb(px(14.))
        .children(rows.into_iter().map(|(label, value)| kv_row(label, value)))
}

pub(super) fn dialog_actions(children: Vec<AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .justify_end()
        .gap(px(10.))
        .mt(px(18.))
        .children(children)
}
