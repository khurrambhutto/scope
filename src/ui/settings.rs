//! App information and the explicit self-update confirmation flow.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::Disableable;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, svg, AnyElement, Context, FontWeight, Pixels, WeakEntity};

use super::app_view::{act, ScopeApp};
use super::dialog::settings_modal_shell;
use super::updater::UpdaterStatus;
use super::widgets::{button, ButtonStyle};
use crate::theme;

const WEBSITE_URL: &str = "https://khurrambhutto.github.io/scope/";
const PROJECT_URL: &str = "https://github.com/khurrambhutto/scope";
const ISSUES_URL: &str = "https://github.com/khurrambhutto/scope/issues";
const ICON_BUG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/bug.svg");

impl ScopeApp {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            return;
        }
        self.open_select = None;
        self.settings_open = true;
        cx.notify();
    }

    pub(super) fn request_self_update(&mut self, cx: &mut Context<Self>) {
        if self.dialog.is_some() || self.updater_busy {
            return;
        }
        if self
            .updater
            .available()
            .is_some_and(|check| check.can_self_update)
        {
            self.open_settings(cx);
            self.updater.confirming = true;
            cx.notify();
        }
    }

    pub(super) fn settings_element(
        &self,
        entity: &WeakEntity<ScopeApp>,
        corner_radius: Pixels,
    ) -> AnyElement {
        let busy = self.updater_busy || matches!(self.updater.status, UpdaterStatus::Checking);
        let body = if self.updater.confirming {
            let version = self
                .updater
                .available()
                .map(|check| check.latest.as_str())
                .unwrap_or("");
            div()
                .px(px(20.))
                .pt(px(4.))
                .pb(px(20.))
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("Install Scope {version}?")),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(theme::text_dim())
                        .child("Scope will download and verify the update before installing it. Restart Scope when it finishes."),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(button(
                            "settings-update-cancel",
                            "Cancel",
                            ButtonStyle::Neutral,
                            act(entity, |this, cx| {
                                this.updater.confirming = false;
                                cx.notify();
                            }),
                        ))
                        .child(
                            button(
                                "settings-update-confirm",
                                "Install update",
                                ButtonStyle::Update,
                                act(entity, |this, cx| this.start_update(cx)),
                            )
                            .disabled(busy),
                        ),
                )
                .into_any_element()
        } else {
            let label = match &self.updater.status {
                UpdaterStatus::Checking => "Checking…",
                UpdaterStatus::UpToDate if self.settings_checked => "Up to date",
                UpdaterStatus::Available(check) if check.can_self_update => "Update available",
                UpdaterStatus::Available(_) => "Download update",
                UpdaterStatus::Installing { .. } => "Installing…",
                UpdaterStatus::Ready { .. } => "Restart Scope",
                UpdaterStatus::Error { .. } if self.settings_checked => "Check failed · Retry",
                _ => "Check for updates",
            };
            let update = button(
                "settings-check-updates",
                label,
                ButtonStyle::Neutral,
                act(entity, |this, cx| {
                    if let Some(check) = this.updater.available() {
                        if check.can_self_update {
                            this.request_self_update(cx);
                        } else {
                            cx.open_url(&check.url);
                        }
                    } else {
                        this.settings_checked = true;
                        this.check_updater(cx);
                    }
                }),
            )
            .disabled(busy || matches!(self.updater.status, UpdaterStatus::Ready { .. }))
            .into_any_element();
            div()
                .px(px(20.))
                .pb(px(8.))
                .flex()
                .flex_col()
                .child(settings_row(
                    "Version",
                    value_text(format!("v{}", env!("CARGO_PKG_VERSION"))),
                ))
                .child(settings_row("Updates", update))
                .child(settings_row(
                    "Website",
                    link("settings-website", "khurrambhutto.github.io", WEBSITE_URL),
                ))
                .child(settings_row(
                    "Source code",
                    Button::new("settings-project")
                        .ghost()
                        .icon(IconName::Github)
                        .accessibility_label("Open Scope on GitHub")
                        .size(px(36.))
                        .rounded_full()
                        .on_click(|_, _, cx| cx.open_url(PROJECT_URL))
                        .into_any_element(),
                ))
                .child(settings_row("Report issue", bug_button()))
                .into_any_element()
        };
        settings_modal_shell(body, entity, corner_radius)
    }
}

fn value_text(value: String) -> AnyElement {
    div()
        .text_size(px(14.))
        .text_color(theme::text_dim())
        .whitespace_nowrap()
        .child(value)
        .into_any_element()
}

/// The bug icon is outside the kit's default bundle, so it is drawn from `assets/bug.svg`.
fn bug_button() -> AnyElement {
    div()
        .id("settings-report")
        .cursor_pointer()
        .size(px(32.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .hover(|this| this.bg(theme::hover_surface()))
        .on_click(|_, _, cx| cx.open_url(ISSUES_URL))
        .child(
            svg()
                .path(ICON_BUG)
                .size(px(20.))
                .flex_none()
                .text_color(theme::text()),
        )
        .into_any_element()
}

fn link(id: &'static str, label: &'static str, url: &'static str) -> AnyElement {
    div()
        .id(id)
        .cursor_pointer()
        .text_size(px(14.))
        .text_color(theme::accent_text())
        .hover(|this| this.text_color(theme::text()))
        .whitespace_nowrap()
        .on_click(move |_, _, cx| cx.open_url(url))
        .child(label)
        .into_any_element()
}

/// A standard settings row: label on the left, control or value on the right.
/// Fixed row height keeps status changes from shifting the list.
fn settings_row(label: &'static str, value: AnyElement) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .h(px(52.))
        .child(
            div()
                .flex_none()
                .text_size(px(14.))
                .text_color(theme::text())
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_end()
                .min_w(px(0.))
                .child(value),
        )
}
