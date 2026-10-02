//! One row in the package list: icon, title/meta line, hover action button.

use gpui::prelude::*;
use gpui::{div, img, px, rgba, rgb, AnyElement, FontWeight, Hsla, SharedString, WeakEntity};

use crate::backend::{self, OpKind};
use crate::package::InstalledPackage;
use crate::theme::{self, danger, display_title, text_dim, update_green};

use super::app_view::ScopeApp;
use super::filters::ViewMode;

pub(super) fn row_element(
    entity: &WeakEntity<ScopeApp>,
    pkg: &InstalledPackage,
    selected: bool,
    view_mode: ViewMode,
) -> AnyElement {
    let title = display_title(pkg);
    let key = pkg.key.clone();
    let version = if pkg.version.is_empty() {
        "—".to_string()
    } else {
        pkg.version.clone()
    };

    let action: Option<AnyElement> = if view_mode == ViewMode::Updates && pkg.has_update {
        Some(action_button(entity, pkg, OpKind::Update, "Update", update_green()))
    } else if view_mode == ViewMode::Uninstall {
        Some(action_button(entity, pkg, OpKind::Uninstall, "Uninstall", danger()))
    } else {
        None
    };

    let row = div()
        .id(SharedString::from(format!("row-{key}")))
        .group("row")
        .relative()
        .flex()
        .items_center()
        .gap(px(12.))
        .w_full()
        .px(px(12.))
        .py(px(10.))
        .my(px(2.))
        .rounded(px(12.))
        .border_1()
        .cursor_pointer()
        .border_color(if selected {
            rgba(0xd4504a26)
        } else {
            rgba(0x00000000)
        })
        .bg(if selected {
            rgba(0xd4504a14)
        } else {
            rgba(0x00000000)
        })
        .hover(|this| this.bg(rgba(0xffffff0a)))
        .on_click({
            let entity = entity.clone();
            let key = key.clone();
            move |_ev, _window, cx| {
                entity
                    .update(cx, |this, cx| this.toggle_select(key.clone(), cx))
                    .ok();
            }
        })
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
                    .right_0()
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
    pkg: &InstalledPackage,
    kind: OpKind,
    label: &'static str,
    color: Hsla,
) -> AnyElement {
    let entity = entity.clone();
    let pkg = pkg.clone();
    let action_id = match kind {
        OpKind::Uninstall => format!("action-uninstall-{}", pkg.key),
        OpKind::Update => format!("action-update-{}", pkg.key),
    };
    div()
        .id(SharedString::from(action_id))
        .h_full()
        .px(px(18.))
        .flex()
        .items_center()
        .rounded_r(px(11.))
        .bg(color)
        .text_color(rgb(0xffffff))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|this| this.opacity(0.9))
        .on_click(move |_ev, _window, cx| {
            cx.stop_propagation();
            entity
                .update(cx, |this, cx| this.open_op(kind, pkg.clone(), cx))
                .ok();
        })
        .child(label)
        .into_any_element()
}

pub(super) fn package_icon(pkg: &InstalledPackage, size: f32) -> AnyElement {
    if let Some(path) = backend::icon_path(pkg) {
        return img(path)
            .id(SharedString::from(format!("icon-{}", pkg.key)))
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
