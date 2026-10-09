//! Uninstall/update preview and confirmation dialogs.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, rgba, AnyElement, App, ClickEvent, FontWeight, Pixels, WeakEntity, Window,
};

use crate::backend::OpKind;
use crate::domain::operations::OperationPlan;
use crate::domain::package::{InstalledPackage, PackageSource};
use crate::theme::{self, border, display_title, elev, text_dim};

use super::app_view::{act, ScopeApp};
use super::widgets::{banner, button, dialog_actions, plan_rows, BannerKind, ButtonStyle};

// ---- Operation dialog ------------------------------------------------------

/// Preview and confirmation only; apply progress lives in the operation footer.
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
}

impl Dialog {
    pub(super) fn kind(&self) -> OpKind {
        match self {
            Dialog::Loading { kind, .. }
            | Dialog::Failed { kind, .. }
            | Dialog::Confirm { kind, .. } => *kind,
        }
    }

    pub(super) fn pkg(&self) -> &InstalledPackage {
        match self {
            Dialog::Loading { pkg, .. }
            | Dialog::Failed { pkg, .. }
            | Dialog::Confirm { pkg, .. } => pkg,
        }
    }
}

impl ScopeApp {
    pub(super) fn dialog_element(
        &self,
        entity: &WeakEntity<ScopeApp>,
        corner_radius: Pixels,
    ) -> AnyElement {
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
        };

        modal_shell(heading, body, entity, corner_radius)
    }
}

/// The modal overlay and card chrome shared by every dialog phase.
fn modal_shell(
    heading: String,
    body: AnyElement,
    entity: &WeakEntity<ScopeApp>,
    corner_radius: Pixels,
) -> AnyElement {
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
        .rounded(corner_radius)
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
                            Button::new("modal-close")
                                .ghost()
                                .icon(IconName::Close)
                                .accessibility_label("Close dialog")
                                .size(px(30.))
                                .rounded(px(16.))
                                .on_click(act(entity, |this, cx| this.close_dialog(cx))),
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
    if pkg.source != PackageSource::Snap {
        rows.push(("Size", theme::format_size(pkg.size_bytes)));
    }

    let confirm_style = match kind {
        OpKind::Uninstall => ButtonStyle::Danger,
        OpKind::Update => ButtonStyle::Update,
    };

    div()
        .p(px(20.))
        .child(plan_rows(rows))
        .when_some(plan.snap_details.as_ref(), |this, details| {
            this.child(super::snap_details::breakdown(Some(details)))
        })
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
