//! Compact package operation status embedded in the app footer.

use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    Icon,
};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, relative, AnyElement, FontWeight, WeakEntity};

use crate::backend::OpKind;
use crate::domain::operations::{OperationResult, OperationStage};
use crate::domain::package::InstalledPackage;
use crate::theme;

use super::app_view::{act, ScopeApp};

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
                    OpKind::Uninstall => format!("{title} was uninstalled"),
                    OpKind::Update => format!("{title} was updated"),
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
                };
                (heading, result.message.clone(), false)
            }
        }
    };
    let cancelled = matches!(status, OperationStatus::Done { result, .. } if is_cancelled(result));
    let failed = matches!(status, OperationStatus::Done { result, .. } if !result.success && !is_cancelled(result));

    let mut row = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .when(!running, |this| {
            this.child(
                Icon::new(if failed {
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
            this.child(div().flex_1().min_w(px(0.)).child(progress_bar()))
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
        row = row.child(
            Button::new("operation-dismiss")
                .ghost()
                .icon(IconName::Close)
                .accessibility_label("Dismiss operation result")
                .size(px(28.))
                .rounded_full()
                .text_color(theme::text_dim())
                .on_click(act(entity, |this, cx| {
                    if matches!(this.operation_status, Some(OperationStatus::Done { .. })) {
                        this.operation_status = None;
                        cx.notify();
                    }
                })),
        );
    }

    div()
        .id("operation-footer")
        .flex_1()
        .min_w(px(0.))
        .min_h(px(48.))
        .bg(theme::elev())
        .rounded(px(18.))
        .pl(px(16.))
        .pr(px(10.))
        .py(px(8.))
        .flex()
        .justify_center()
        .flex_col()
        .child(row)
        .into_any_element()
}

/// Package managers expose stages, but no reliable completion percentage.
fn progress_bar() -> impl IntoElement {
    use gpui_kit::{Animation, AnimationExt};

    div()
        .w_full()
        .h(px(4.))
        .relative()
        .overflow_hidden()
        .rounded_full()
        .bg(theme::border())
        .child(
            div()
                .absolute()
                .top_0()
                .h_full()
                .w(relative(0.3))
                .rounded_full()
                .bg(theme::accent())
                .with_animation(
                    "operation-progress",
                    Animation::new(Duration::from_millis(1500)).repeat(),
                    |this, delta| this.left(relative(-0.3 + delta * 1.3)),
                ),
        )
}

/// A dismissed Polkit prompt comes back as exit 126 rather than an error.
fn is_cancelled(result: &OperationResult) -> bool {
    if result.success || result.exit_code != Some(126) {
        return false;
    }
    let haystack = format!("{}\n{}", result.message, result.logs).to_lowercase();
    haystack.contains("dismiss") || haystack.contains("cancel")
}
