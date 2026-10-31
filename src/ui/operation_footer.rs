//! Compact package operation status embedded in the app footer.

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, relative, AnyElement, FontWeight, WeakEntity};

use crate::backend::OpKind;
use crate::domain::operations::{OperationResult, OperationStage};
use crate::domain::package::InstalledPackage;
use crate::theme;

use super::app_view::{act, ScopeApp};
use super::widgets::{dismiss_button, progress_bar};

pub(super) enum OperationStatus {
    Running {
        kind: OpKind,
        pkg: InstalledPackage,
        requires_auth: bool,
        stage: OperationStage,
        elapsed: u64,
    },
    Done {
        kind: OpKind,
        pkg: InstalledPackage,
        result: OperationResult,
    },
}

pub(super) fn operation_footer(
    entity: &WeakEntity<ScopeApp>,
    status: &OperationStatus,
) -> AnyElement {
    let (heading, hint, running) = match status {
        OperationStatus::Running {
            kind,
            pkg,
            requires_auth,
            stage,
            ..
        } => {
            let title = theme::display_title(pkg);
            let heading = match stage {
                OperationStage::Verifying => format!("Checking {title}…"),
                OperationStage::Executing => match kind {
                    OpKind::Uninstall => format!("Uninstalling {title}…"),
                    OpKind::Update => format!("Updating {title}…"),
                    OpKind::Install => format!("Installing {title}…"),
                },
            };
            let hint = if *requires_auth {
                "A password prompt may appear."
            } else {
                ""
            };
            (heading, hint.to_string(), true)
        }
        OperationStatus::Done { kind, pkg, result } => {
            let title = theme::display_title(pkg);
            if result.success {
                let heading = match kind {
                    OpKind::Uninstall if !result.warnings.is_empty() => {
                        format!("{title} was uninstalled with cleanup warnings")
                    }
                    OpKind::Uninstall => format!("{title} was uninstalled"),
                    OpKind::Update => format!("{title} was updated"),
                    OpKind::Install => format!("{title} was installed"),
                };
                (heading, String::new(), false)
            } else if is_cancelled(result) {
                (
                    "Authentication cancelled".to_string(),
                    "Nothing was changed.".to_string(),
                    false,
                )
            } else {
                let heading = match kind {
                    OpKind::Uninstall => format!("Could not uninstall {title}"),
                    OpKind::Update => format!("Could not update {title}"),
                    OpKind::Install => format!("Could not install {title}"),
                };
                (heading, result.message.clone(), false)
            }
        }
    };
    let cancelled = matches!(status, OperationStatus::Done { result, .. } if is_cancelled(result));
    let failed = matches!(status, OperationStatus::Done { result, .. } if !result.success && !is_cancelled(result));

    let mut warnings = match status {
        OperationStatus::Done { result, .. } => result.warnings.clone(),
        OperationStatus::Running { .. } => Vec::new(),
    };

    if let OperationStatus::Done { result, .. } = status {
        for outcome in &result.cleanup {
            let status = match outcome.status {
                crate::domain::app_data::CleanupStatus::Included => "Included in parent removal",
                crate::domain::app_data::CleanupStatus::Trashed => "Moved to Trash",
                crate::domain::app_data::CleanupStatus::Removed => "Removed",
                crate::domain::app_data::CleanupStatus::Kept => "Kept",
                crate::domain::app_data::CleanupStatus::Shared => "Shared — kept",
                crate::domain::app_data::CleanupStatus::Changed => "Changed — kept",
                crate::domain::app_data::CleanupStatus::Failed => "Cleanup failed",
                crate::domain::app_data::CleanupStatus::Unknown => "Could not verify",
            };
            warnings.push(format!(
                "{status}: {} — {}",
                if outcome.path.as_os_str().is_empty() {
                    outcome.label.clone()
                } else {
                    outcome.path.display().to_string()
                },
                outcome.message
            ));
        }
    }

    let mut row = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .when(!running, |this| {
            this.child(
                Icon::new(if failed || matches!(status, OperationStatus::Done { result, .. } if !result.warnings.is_empty()) {
                    IconName::CircleAlert
                } else if cancelled {
                    IconName::Close
                } else {
                    IconName::Check
                })
                .size(px(16.))
                .text_color(theme::accent_text()),
            )
        })
        .child(
            div()
                .min_w(px(0.))
                .when(running, |this| this.w(relative(0.4)).max_w(px(240.)))
                .when(!running, |this| this.flex_1())
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if failed {
                            theme::accent_text()
                        } else {
                            theme::text()
                        })
                        .truncate()
                        .child(heading),
                )
                .when(!hint.is_empty(), |this| {
                    this.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme::text_dim())
                            .child(hint),
                    )
                }),
        )
        .when(running, |this| {
            this.child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .child(progress_bar("operation-progress")),
            )
        });
    if let OperationStatus::Running { elapsed, .. } = status {
        row = row.child(
            div()
                .flex_none()
                .text_size(px(12.))
                .text_color(theme::text_faint())
                .child(theme::format_elapsed(*elapsed)),
        );
    } else {
        row = row.child(dismiss_button(
            "operation-dismiss".into(),
            "Dismiss operation result",
            act(entity, |this, cx| {
                if matches!(this.operation_status, Some(OperationStatus::Done { .. })) {
                    this.operation_status = None;
                    cx.notify();
                }
            }),
        ));
    }

    div()
        .id("operation-footer")
        .flex_1()
        .min_w(px(0.))
        .min_h(px(28.))
        .bg(theme::elev())
        .rounded(px(18.))
        .pl(px(16.))
        .pr(px(10.))
        .py(px(8.))
        .flex()
        .justify_center()
        .flex_col()
        .child(row)
        .when(!warnings.is_empty(), |this| {
            this.child(
                div()
                    .id("operation-warnings")
                    .max_h(px(140.))
                    .overflow_y_scroll()
                    .mt(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .text_size(px(12.))
                    .text_color(theme::text_dim())
                    .children(warnings.into_iter().map(|warning| div().child(warning))),
            )
        })
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
