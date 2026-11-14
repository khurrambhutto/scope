//! Small reusable building blocks shared by the list, filters, and dialog.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    spinner::Spinner,
    Disableable, Icon, Selectable, Sizable,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    deferred, div, px, relative, AnyElement, App, ClickEvent, FontWeight, IntoElement, Window,
};

use crate::theme::{self, border, elev2, text, text_dim, text_faint};

const ICON_TRASH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/trash.svg");

pub(super) fn view_toggle_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    Button::new(id)
        .accessibility_label(label)
        .ghost()
        .rounded(px(18.))
        .h(px(34.))
        .px(px(16.))
        .child(div().text_size(px(12.)).line_height(px(16.)).child(label))
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

/// `on_clear` is set only while a query is present.
pub(super) fn empty_state(
    on_clear: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(12.))
        .py(px(40.))
        .text_color(text_faint())
        .text_size(px(14.))
        .line_height(px(20.))
        .child("No installed apps match your search.")
        .when_some(on_clear, |this, on_clear| {
            this.child(
                button(
                    "empty-clear-search",
                    "Clear search",
                    ButtonStyle::Neutral,
                    on_clear,
                )
                .ghost(),
            )
        })
}

pub(super) fn loading_state(cx: &App) -> impl IntoElement {
    let animate = !cx.reduce_motion();
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(10.))
                .py(px(20.))
                .text_color(text_faint())
                .text_size(px(14.))
                .child(Spinner::new().small().color(text_faint()))
                .child(
                    "Scanning installed apps across APT, Snap, Flatpak, AppImage, Steam, and JetBrains Toolbox…",
                ),
        )
        .children(
            [(180., 240.), (140., 200.), (210., 260.), (160., 220.)]
                .into_iter()
                .enumerate()
                .map(move |(index, (name, meta))| skeleton_row(index, name, meta, animate)),
        )
}

/// A spinner with one line of copy, for a screen that waits on a single job.
pub(super) fn loading_message(message: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .text_color(text_faint())
        .text_size(px(14.))
        .line_height(px(20.))
        .child(Spinner::new().small().color(text_faint()))
        .child(message)
}

/// A list-row placeholder at the real row's icon, name, and metadata sizes.
fn skeleton_row(index: usize, name_width: f32, meta_width: f32, animate: bool) -> AnyElement {
    use gpui_kit::{Animation, AnimationExt};

    let bar = |width: f32, height: f32| {
        div()
            .w(px(width))
            .h(px(height))
            .rounded(px(4.))
            .bg(theme::skeleton())
    };
    let row = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(12.))
        .py(px(10.))
        .my(px(2.))
        .child(
            div()
                .flex_none()
                .size(px(40.))
                .rounded(px(10.))
                .bg(theme::skeleton()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(7.))
                .child(bar(name_width, 12.))
                .child(bar(meta_width, 10.)),
        );
    if !animate {
        return row.into_any_element();
    }
    row.with_animation(
        ("skeleton-row", index),
        Animation::new(std::time::Duration::from_millis(1400))
            .repeat()
            .with_easing(gpui_kit::bounce(gpui_kit::ease_in_out)),
        |this, delta| this.opacity(1.0 - delta * 0.5),
    )
    .into_any_element()
}

pub(super) enum BannerKind {
    Error,
    Warn,
}

pub(super) fn banner(message: &str, kind: BannerKind) -> gpui_kit::Div {
    let (bg, fg, icon) = match kind {
        BannerKind::Error => (
            theme::selected_surface(),
            theme::accent_text(),
            IconName::CircleX,
        ),
        BannerKind::Warn => (elev2(), text(), IconName::TriangleAlert),
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .border_color(theme::border_hover())
        .bg(bg)
        .text_size(px(13.))
        .line_height(px(18.))
        .text_color(fg)
        .child(
            Icon::new(icon)
                .size(px(16.))
                .flex_none()
                .mt(px(1.))
                .text_color(fg),
        )
        .child(div().flex_1().min_w(px(0.)).child(message.to_string()))
}

pub(super) fn dismiss_button(
    id: gpui_kit::ElementId,
    label: &'static str,
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .icon(IconName::Close)
        .accessibility_label(label)
        .size(px(28.))
        .flex_none()
        .rounded_full()
        .text_color(text_dim())
        .on_click(on_dismiss)
}

pub(super) fn dismissible_banner(
    id: gpui_kit::ElementId,
    message: &str,
    kind: BannerKind,
    on_dismiss: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    banner(message, kind)
        .pr(px(8.))
        .child(dismiss_button(id, "Dismiss notification", on_dismiss))
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
        // Primary and danger share one hue, so removal also carries weight and an icon.
        ButtonStyle::Danger => button
            .danger()
            .font_weight(FontWeight::SEMIBOLD)
            .icon(Icon::empty().path(ICON_TRASH)),
        ButtonStyle::Update => button.primary(),
    }
}

/// Disabled controls share one look: transparent, outlined, with a muted label.
/// Instance styles override the kit's disabled style, so they apply only here.
pub(super) trait DisabledLook {
    fn disabled_look(self, disabled: bool) -> Self;
}

impl DisabledLook for Button {
    fn disabled_look(self, disabled: bool) -> Self {
        self.disabled(disabled).when(disabled, |this| {
            this.bg(gpui_kit::transparent_black())
                .border_1()
                .border_color(border())
                .text_color(theme::text_disabled())
        })
    }
}

pub(super) fn plan_rows(rows: Vec<(&'static str, String)>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .children(rows.into_iter().map(|(label, value)| {
            div()
                .flex()
                .items_start()
                .gap(px(theme::DETAIL_ROW_GAP))
                .py(px(theme::DETAIL_ROW_PADDING))
                .text_size(px(13.))
                .line_height(px(18.))
                .child(
                    div()
                        .flex_none()
                        .w(px(theme::DETAIL_LABEL_WIDTH))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(div().flex_1().min_w(px(0.)).child(value))
        }))
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
