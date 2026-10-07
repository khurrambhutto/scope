//! Uninstall/update dialog: preview, running, and result phases.

use std::time::Duration;

use gpui_kit::prelude::*;
use gpui_kit::{div, px, rgb, rgba, AnyElement, App, ClickEvent, FontWeight, WeakEntity, Window};

use crate::backend::OpKind;
use crate::domain::operations::{OperationPlan, OperationResult, OperationStage};
use crate::domain::package::InstalledPackage;
use crate::theme::{self, accent, border, display_title, elev, elev2, text, text_dim, text_faint};

use super::app_view::{act, ScopeApp};
use super::widgets::{banner, button, dialog_actions, plan_rows, BannerKind, ButtonStyle};

// ---- Operation dialog ------------------------------------------------------

/// The dialog's state machine. Each variant carries exactly the data that
/// phase needs: a plan exists from `Confirm` on, a stage/lines/elapsed only
/// while `Running`, a result only once `Done` — so invalid combinations
/// cannot be constructed.
#[derive(Clone)]
pub(super) enum Dialog {
    Loading {
        kind: OpKind,
        pkg: InstalledPackage,
    },
    Failed {
        kind: OpKind,
        pkg: InstalledPackage,
        message: String,
    },
    Confirm {
        kind: OpKind,
        pkg: InstalledPackage,
        plan: OperationPlan,
    },
    Running {
        kind: OpKind,
        pkg: InstalledPackage,
        plan: OperationPlan,
        stage: OperationStage,
        lines: Vec<String>,
        elapsed: u64,
    },
    Done {
        kind: OpKind,
        pkg: InstalledPackage,
        result: OperationResult,
        show_logs: bool,
    },
}

impl Dialog {
    pub(super) fn kind(&self) -> OpKind {
        match self {
            Dialog::Loading { kind, .. }
            | Dialog::Failed { kind, .. }
            | Dialog::Confirm { kind, .. }
            | Dialog::Running { kind, .. }
            | Dialog::Done { kind, .. } => *kind,
        }
    }

    pub(super) fn pkg(&self) -> &InstalledPackage {
        match self {
            Dialog::Loading { pkg, .. }
            | Dialog::Failed { pkg, .. }
            | Dialog::Confirm { pkg, .. }
            | Dialog::Running { pkg, .. }
            | Dialog::Done { pkg, .. } => pkg,
        }
    }
}

impl ScopeApp {
    pub(super) fn dialog_element(&self, entity: &WeakEntity<ScopeApp>) -> AnyElement {
        let Some(dialog) = &self.dialog else {
            return div().into_any_element();
        };
        let title = display_title(dialog.pkg());
        let heading = match dialog.kind() {
            OpKind::Uninstall => format!("Uninstall {title}"),
            OpKind::Update => format!("Update {title}"),
        };

        let body: AnyElement = match dialog {
            Dialog::Loading { kind, .. } => div()
                .p(px(20.))
                .text_color(text_dim())
                .text_size(px(14.))
                .child(match kind {
                    OpKind::Uninstall => "Preparing uninstall preview…",
                    OpKind::Update => "Preparing update preview…",
                })
                .into_any_element(),
            Dialog::Failed { message, .. } => div()
                .p(px(20.))
                .flex()
                .flex_col()
                .gap(px(16.))
                .child(banner(message, BannerKind::Error))
                .child(div().flex().justify_end().child(button(
                    "dlg-error-close",
                    "Close",
                    ButtonStyle::Neutral,
                    act(entity, |this, cx| this.close_dialog(cx)),
                )))
                .into_any_element(),
            Dialog::Confirm { kind, pkg, plan } => confirm_body(entity, *kind, pkg, plan),
            Dialog::Running {
                kind,
                pkg,
                plan,
                stage,
                lines,
                elapsed,
            } => running_body(*kind, pkg, plan, *stage, lines, *elapsed),
            Dialog::Done {
                result, show_logs, ..
            } => done_body(entity, result, *show_logs),
        };

        modal_shell(heading, body, entity)
    }
}

