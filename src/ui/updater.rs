//! Self-update banner state and element.
//!
//! Small module so `app_view.rs` stays composition-only: this owns the
//! updater status machine, `app_view` owns the async pumps that drive it.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    Disableable, Icon,
};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight, IntoElement, SharedString};

use crate::domain::operations::OperationResult;
use crate::domain::updater::{InstallKind, UpdateCheck, RELEASES_URL};
use crate::theme;
use crate::ui::widgets::{button, progress_bar, ButtonStyle};

/// The updater lifecycle. Each phase carries exactly the data it needs, so an
/// "available" banner cannot render without a check and a finished banner cannot
/// render without a message — the invalid combinations are unrepresentable.
#[derive(Debug, Default)]
pub enum UpdaterStatus {
    #[default]
    Idle,
    Checking,
    Dismissed,
    UpToDate,
    /// A newer release is available and can be acted on.
    Available(UpdateCheck),
    /// Download/install in progress, with the tail of live output.
    Installing {
        check: UpdateCheck,
        lines: Vec<String>,
    },
    /// Finished successfully.
    Ready {
        message: String,
    },
    /// Finished with an error.
    Error {
        message: String,
    },
}

#[derive(Debug, Default)]
pub struct UpdaterUi {
    pub status: UpdaterStatus,
    pub confirming: bool,
}

impl UpdaterUi {
    pub fn show_banner(&self) -> bool {
        matches!(
            self.status,
            UpdaterStatus::Available(_)
                | UpdaterStatus::Installing { .. }
                | UpdaterStatus::Ready { .. }
                | UpdaterStatus::Error { .. }
        )
    }

    pub fn title(&self) -> SharedString {
        match &self.status {
            UpdaterStatus::Available(_) => "Scope update available".into(),
            UpdaterStatus::Installing { .. } => "Updating Scope…".into(),
            UpdaterStatus::Ready { .. } => "Scope update installed".into(),
            UpdaterStatus::Error { .. } => "Could not update Scope".into(),
            _ => "".into(),
        }
    }

    pub fn body(&self) -> String {
        match &self.status {
            UpdaterStatus::Available(check) => {
                if !check.can_self_update {
                    "Download the latest version to update this installation.".to_string()
                } else if matches!(check.kind, InstallKind::Deb | InstallKind::Rpm) {
                    "Your desktop will ask for your administrator password.".to_string()
                } else {
                    "Install the latest version, then restart Scope.".to_string()
                }
            }
            UpdaterStatus::Installing { check, .. } => {
                format!(
                    "Installing version {}. You can keep browsing.",
                    check.latest
                )
            }
            UpdaterStatus::Ready { message } | UpdaterStatus::Error { message } => message.clone(),
            _ => String::new(),
        }
    }

    /// The update that can be installed, if one is available.
    pub fn available(&self) -> Option<&UpdateCheck> {
        match &self.status {
            UpdaterStatus::Available(check) => Some(check),
            _ => None,
        }
    }

    /// Transition into the installing phase, returning the check to install.
    pub fn begin_install(&mut self) -> Option<UpdateCheck> {
        if !self.confirming {
            return None;
        }
        let check = self.available()?.clone();
        if !check.can_self_update {
            return None;
        }
        self.confirming = false;
        self.status = UpdaterStatus::Installing {
            check: check.clone(),
            lines: Vec::new(),
        };
        Some(check)
    }

    /// Append one line of install output, keeping only the last five.
    pub fn push_line(&mut self, line: String) {
        if let UpdaterStatus::Installing { lines, .. } = &mut self.status {
            lines.push(line);
            if lines.len() > 5 {
                let excess = lines.len() - 5;
                lines.drain(..excess);
            }
        }
    }

    /// Record the outcome of an install attempt.
    pub fn finish(&mut self, result: OperationResult) {
        self.status = if result.success {
            UpdaterStatus::Ready {
                message: "Restart Scope to use the new version.".to_string(),
            }
        } else {
            UpdaterStatus::Error {
                message: result.message,
            }
        };
    }
}

