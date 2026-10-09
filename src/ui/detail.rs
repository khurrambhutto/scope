//! Inline detail panel shown beneath the selected row.

use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight, SharedString};

use crate::domain::listing;
use crate::domain::package::{InstalledPackage, PackageSource};
use crate::domain::snap_details::SnapDetails;
use crate::theme::{self, display_title, text_dim};

use super::row::package_icon;
use super::widgets::{button, kv_row, tag, ButtonStyle};

pub(super) fn detail_element(
    pkg: &InstalledPackage,
    snap_details: Option<&SnapDetails>,
) -> AnyElement {
    if pkg.source == PackageSource::Snap {
        return div()
            .px(px(16.))
            .pb(px(14.))
            .pt(px(6.))
            .child(super::snap_details::breakdown(snap_details))
            .into_any_element();
    }
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
        (
            "Installed size".to_string(),
            theme::format_size(pkg.size_bytes),
        ),
        (
            "Categories".to_string(),
            pkg.categories.clone().unwrap_or_else(|| "—".to_string()),
        ),
    ];
    let steam_status_note = (pkg.source == PackageSource::Steam)
        .then_some("Steam's last recorded status; open Steam to check for newer updates.");
    if pkg.source == PackageSource::Steam {
        rows.push((
            "Steam update".to_string(),
            theme::steam_update_status_label(pkg.steam_update_status).to_string(),
        ));
    } else {
        rows.push((
            "Update available".to_string(),
            if pkg.has_update {
                "Yes".to_string()
            } else {
                "—".to_string()
            },
        ));
    }
    if let Some(reason) = listing::uninstall_block_reason(pkg) {
        rows.push(("Uninstall".to_string(), reason));
    }
    rows.retain(|(label, value)| {
        !value.is_empty()
            && value != "—"
            && !(pkg.source == PackageSource::Snap && label == "Installed size")
    });

    div()
        .id(SharedString::from(format!("package-detail:{}", pkg.key)))
        .mx(px(8.))
        .mb(px(8.))
        .ml(px(12.))
        .p(px(16.))
        .rounded(px(12.))
        .border_1()
        .border_color(theme::border())
        .bg(theme::elev())
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
                                .child(tag(
                                    theme::package_source_label(pkg),
                                    theme::source_color(pkg.source),
                                ))
                                .child(tag(
                                    theme::kind_label(pkg.app_kind),
                                    theme::kind_color(pkg.app_kind),
                                )),
                        ),
                ),
        )
        .when_some(
            pkg.description
                .clone()
                .or_else(|| snap_details.and_then(|details| details.summary.clone())),
            |this, description| {
                this.child(
                    div()
                        .text_size(px(14.))
                        .line_height(px(22.))
                        .text_color(text_dim())
                        .child(description),
                )
            },
        )
        .when_some(steam_status_note, |this, note| {
            this.child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(text_dim())
                    .child(note),
            )
        })
        .child(
            div()
                .flex()
                .flex_col()
                .children(rows.into_iter().map(|(label, value)| kv_row(label, value))),
        )
        .when_some(pkg.steam_library_url(), |this, url| {
            this.child(
                div().flex().justify_end().child(
                    button(
                        "detail-open-steam",
                        "Open in Steam",
                        ButtonStyle::Neutral,
                        move |_ev, _window, cx| {
                            cx.stop_propagation();
                            cx.open_url(&url);
                        },
                    )
                    .tooltip("Manage updates in Steam")
                    .accessibility_label("Open this item in Steam"),
                ),
            )
        })
        .into_any_element()
}
