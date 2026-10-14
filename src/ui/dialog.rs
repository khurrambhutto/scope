//! Uninstall/update preview and confirmation dialogs.

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    div, linear_color_stop, linear_gradient, px, rgba, AnyElement, App, ClickEvent, FontWeight,
    Pixels, WeakEntity, Window,
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
        plan: Box<OperationPlan>,
        /// Opt-in removal of `plan.app_data`; always starts unchecked.
        remove_app_data: bool,
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
            OpKind::Install => format!("Install {title}"),
        };

        let body: AnyElement = match dialog {
            Dialog::Loading { kind, .. } => div()
                .p(px(20.))
                .text_color(text_dim())
                .text_size(px(14.))
                .child(match kind {
                    OpKind::Uninstall => "Preparing uninstall preview…",
                    OpKind::Update => "Preparing update preview…",
                    OpKind::Install => "Reading the AppImage…",
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
            Dialog::Confirm {
                kind,
                pkg,
                plan,
                remove_app_data,
            } => confirm_body(entity, *kind, pkg, plan, *remove_app_data),
        };

        modal_shell(heading, body, entity, corner_radius)
    }
}

/// The modal overlay and card chrome shared by every dialog phase.
pub(super) fn modal_shell(
    heading: String,
    body: AnyElement,
    entity: &WeakEntity<ScopeApp>,
    corner_radius: Pixels,
) -> AnyElement {
    modal_shell_styled(heading, body, entity, corner_radius, false)
}

pub(super) fn settings_modal_shell(
    body: AnyElement,
    entity: &WeakEntity<ScopeApp>,
    corner_radius: Pixels,
) -> AnyElement {
    modal_shell_styled("Settings".into(), body, entity, corner_radius, true)
}

