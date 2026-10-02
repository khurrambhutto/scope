//! Inline detail panel shown beneath the selected row.

use gpui::prelude::*;
use gpui::{div, px, rgba, AnyElement, FontWeight};

use crate::package::InstalledPackage;
use crate::theme::{self, display_title, text_dim};

use super::row::package_icon;
use super::widgets::{kv_row, tag};

pub(super) fn detail_element(pkg: &InstalledPackage) -> AnyElement {
    let title = display_title(pkg);

    let mut rows: Vec<(String, String)> = vec![
        ("Package id".to_string(), pkg.package_id.clone()),
        (
            "Version".to_string(),
            if pkg.version.is_empty() {
                "—".to_string()
            } else {
                pkg.version.clone()
            },
        ),
        ("Installed size".to_string(), theme::format_size(pkg.size_bytes)),
        (
            "Categories".to_string(),
            pkg.categories.clone().unwrap_or_else(|| "—".to_string()),
        ),
        (
            "Update available".to_string(),
            if pkg.has_update {
                "Yes".to_string()
            } else {
                "—".to_string()
            },
        ),
    ];
    rows.retain(|(_, value)| !value.is_empty() && value != "—");

    div()
        .mx(px(8.))
        .mb(px(8.))
        .ml(px(12.))
        .p(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(rgba(0xffffff0f))
        .bg(rgba(0xffffff08))
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(package_icon(pkg, 56.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(6.))
                                .child(tag(theme::source_label(pkg.source), theme::source_color(pkg.source)))
                                .child(tag(theme::kind_label(pkg.app_kind), theme::kind_color(pkg.app_kind))),
                        ),
                ),
        )
        .when_some(pkg.description.clone(), |this, description| {
            this.child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .text_color(text_dim())
                    .child(description),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .children(
                    rows.into_iter()
                        .map(|(label, value)| kv_row(label, value)),
                ),
        )
        .into_any_element()
}
