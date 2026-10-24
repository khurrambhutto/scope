//! The shared app card for opened Debian packages and AppImages.

use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight, WeakEntity};

use crate::domain::install_metadata::InstallMetadata;
use crate::domain::operations::{OperationPlan, OperationTarget};
use crate::domain::package::InstalledPackage;
use crate::theme;

use super::app_view::{act, ScopeApp};
use super::dialog::modal_close;
use super::row::package_icon;
use super::widgets::{banner, button, BannerKind, ButtonStyle};

pub(super) fn body(
    entity: &WeakEntity<ScopeApp>,
    pkg: &InstalledPackage,
    plan: &OperationPlan,
) -> AnyElement {
    let (metadata, size, version, notes) = match &plan.target {
        OperationTarget::DebInstall { plan: install } => {
            let mut notes = vec![
                "Installs system-wide. Package scripts run with administrator privileges.".into(),
            ];
            if let Some(current) = &install.installed_version {
                notes.push(format!("Replaces installed version {current}."));
            }
            (
                &install.metadata,
                install.size_bytes,
                install.version.as_str(),
                notes,
            )
        }
        OperationTarget::AppImageInstall { plan: install } => {
            let mut notes = vec![if install.move_source {
                "Moves the original file into ~/AppImages and adds it to your app menu.".into()
            } else {
                "Adds this AppImage to your app menu.".into()
            }];
            notes.extend(install.notes.iter().cloned());
            (
                &install.metadata,
                install.source_size,
                install.version.as_deref().unwrap_or("Unknown"),
                notes,
            )
        }
        _ => return div().into_any_element(),
    };
    let mut app = pkg.clone();
    app.display_name = Some(plan.display_name.clone());
    if let Some(icon) = &metadata.icon {
        app.icon = Some(icon.url());
    }

    let mut content = div()
        .id("install-information")
        .min_h(px(0.))
        .overflow_y_scroll()
        .px(px(28.))
        .pb(px(8.))
        .flex()
        .flex_col()
        .gap(px(24.))
        .child(
            div()
                .flex()
                .gap(px(24.))
                .child(fact("File size", theme::format_size(size)))
                .child(fact("Version", version.to_string()))
                .child(website(metadata)),
        )
        .when_some(metadata.description.as_ref(), |this, description| {
            this.child(
                div()
                    .text_size(px(14.))
                    .line_height(px(22.))
                    .child(description.clone()),
            )
        });
    if !notes.is_empty() {
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(theme::text_dim())
                .children(notes.into_iter().map(|note| div().child(note))),
        );
    }
    if let OperationTarget::DebInstall { plan: install } = &plan.target {
        if !install.changes.is_empty() {
            content = content.child(banner(&install.changes.join("\n"), BannerKind::Warn));
        }
    }

    div()
        .min_h(px(0.))
        .flex()
        .flex_col()
        .child(
            div()
                .flex_none()
                .flex()
                .items_start()
                .gap(px(16.))
                .p(px(28.))
                .child(package_icon(&app, 64.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .pt(px(17.))
                        .text_size(px(23.))
                        .line_height(px(29.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(plan.display_name.clone()),
                )
                .child(modal_close(entity)),
        )
        .child(content)
        .child(
            div()
                .flex_none()
                .flex()
                .justify_end()
                .px(px(28.))
                .pt(px(16.))
                .pb(px(24.))
                .child(
                    button(
                        "dlg-confirm",
                        "Install",
                        ButtonStyle::Update,
                        act(entity, |this, cx| this.confirm_op(cx)),
                    )
                    .min_w(px(112.))
                    .h(px(40.))
                    .rounded(px(20.)),
                ),
        )
        .into_any_element()
}

fn fact(label: &'static str, value: String) -> gpui_kit::Div {
    div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(8.))
        .text_size(px(14.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .child(
            div()
                .text_size(px(12.))
                .font_weight(FontWeight::NORMAL)
                .text_color(theme::text_dim())
                .child(label),
        )
        .when(!value.is_empty(), |this| this.child(value))
}

fn website(metadata: &InstallMetadata) -> AnyElement {
    let mut column = fact("Developer site", String::new()).flex_grow(1.4);
    match metadata.homepage.as_ref() {
        Some(url) => {
            let label = url::Url::parse(url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_string))
                .unwrap_or_else(|| url.clone());
            let url = url.clone();
            column = column.child(
                div()
                    .id("install-developer-site")
                    .cursor_pointer()
                    .text_color(theme::accent_text())
                    .hover(|this| this.underline())
                    .child(label)
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            );
        }
        None => column = column.child(div().text_color(theme::text_dim()).child("Not provided")),
    }
    column.into_any_element()
}
