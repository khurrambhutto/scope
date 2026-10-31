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
    let mut content = div().flex().flex_col().gap(px(6.)).mt(px(14.));
    content = content.child(heading("App files"));
    if let Some(details) = &plan.snap_details {
        content = content.child(super::package_details::breakdown(
            Some(&crate::domain::package_details::PackageDetails::from(
                details.as_ref(),
            )),
            crate::domain::package::PackageSource::Snap,
        ));
        content = content.child(
            div()
                .text_size(px(12.))
                .text_color(theme::text_dim())
                .child("Snap may create a recovery snapshot; existing snapshots are kept."),
        );
    } else {
        content = content.child(super::widgets::plan_rows(installation_paths(plan)));
    }
    // APT simulation effects and tool environment effects remain visible;
    // local-file plans are already represented by the path rows above.
    if matches!(
        plan.target,
        OperationTarget::Apt { .. } | OperationTarget::Tool { .. }
    ) {
        for step in plan.steps.iter().skip(usize::from(matches!(
            plan.target,
            OperationTarget::Apt { .. }
        ))) {
            content = content.child(div().text_size(px(12.)).child(step.description.clone()));
        }
    }
    let eligible = plan
        .cleanup
        .items
        .iter()
        .filter(|item| item.selectable())
        .count();
    if eligible > 0 {
        let entity = entity.clone();
        content = content.child(
            div().mt(px(12.)).child(
                Checkbox::new("select-user-data")
                    .label("User data")
                    .checked(selected.len() == eligible)
                    .on_click(move |checked, _window, cx| {
                        entity
                            .update(cx, |app, cx| app.keep_app_data(!*checked, cx))
                            .ok();
                    }),
            ),
        );
    }
    for (index, item) in plan
        .cleanup
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.selectable())
    {
        let id = item.id.clone();
        let entity = entity.clone();
        let checkbox = Checkbox::new(("cleanup-item", index))
            .label(item.label.clone())
            .checked(selected.contains(&item.id))
            .on_click(move |checked, _window, cx| {
                entity
                    .update(cx, |app, cx| app.set_cleanup_item(&id, *checked, cx))
                    .ok();
            })
            .into_any_element();
        let size = if item.path.as_os_str().is_empty() {
            String::new()
        } else {
            item.bytes
                .map(theme::format_size)
                .unwrap_or_else(|| "Unknown".into())
        };
        content = content.child(super::footprint_row::row(
            "",
            (!item.path.as_os_str().is_empty())
                .then(|| item.location_label())
                .as_deref(),
            size,
            Some(checkbox),
            None,
        ));
    }
    let retained = plan
        .cleanup
        .items
        .iter()
        .filter(|item| !item.selectable() && item.effect != CleanupEffect::Included);
    if retained.clone().next().is_some() {
        content = content.child(heading("Kept"));
        for item in retained {
            content = content.child(super::footprint_row::row(
                item.label.clone(),
                Some(&item.location_label()),
                item.bytes
                    .map(theme::format_size)
                    .unwrap_or_else(|| "Unknown".into()),
                None,
                Some(&item.reason),
            ));
        }
    }
    for warning in &plan.cleanup.warnings {
        content = content.child(
            div()
                .mt(px(4.))
                .text_size(px(12.))
                .text_color(theme::text_dim())
                .child(warning.clone()),
        );
    }
    if eligible > 0 {
        content = content.child(div().mt(px(8.)).text_size(px(12.)).text_color(theme::text_dim())
            .child("Selected folders and their contents move to Trash, including saved content. Trash still uses disk space."));
    }
    content.into_any_element()
}

fn installation_paths(plan: &OperationPlan) -> Vec<(&'static str, String)> {
    let mut paths = Vec::new();
    match &plan.target {
        OperationTarget::Apt { package_id } => {
            paths.push(("APT package", package_id.clone()));
        }
        OperationTarget::Flatpak { app_id, .. } => {
            paths.push(("Flatpak app", app_id.clone()));
        }
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
