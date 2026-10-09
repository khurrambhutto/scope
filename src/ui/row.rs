//! One row in the package list: icon, title/meta line, hover action button.

use gpui_kit::component::{button::Button, Disableable};
use gpui_kit::prelude::*;
use gpui_kit::{div, img, px, rgb, rgba, AnyElement, FontWeight, WeakEntity};

use crate::backend::{self, OpKind};
use crate::domain::listing;
use crate::domain::package::{InstalledPackage, PackageSource};
use crate::theme::{self, display_title, text_dim};

use super::app_view::{act, ScopeApp};
use super::filters::ViewMode;
use super::widgets::{button, ButtonStyle};

pub(super) fn row_element(
    entity: &WeakEntity<ScopeApp>,
    pkg: &InstalledPackage,
    selected: bool,
    view_mode: ViewMode,
    operation_busy: bool,
    index: usize,
) -> AnyElement {
    let title = display_title(pkg);
    let key = pkg.key.clone();
    let version = if pkg.version.is_empty() {
        "—".to_string()
    } else {
        pkg.version.clone()
    };

    let action: Option<AnyElement> = if view_mode == ViewMode::Updates && pkg.has_update {
        Some(action_button(
            entity,
            pkg.key.clone(),
            OpKind::Update,
            "Update",
            selected,
            operation_busy,
        ))
    } else if view_mode == ViewMode::Uninstall {
        Some(if listing::can_uninstall(pkg) {
            action_button(
                entity,
                pkg.key.clone(),
                OpKind::Uninstall,
                "Uninstall",
                selected,
                operation_busy,
            )
        } else {
            unavailable_action_button()
        })
    } else {
        None
    };

    let has_action = action.is_some();
    let row = div()
        .id(("row", index))
        .group("row")
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .when(has_action, |this| this.pr(px(132.)))
        .py(px(10.))
        .my(px(2.))
        .rounded(px(12.))
        .border_1()
        .when(selected && pkg.source == PackageSource::Snap, |this| {
            this.my(px(0.))
                .rounded(px(0.))
                .rounded_t(px(12.))
                .border_0()
        })
        .cursor_pointer()
        .border_color(if selected {
            theme::border_hover()
        } else {
            rgba(0x00000000).into()
        })
        .bg(if selected {
            theme::selected_surface()
        } else {
            rgba(0x00000000).into()
        })
        .hover(|this| this.bg(theme::hover_surface()))
        .on_click(act(entity, move |this, cx| this.toggle_select(&key, cx)))
        .child(package_icon(pkg, 40.))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(15.))
                        .line_height(px(18.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(title),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(px(13.))
                        .line_height(px(16.))
                        .text_color(text_dim())
                        .truncate()
                        .child(version)
                        .child("·")
                        .child(theme::format_size(pkg.size_bytes))
                        .child("·")
                        .child(theme::source_label(pkg.source)),
                ),
        )
        .when_some(action, |this, action| {
            this.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right(px(12.))
                    .flex()
                    .items_center()
                    .opacity(if selected { 1.0 } else { 0.0 })
                    .group_hover("row", |this| this.opacity(1.0))
                    .child(action),
            )
        });

    row.into_any_element()
}

fn action_button(
    entity: &WeakEntity<ScopeApp>,
    key: String,
    kind: OpKind,
    label: &'static str,
    selected: bool,
    operation_busy: bool,
) -> AnyElement {
    let entity = entity.clone();
    let action_id = match kind {
        OpKind::Uninstall => "action-uninstall",
        OpKind::Update => "action-update",
    };
    button(
        action_id,
        label,
        match kind {
            OpKind::Uninstall => ButtonStyle::Danger,
            OpKind::Update => ButtonStyle::Update,
        },
        move |_ev, _window, cx| {
            cx.stop_propagation();
            // Resolve by key at click time; rows never retain stale package data.
            entity
                .update(cx, |this, cx| this.open_op_by_key(kind, &key, cx))
                .ok();
        },
    )
    .disabled(operation_busy)
    .tab_stop(selected && !operation_busy)
    .into_any_element()
}

fn unavailable_action_button() -> AnyElement {
    Button::new("action-unavailable")
        .label("Not supported")
        .rounded(px(18.))
        .h(px(34.))
        .px(px(12.))
        .text_size(px(12.))
        .disabled(true)
        .into_any_element()
}

pub(super) fn package_icon(pkg: &InstalledPackage, size: f32) -> AnyElement {
    if let Some(path) = backend::icon_path(pkg) {
        return img(path)
            .id("icon")
            .size(px(size))
            .rounded(px(10.))
            .into_any_element();
    }
    let title = display_title(pkg);
    div()
        .flex_none()
        .size(px(size))
        .rounded(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::source_color(pkg.source))
        .text_color(rgb(0xffffff))
        .font_weight(FontWeight::SEMIBOLD)
        .text_size(px(if size > 48. { 18. } else { 14. }))
        .child(theme::initials(&title))
        .into_any_element()
}