/// Banner row with inline actions. Callbacks come from `app_view` so this
/// module never touches `ScopeApp` directly.
pub fn updater_banner(
    ui: &UpdaterUi,
    can_act: bool,
    on_update: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
    on_dismiss: impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut gpui_kit::App) + 'static,
) -> AnyElement {
    let installing = matches!(ui.status, UpdaterStatus::Installing { .. });
    let icon = match ui.status {
        UpdaterStatus::Ready { .. } => IconName::Check,
        UpdaterStatus::Error { .. } => IconName::CircleAlert,
        _ => IconName::RefreshCw,
    };
    let mut actions = div().flex_none().flex().items_center().gap(px(8.));
    if let Some(check) = ui.available() {
        if !check.notes.trim().is_empty() {
            let notes_url = format!("{RELEASES_URL}/tag/v{}", check.latest);
            actions = actions.child(
                Button::new("updater-notes")
                    .label("Release notes")
                    .ghost()
                    .rounded(px(18.))
                    .h(px(34.))
                    .px(px(12.))
                    .text_size(px(13.))
                    .on_click(move |_, _, cx| cx.open_url(&notes_url)),
            );
        }
        if check.can_self_update {
            actions = actions.child(
                button(
                    "updater-apply",
                    "Update Scope",
                    ButtonStyle::Update,
                    on_update,
                )
                .disabled(!can_act),
            );
        } else {
            let url = check.url.clone();
            actions = actions.child(button(
                "updater-download",
                "Download",
                ButtonStyle::Update,
                move |_, _, cx| cx.open_url(&url),
            ));
        }
    }
    if !installing {
        actions = actions.child(
            Button::new("updater-dismiss")
                .ghost()
                .icon(IconName::Close)
                .accessibility_label("Dismiss Scope update notice")
                .size(px(28.))
                .rounded_full()
                .text_color(theme::text_dim())
                .on_click(on_dismiss),
        );
    }

    div()
        .id("self-update-notice")
        .mx(px(32.))
        .mt(px(10.))
        .mb(px(12.))
        .p(px(16.))
        .rounded(px(18.))
        .bg(theme::elev())
        .flex()
        .flex_col()
        .gap(px(14.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .child(
                    div()
                        .flex_none()
                        .size(px(34.))
                        .rounded_full()
                        .bg(theme::selected_surface())
                        .text_color(theme::accent_text())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(icon).size(px(17.))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(5.))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .text_size(px(14.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(theme::text())
                                        .child(ui.title()),
                                )
                                .when_some(ui.available(), |this, check| {
                                    this.child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(theme::accent_text())
                                            .child(format!("{} → {}", check.current, check.latest)),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(18.))
                                .text_color(theme::text_dim())
                                .child(ui.body()),
                        ),
                )
                .child(actions),
        )
        .when(installing, |this| {
            this.child(progress_bar("self-update-progress"))
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::updater::InstallKind;

    fn check() -> UpdateCheck {
        UpdateCheck {
            current: "0.3.0".into(),
            latest: "0.4.0".into(),
            notes: String::new(),
            url: "https://example.com/scope".into(),
            kind: InstallKind::Deb,
            can_self_update: true,
            artifact_name: Some("scope_0.4.0_amd64.deb".into()),
            checksums_url: Some("https://example.com/SHA256SUMS".into()),
            signature_url: Some("https://example.com/SHA256SUMS.minisig".into()),
        }
    }

    #[test]
    fn available_status_shows_a_banner() {
        let ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        assert!(ui.show_banner());
    }

    #[test]
    fn idle_and_checking_do_not_show_a_banner() {
        assert!(!UpdaterUi::default().show_banner());
        let ui = UpdaterUi {
            status: UpdaterStatus::Checking,
            confirming: false,
        };
        assert!(!ui.show_banner());
    }

    #[test]
    fn available_returns_the_check_to_install() {
        let ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        assert_eq!(ui.available().map(|c| c.latest.as_str()), Some("0.4.0"));
    }

    #[test]
    fn available_update_cannot_install_without_confirmation() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: false,
        };
        assert!(ui.begin_install().is_none());
        assert!(ui.available().is_some());
    }

    #[test]
    fn confirmation_is_consumed_and_cannot_start_a_second_install() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        assert!(ui.begin_install().is_some());
        assert!(!ui.confirming);
        assert!(ui.begin_install().is_none());
    }

    #[test]
    fn unsupported_installation_cannot_install_even_with_confirmation() {
        let mut check = check();
        check.can_self_update = false;
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check),
            confirming: true,
        };
        assert!(ui.begin_install().is_none());
    }

    #[test]
    fn begin_install_moves_into_installing_with_the_check() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        assert_eq!(ui.begin_install().map(|c| c.latest), Some("0.4.0".into()));
    }

    #[test]
    fn push_line_keeps_only_the_last_five() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        let _ = ui.begin_install();
        for i in 0..8 {
            ui.push_line(format!("line {i}"));
        }
        match &ui.status {
            UpdaterStatus::Installing { lines, .. } => {
                assert_eq!(lines.first().map(String::as_str), Some("line 3"));
            }
            other => panic!("expected Installing, got {other:?}"),
        }
    }

    #[test]
    fn finish_records_a_success_message() {
        let mut ui = UpdaterUi::default();
        ui.finish(OperationResult {
            success: true,
            message: "installed".into(),
            logs: String::new(),
            exit_code: Some(0),
        });
        assert!(ui.body().contains("Restart Scope"));
    }

    #[test]
    fn finish_records_a_failure_message() {
        let mut ui = UpdaterUi::default();
        ui.finish(OperationResult {
            success: false,
            message: "boom".into(),
            logs: String::new(),
            exit_code: Some(1),
        });
        assert_eq!(ui.body(), "boom");
    }

    #[test]
    fn installing_keeps_command_output_out_of_the_notice() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
            confirming: true,
        };
        ui.begin_install();
        ui.push_line("[scope] Running: pkexec dpkg -i /tmp/update.deb".into());

        assert_eq!(
            ui.body(),
            "Installing version 0.4.0. You can keep browsing."
        );
    }
}