/// The modal overlay and card chrome shared by every dialog phase.
fn modal_shell(heading: String, body: AnyElement, entity: &WeakEntity<ScopeApp>) -> AnyElement {
    div()
        .id("modal-overlay")
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        // Block mouse events from reaching the package list behind the modal.
        // Without this the overlay paints on top but clicks/hover fall through
        // to the rows underneath.
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000099))
        .on_click(act(entity, |this, cx| this.close_dialog(cx)))
        .child(
            div()
                .id("modal-card")
                .on_click(|_ev: &ClickEvent, _window: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                })
                .w(px(560.))
                .max_h(px(600.))
                .overflow_y_scroll()
                .rounded(px(16.))
                .border_1()
                .border_color(border())
                .bg(elev())
                .shadow_lg()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(20.))
                        .py(px(16.))
                        .border_b_1()
                        .border_color(border())
                        .child(
                            div()
                                .text_size(px(17.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(heading),
                        )
                        .child(
                            div()
                                .id("modal-close")
                                .cursor_pointer()
                                .px(px(6.))
                                .rounded(px(6.))
                                .text_color(text_dim())
                                .hover(|this| this.text_color(text()).bg(elev2()))
                                .on_click(act(entity, |this, cx| this.close_dialog(cx)))
                                .child("✕"),
                        ),
                )
                .child(body),
        )
        .into_any_element()
}