fn modal_shell_styled(
    heading: String,
    body: AnyElement,
    entity: &WeakEntity<ScopeApp>,
    corner_radius: Pixels,
    settings: bool,
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
                .when(settings, |this| this.w(px(420.)).rounded(px(12.)))
                .border_1()
                .border_color(border())
                .bg(elev())
                .when(settings, |this| {
                    this.bg(linear_gradient(
                        180.,
                        linear_color_stop(theme::background_top(), 0.),
                        linear_color_stop(theme::background_bottom(), 1.),
                    ))
                })
                .shadow_lg()
                .child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px(px(20.))
                        .py(px(16.))
                        .when(settings, |this| {
                            this.pl(px(20.)).pr(px(16.)).pt(px(18.)).pb(px(14.))
                        })
                        .when(!settings, |this| this.border_b_1().border_color(border()))
                        .child(
                            div()
                                .text_size(px(17.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .when(settings, |this| this.text_size(px(16.)))
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
    remove_app_data: bool,
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

    if let crate::domain::operations::OperationTarget::AppImageInstall { plan: install } =
        &plan.target
    {
        return confirm_actions(
            entity,
            ButtonStyle::Update,
            div().child(plan_rows(install_rows(install, pkg))),
        );
    }

    let mut rows: Vec<(&'static str, String)> = vec![("Package", plan.target.id().to_string())];
    if let Some(scope) = plan.target.install_scope() {
        rows.push(("Scope", scope.id().to_string()));
    }
    match kind {
        OpKind::Uninstall | OpKind::Install => {
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
    if pkg.source != PackageSource::Snap
        && !crate::domain::tool_managers::is_tool_source(pkg.source)
    {
        rows.push(("Size", theme::format_size(pkg.size_bytes)));
    }

    if let crate::domain::operations::OperationTarget::Tool { installation, .. } = &plan.target {
        rows.push(("Installation", installation.root.display().to_string()));
        if let Some(environment) = &installation.environment {
            rows.push(("Environment", environment.display().to_string()));
        }
        if let Some(step) = plan.steps.first() {
            rows.push(("Command", step.command_summary.clone()));
            if kind == OpKind::Update {
                rows.push(("Resolution", step.description.clone()));
            }
        }
        rows.push(("Effects", installation.notes.join("\n\n")));
    }

    if let crate::domain::operations::OperationTarget::AppImage {
        installation: Some(installation),
        ..
    } = &plan.target
    {
        match (kind, &installation.update) {
            (OpKind::Update, Some(update)) => {
                rows.push(("Source", update.source.clone()));
                rows.push(("Download", update.download_url.clone()));
                if let Some(step) = plan.steps.get(1) {
                    rows.push(("Checksum", step.command_summary.clone()));
                }
                rows.push((
                    "Effects",
                    "Replaces the file in place and keeps its launcher settings.".into(),
                ));
            }
            _ => {
                rows.push(("Launchers", paths(&installation.launchers)));
                if !installation.icons.is_empty() {
                    rows.push(("Icons", paths(&installation.icons)));
                }
                rows.push((
                    "Effects",
                    if plan.app_data.is_empty() {
                        "Moves these files to Trash. Portable .home/.config folders are kept, and no app data folders were found by name."
                    } else {
                        "Moves these files to Trash. Portable .home/.config folders are kept, and the app data below is kept unless you select it."
                    }
                    .into(),
                ));
            }
        }
    }

    let confirm_style = match kind {
        OpKind::Uninstall => ButtonStyle::Danger,
        OpKind::Update | OpKind::Install => ButtonStyle::Update,
    };

    confirm_actions(
        entity,
        confirm_style,
        div()
            .child(plan_rows(rows))
            .when_some(plan.snap_details.as_ref(), |this, details| {
                this.child(super::package_details::breakdown(
                    Some(&crate::domain::package_details::PackageDetails::from(
                        details.as_ref(),
                    )),
                    PackageSource::Snap,
                ))
            })
            .when(
                kind == OpKind::Uninstall && !plan.app_data.is_empty(),
                |this| this.child(app_data_choice(entity, &plan.app_data, remove_app_data)),
            ),
    )
}

/// Opt-in list of app data found by name, moved to Trash only when checked.
fn app_data_choice(
    entity: &WeakEntity<ScopeApp>,
    app_data: &[crate::domain::appimage::app_data::AppData],
    checked: bool,
) -> AnyElement {
    let total: u64 = app_data.iter().filter_map(|data| data.bytes).sum();
    let partial = app_data.iter().any(|data| data.bytes.is_none());
    let label = format!(
        "Also move app data to Trash ({}{})",
        theme::format_size(total),
        if partial { "+" } else { "" }
    );
    let entity = entity.clone();
    div()
        .mt(px(14.))
        .pt(px(12.))
        .border_t_1()
        .border_color(border())
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            gpui_kit::component::checkbox::Checkbox::new("dlg-remove-app-data")
                .label(label)
                .checked(checked)
                .on_click(move |checked, _window, cx| {
                    let checked = *checked;
                    entity
                        .update(cx, |this, cx| this.set_remove_app_data(checked, cx))
                        .ok();
                }),
        )
        .child(
            div().text_color(text_dim()).child(plan_rows(
                app_data
                    .iter()
                    .map(|data| {
                        let size = data
                            .bytes
                            .map(theme::format_size)
                            .unwrap_or_else(|| "size unknown".into());
                        (
                            data.kind.label(),
                            format!("{}  ·  {size}", data.path.display()),
                        )
                    })
                    .collect(),
            )),
        )
        .into_any_element()
}

fn paths(paths: &[std::path::PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn install_rows(
    install: &crate::domain::appimage::install::InstallPlan,
    pkg: &InstalledPackage,
) -> Vec<(&'static str, String)> {
    let mut rows = vec![
        ("File", install.source.display().to_string()),
        (
            "Version",
            install.version.clone().unwrap_or_else(|| "—".into()),
        ),
        ("Size", theme::format_size(pkg.size_bytes)),
        ("Install to", install.destination.display().to_string()),
        ("Launcher", install.launcher.display().to_string()),
    ];
    if let Some(icon) = &install.icon {
        rows.push(("Icon", icon.display().to_string()));
    }
    rows.push((
        "Updates",
        install
            .update_source
            .clone()
            .unwrap_or_else(|| "Not available".into()),
    ));
    if !install.notes.is_empty() {
        rows.push(("Notes", install.notes.join("\n\n")));
    }
    rows
}

fn confirm_actions(
    entity: &WeakEntity<ScopeApp>,
    confirm_style: ButtonStyle,
    content: gpui_kit::Div,
) -> AnyElement {
    div()
        .p(px(20.))
        .flex()
        .flex_col()
        .child(content)
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
