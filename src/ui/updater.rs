//! Self-update banner state and element.
//!
//! Small module so `app_view.rs` stays composition-only: this owns the
//! updater status machine, `app_view` owns the async pumps that drive it.

use gpui::prelude::*;
use gpui::{div, px, AnyElement, IntoElement, SharedString};

use crate::domain::operations::OperationResult;
use crate::domain::updater::UpdateCheck;
use crate::ui::widgets::{banner, BannerKind};

/// The updater lifecycle. Each phase carries exactly the data it needs, so an
/// "available" banner cannot render without a check and a finished banner cannot
/// render without a message — the invalid combinations are unrepresentable.
#[derive(Debug, Default)]
pub enum UpdaterStatus {
    #[default]
    Idle,
    Checking,
    Dismissed,
    /// A newer release is available and can be acted on.
    Available(UpdateCheck),
    /// Download/install in progress, with the tail of live output.
    Installing {
        check: UpdateCheck,
        lines: Vec<String>,
    },
    /// Finished successfully.
    Ready { message: String },
    /// Finished with an error.
    Error { message: String },
}

#[derive(Debug, Default)]
pub struct UpdaterUi {
    pub status: UpdaterStatus,
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
            UpdaterStatus::Available(_) => "A new version of Scope is available".into(),
            UpdaterStatus::Installing { .. } => "Installing update…".into(),
            UpdaterStatus::Ready { .. } => "Update ready".into(),
            UpdaterStatus::Error { .. } => "Update failed".into(),
            _ => "".into(),
        }
    }

    pub fn body(&self) -> String {
        match &self.status {
            UpdaterStatus::Available(check) => {
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
            }
            UpdaterStatus::Installing { check, lines } => {
                if lines.is_empty() {
                    format!("Downloading {}…", check.latest)
                } else {
                    lines.last().cloned().unwrap_or_default()
                }
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
        let check = self.available()?.clone();
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
                message: format!("{}. Restart Scope to use the new version.", result.message),
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
    on_update: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let kind = match ui.status {
        UpdaterStatus::Error { .. } => BannerKind::Error,
        UpdaterStatus::Ready { .. } => BannerKind::Ok,
        _ => BannerKind::Warn,
    };
    let text = format!("{}\n{}", ui.title(), ui.body());
    let show_update = matches!(ui.status, UpdaterStatus::Available(_))
        && ui.available().is_some_and(|c| c.can_self_update)
        && can_act;
    let action_label: &'static str = match ui.status {
        UpdaterStatus::Available(_) => "Update",
        UpdaterStatus::Ready { .. } | UpdaterStatus::Error { .. } => "Dismiss",
        UpdaterStatus::Installing { .. } => "Working…",
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
        }
    }

    #[test]
    fn available_status_shows_a_banner() {
        let ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
        };
        assert!(ui.show_banner());
    }

    #[test]
    fn idle_and_checking_do_not_show_a_banner() {
        assert!(!UpdaterUi::default().show_banner());
        let ui = UpdaterUi {
            status: UpdaterStatus::Checking,
        };
        assert!(!ui.show_banner());
    }

    #[test]
    fn available_returns_the_check_to_install() {
        let ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
        };
        assert_eq!(ui.available().map(|c| c.latest.as_str()), Some("0.4.0"));
    }

    #[test]
    fn begin_install_moves_into_installing_with_the_check() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
        };
        assert_eq!(ui.begin_install().map(|c| c.latest), Some("0.4.0".into()));
    }

    #[test]
    fn push_line_keeps_only_the_last_five() {
        let mut ui = UpdaterUi {
            status: UpdaterStatus::Available(check()),
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
}