fn confirm_body(
    entity: &WeakEntity<ScopeApp>,
    kind: OpKind,
    pkg: &InstalledPackage,
    plan: &OperationPlan,
) -> AnyElement {
    // Protected covers AppImages too: `safety::check_package` denies every
    // AppImage path, so their preview plans always arrive protected.
    if plan.protected {
        let reason = plan
            .protection_reason
            .clone()
            .unwrap_or_else(|| "This package is protected and cannot be changed.".to_string());
        return div()
            .p(px(20.))
            .flex()
            .flex_col()
            .child(banner(&reason, BannerKind::Warn))
            .child(dialog_actions(vec![button(
                "dlg-protected-cancel",
                "Close",
                ButtonStyle::Neutral,
                act(entity, |this, cx| this.close_dialog(cx)),
            )
            .into_any_element()]))
            .into_any_element();
    }

    let mut rows: Vec<(&'static str, String)> = vec![("Package", plan.target.id().to_string())];
    if let Some(scope) = plan.target.install_scope() {
        rows.push(("Scope", scope.id().to_string()));
    }
    match kind {
        OpKind::Uninstall => {
            rows.push((
                "Version",
                if plan.current_version.is_empty() {
                    "—".to_string()
                } else {
                    plan.current_version.clone()
                },
            ));
        }
        OpKind::Update => {
            rows.push((
                "Current version",
                if plan.current_version.is_empty() {
                    "—".to_string()
                } else {
                    plan.current_version.clone()
                },
            ));
            rows.push((
                "Target version",
                if plan.target_version.is_empty() {
                    "latest".to_string()
                } else {
                    plan.target_version.clone()
                },
            ));
        }
    }
    rows.push(("Size", theme::format_size(pkg.size_bytes)));

    let confirm_style = match kind {
        OpKind::Uninstall => ButtonStyle::Danger,
        OpKind::Update => ButtonStyle::Update,
    };

    div()
        .p(px(20.))
        .child(plan_rows(rows))
        .child(dialog_actions(vec![
            button(
                "dlg-cancel",
                "Cancel",
                ButtonStyle::Neutral,
                act(entity, |this, cx| this.close_dialog(cx)),
            )
            .into_any_element(),
            button(
                "dlg-confirm",
                "Confirm",
                confirm_style,
                act(entity, |this, cx| this.confirm_op(cx)),
            )
            .into_any_element(),
        ]))
        .into_any_element()
}

fn running_body(
    kind: OpKind,
    pkg: &InstalledPackage,
    plan: &OperationPlan,
    stage: OperationStage,
    lines: &[String],
    elapsed: u64,
) -> AnyElement {
    let title = display_title(pkg);
    let requires_auth = plan.requires_auth;
    let verb = match kind {
        OpKind::Uninstall => "removed",
        OpKind::Update => "updated",
    };
    let text = if stage == OperationStage::Verifying {
        format!("Checking that {title} can be {verb}…")
    } else {
        format!(
            "{} {title}…",
            match kind {
                OpKind::Uninstall => "Removing",
                OpKind::Update => "Updating",
            }
        )
    };
    let hint = if requires_auth {
        "A system password prompt may appear."
    } else {
        ""
    };
    let logs = if lines.is_empty() {
        "Waiting for output…".to_string()
    } else {
        lines.join("\n")
    };

    div()
        .p(px(20.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(spinner())
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(px(14.))
                        .text_color(text_dim())
                        .child(format!("{text} {hint}")),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(text_faint())
                        .child(theme::format_elapsed(elapsed)),
                ),
        )
        .child(
            div()
                .mt(px(14.))
                .p(px(12.))
                .max_h(px(200.))
                .overflow_hidden()
                .rounded(px(10.))
                .border_1()
                .border_color(border())
                .bg(rgb(0x0b0c0f))
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(text_dim())
                .child(logs),
        )
        .into_any_element()
}

/// A simple pulsing dot standing in for the CSS spinner (GPUI has no
/// transform animation on `Div`, so we animate opacity instead).
fn spinner() -> impl IntoElement {
    use gpui_kit::{Animation, AnimationExt};
    div()
        .flex_none()
        .size(px(10.))
        .rounded_full()
        .bg(accent())
        .with_animation(
            "spinner-pulse",
            Animation::new(Duration::from_millis(800)).repeat(),
            |this, delta| this.opacity(0.3 + delta * 0.7),
        )
}

fn done_body(
    entity: &WeakEntity<ScopeApp>,
    result: &OperationResult,
    show_logs: bool,
) -> AnyElement {
    if is_cancelled(result) {
        return div()
            .p(px(20.))
            .child(banner(
                "Authentication was cancelled. Nothing was changed.",
                BannerKind::Muted,
            ))
            .child(dialog_actions(vec![button(
                "dlg-cancelled-close",
                "Close",
                ButtonStyle::Neutral,
                act(entity, |this, cx| this.close_dialog(cx)),
            )
            .into_any_element()]))
            .into_any_element();
    }

    let kind = if result.success {
        BannerKind::Ok
    } else {
        BannerKind::Error
    };
    let label = if result.success { "Done" } else { "Close" };

    div()
        .p(px(20.))
        .child(banner(&result.message, kind))
        .child(
            div()
                .id("dlg-logtoggle")
                .mt(px(10.))
                .text_size(px(13.))
                .text_color(accent())
                .cursor_pointer()
                .on_click(act(entity, |this, cx| {
                    if let Some(Dialog::Done { show_logs, .. }) = &mut this.dialog {
                        *show_logs = !*show_logs;
                    }
                    cx.notify();
                }))
                .child(format!(
                    "{} command output",
                    if show_logs { "Hide" } else { "Show" }
                )),
        )
        .when(show_logs, |this| {
            this.child(
                div()
                    .id("dlg-logs")
                    .mt(px(8.))
                    .p(px(12.))
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .rounded(px(10.))
                    .border_1()
                    .border_color(border())
                    .bg(rgb(0x0b0c0f))
                    .text_size(px(12.))
                    .text_color(text_dim())
                    .child(result.logs.clone()),
            )
        })
        .child(dialog_actions(vec![button(
            "dlg-done",
            label,
            ButtonStyle::Neutral,
            act(entity, |this, cx| this.close_dialog(cx)),
        )
        .into_any_element()]))
        .into_any_element()
}

/// A dismissed Polkit prompt comes back as exit 126 rather than an error.
fn is_cancelled(result: &OperationResult) -> bool {
    if result.success || result.exit_code != Some(126) {
        return false;
    }
    let haystack = format!("{}\n{}", result.message, result.logs).to_lowercase();
    haystack.contains("dismiss") || haystack.contains("cancel")
}
