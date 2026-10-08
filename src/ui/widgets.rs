//! Small reusable building blocks shared by the list, filters, and dialog.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    Icon, Selectable,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    deferred, div, px, relative, AnyElement, App, ClickEvent, FontWeight, Hsla, IntoElement,
    SharedString, Window,
};

use crate::theme::{self, border, elev2, text, text_dim, text_faint};

pub(super) fn view_toggle_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(id)
        .label(label)
        .ghost()
        .rounded(px(18.))
        .h(px(34.))
        .px(px(16.))
        .text_size(px(13.))
        .selected(active)
        .toggled(active)
        .on_click(on_click)
}

/// A pill select trigger plus its deferred dropdown menu.
pub(super) fn select_widget(
    trigger_id: &'static str,
    label: &'static str,
    open: bool,
    on_trigger: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    menu: impl Fn() -> Vec<AnyElement> + 'static,
) -> impl IntoElement {
    let trigger = Button::new(trigger_id)
        .label(label)
        .dropdown_caret(true)
        .rounded(px(18.))
        .h(px(34.))
        .px(px(16.))
        .text_size(px(13.))
        .selected(open)
        .on_click(on_trigger);

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
        .bg(elev2())
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
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .text_size(px(13.))
        .cursor_pointer()
        .when(selected, |this| {
            this.bg(theme::selected_surface())
                .text_color(theme::accent_text())
        })
        .when(!selected, |this| this.text_color(text_dim()))
        .hover(|this| this.bg(theme::hover_surface()).text_color(text()))
        .on_click(on_click)
        .child(label)
        .child(
            Icon::new(IconName::Check)
                .size(px(14.))
                .opacity(if selected { 1. } else { 0. }),
        )
}

pub(super) fn tag(label: &'static str, color: Hsla) -> impl IntoElement {
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .border_1()
        .border_color(border())
        .bg(elev2())
        .text_color(text_dim())
        .flex()
        .items_center()
        .gap(px(6.))
        .child(div().size(px(5.)).rounded_full().bg(color))
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
}

pub(super) fn banner(message: &str, kind: BannerKind) -> impl IntoElement {
    let (bg, border_c, fg) = match kind {
        BannerKind::Error => (
            theme::selected_surface(),
            theme::border_hover(),
            theme::accent_text(),
        ),
        BannerKind::Warn => (elev2(), theme::border_hover(), text()),
    };
    div()
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
) -> Button {
    let button = Button::new(id)
        .label(label)
        .rounded(px(18.))
        .h(px(34.))
        .px(px(16.))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .on_click(on_click);
    match style {
        ButtonStyle::Neutral => button,
        ButtonStyle::Danger => button.danger(),
        ButtonStyle::Update => button.primary(),
    }
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

/// Package managers expose stages, but no reliable completion percentage.
pub(super) fn progress_bar(id: &'static str) -> impl IntoElement {
    use gpui_kit::{Animation, AnimationExt};

    div()
        .w_full()
        .h(px(4.))
        .relative()
        .overflow_hidden()
        .rounded_full()
        .bg(theme::border())
        .child(
            div()
                .absolute()
                .top_0()
                .h_full()
                .w(relative(0.3))
                .rounded_full()
                .bg(theme::accent())
                .with_animation(
                    id,
                    Animation::new(std::time::Duration::from_millis(1500)).repeat(),
                    |this, delta| this.left(relative(-0.3 + delta * 1.3)),
                ),
        )
}
