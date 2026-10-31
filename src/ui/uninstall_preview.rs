//! The same reviewed uninstall effects and data choices for every source.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight, WeakEntity};

use crate::domain::app_data::CleanupEffect;
use crate::domain::operations::{OperationPlan, OperationTarget};
use crate::theme;

use super::app_view::ScopeApp;

pub(super) fn body(
    entity: &WeakEntity<ScopeApp>,
    plan: &OperationPlan,
    selected: &[String],
) -> AnyElement {
    let mut content = div().flex().flex_col().gap(px(10.)).mt(px(14.));
    content = content
        .child(heading("App files"))
        .child(super::widgets::plan_rows(installation_paths(plan)));
    for step in &plan.steps {
        content = content.child(div().text_size(px(12.)).child(step.description.clone()));
    }
    let eligible = plan.cleanup.items.iter().any(|item| item.selectable());
    if eligible {
        let entity = entity.clone();
        content = content.child(heading("App data")).child(
            Checkbox::new("keep-app-data")
                .label("Keep app data")
                .checked(selected.is_empty())
                .on_click(move |checked, _window, cx| {
                    entity
                        .update(cx, |app, cx| app.keep_app_data(*checked, cx))
                        .ok();
                }),
        );
    }
    for automatic in [true, false] {
        if !automatic
            && plan
                .cleanup
                .items
                .iter()
                .any(|item| item.selectable() && !item.default_selected)
        {
            content = content.child(heading("Review separately"));
        }
        for (index, item) in plan
            .cleanup
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.selectable() && item.default_selected == automatic)
        {
            let id = item.id.clone();
            let entity = entity.clone();
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.))
                    .py(px(theme::DETAIL_ROW_PADDING))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(theme::DETAIL_ROW_GAP))
                            .child(
                                Checkbox::new(("cleanup-item", index))
                                    .label(item.label.clone())
                                    .checked(selected.contains(&item.id))
                                    .on_click(move |checked, _window, cx| {
                                        entity
                                            .update(cx, |app, cx| {
                                                app.set_cleanup_item(&id, *checked, cx)
                                            })
                                            .ok();
                                    }),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(12.))
                                    .text_color(theme::text_dim())
                                    .child(if item.path.as_os_str().is_empty() {
                                        String::new()
                                    } else {
                                        item.bytes
                                            .map(theme::format_size)
                                            .unwrap_or_else(|| "size unknown".into())
                                    }),
                            ),
                    )
                    .when(!item.path.as_os_str().is_empty(), |this| {
                        this.child(
                            div()
                                .pl(px(24.))
                                .text_size(px(11.))
                                .text_color(theme::text_dim())
                                .child(item.location_label()),
                        )
                    })
                    .child(
                        div()
                            .pl(px(24.))
                            .text_size(px(11.))
                            .text_color(theme::text_dim())
                            .child(item.reason.clone()),
                    ),
            );
        }
    }
    let retained: Vec<_> = plan
        .cleanup
        .items
        .iter()
        .filter(|item| !item.selectable() && item.effect != CleanupEffect::Included)
        .collect();
    if !retained.is_empty() || !plan.cleanup.warnings.is_empty() {
        content = content.child(heading("Kept / cannot verify"));
        for item in retained {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(theme::text_dim())
                    .child(format!("{} — {}", item.location_label(), item.reason)),
            );
        }
        for warning in &plan.cleanup.warnings {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .text_color(theme::text_dim())
                    .child(warning.clone()),
            );
        }
    }
    content.child(div().mt(px(6.)).text_size(px(11.)).text_color(theme::text_dim())
        .child("Files inside selected folders move to Trash, including any profiles, history, credentials or saved content there. Documents outside those folders are kept. Trash continues to use disk space."))
        .into_any_element()
}

fn installation_paths(plan: &OperationPlan) -> Vec<(&'static str, String)> {
    let mut paths = Vec::new();
    match &plan.target {
        OperationTarget::AppImage {
            installation: Some(app),
            ..
        } => {
            paths.push(("AppImage", app.path.display().to_string()));
            paths.extend(
                app.launchers
                    .iter()
                    .map(|path| ("Launcher", path.display().to_string())),
            );
            paths.extend(
                app.icons
                    .iter()
                    .map(|path| ("Icon", path.display().to_string())),
            );
        }
        OperationTarget::Extracted { installation } => {
            paths.push(("Installation", installation.root.display().to_string()));
            paths.extend(
                installation
                    .launchers
                    .iter()
                    .map(|path| ("Launcher", path.display().to_string())),
            );
        }
        OperationTarget::Tool { installation, .. } => {
            paths.push(("Installation", installation.root.display().to_string()));
            if let Some(environment) = &installation.environment {
                paths.push(("Environment", environment.display().to_string()));
            }
            if let Some(step) = plan.steps.first() {
                paths.push(("Command", step.command_summary.clone()));
            }
        }
        _ => {}
    }
    paths
}

fn heading(label: &'static str) -> gpui_kit::Div {
    div()
        .mt(px(6.))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(label)
}
