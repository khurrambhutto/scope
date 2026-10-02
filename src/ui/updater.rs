//! Self-update banner state and element.
//!
//! Small module so `app_view.rs` stays composition-only: this owns the
//! updater status machine, `app_view` owns the async pumps that drive it.

use gpui::prelude::*;
use gpui::{div, px, AnyElement, IntoElement, SharedString};

use crate::domain::updater::UpdateCheck;
use crate::ui::widgets::{banner, BannerKind};

#[derive(Debug, Default)]
pub enum UpdaterStatus {
    #[default]
    Idle,
    Checking,
    Available,
    Installing,
    Ready,
    Error,
    Dismissed,
}

#[derive(Debug, Default)]
pub struct UpdaterUi {
    pub status: UpdaterStatus,
    pub check: Option<UpdateCheck>,
    pub lines: Vec<String>,
    pub message: String,
}

impl UpdaterUi {
    pub fn show_banner(&self) -> bool {
        match &self.status {
            UpdaterStatus::Available
            | UpdaterStatus::Installing
            | UpdaterStatus::Ready
            | UpdaterStatus::Error => self.check.is_some() || !self.message.is_empty(),
            UpdaterStatus::Idle | UpdaterStatus::Checking | UpdaterStatus::Dismissed => false,
        }
    }

    pub fn title(&self) -> SharedString {
        match &self.status {
            UpdaterStatus::Available => "A new version of Scope is available".into(),
            UpdaterStatus::Installing => "Installing update…".into(),
            UpdaterStatus::Ready => "Update ready".into(),
            UpdaterStatus::Error => "Update failed".into(),
            _ => "".into(),
        }
    }

    pub fn body(&self) -> String {
        match &self.status {
            UpdaterStatus::Available => {
                if let Some(check) = &self.check {
                    let mut s = format!("{} → {}", check.current, check.latest);
                    if !check.can_self_update {
                        s.push_str(&format!(
                            "\nThis install can't update itself. Download from {}",
                            check.url
                        ));
                    } else if check.kind == crate::domain::updater::InstallKind::Deb
                        || check.kind == crate::domain::updater::InstallKind::Rpm
                    {
                        s.push_str("\nYour desktop will ask for your administrator password.");
                    }
                    if !check.notes.is_empty() {
                        let first: String =
                            check.notes.lines().take(3).collect::<Vec<_>>().join("\n");
                        s.push_str(&format!("\n{first}"));
                    }
                    s
                } else {
                    String::new()
                }
            }
            UpdaterStatus::Installing => {
                if self.lines.is_empty() {
                    "Downloading…".to_string()
                } else {
                    self.lines.last().cloned().unwrap_or_default()
                }
            }
            UpdaterStatus::Ready | UpdaterStatus::Error => self.message.clone(),
            _ => String::new(),
        }
    }
}

/// Banner row with inline actions. Callbacks come from `app_view` so this
/// module never touches `ScopeApp` directly.
pub fn updater_banner(
    ui: &UpdaterUi,
    can_act: bool,
    on_update: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let kind = match ui.status {
        UpdaterStatus::Error => BannerKind::Error,
        UpdaterStatus::Ready => BannerKind::Ok,
        _ => BannerKind::Warn,
    };
    let text = format!("{}\n{}", ui.title(), ui.body());
    let show_update = matches!(ui.status, UpdaterStatus::Available)
        && ui.check.as_ref().is_some_and(|c| c.can_self_update)
        && can_act;
    let action_label: &'static str = match ui.status {
        UpdaterStatus::Available => "Update",
        UpdaterStatus::Ready | UpdaterStatus::Error => "Dismiss",
        UpdaterStatus::Installing => "Working…",
        _ => "Dismiss",
    };
    // Build manually: message banner + action row (widgets::banner is
    // text-only, so compose here instead of extending it).
    div()
        .mx(px(18.))
        .mt(px(10.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(12.))
        .border_1()
        .child(banner(&text, kind).into_any_element())
        .child(
            div()
                .flex()
                .justify_end()
                .gap(px(10.))
                .mt(px(8.))
                .child(
                    div()
                        .id("updater-dismiss")
                        .px(px(16.))
                        .py(px(6.))
                        .rounded(px(10.))
                        .cursor_pointer()
                        .on_click(on_dismiss)
                        .child("Later"),
                )
                .when(show_update, |this| {
                    this.child(
                        div()
                            .id("updater-apply")
                            .px(px(16.))
                            .py(px(6.))
                            .rounded(px(10.))
                            .cursor_pointer()
                            .on_click(on_update)
                            .child(action_label),
                    )
                }),
        )
        .into_any_element()
}
