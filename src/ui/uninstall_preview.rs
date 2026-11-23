//! The same reviewed uninstall effects and data choices for every source.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight, WeakEntity};

use crate::domain::operations::{OperationPlan, OperationTarget};
use crate::theme;

use super::app_view::ScopeApp;

pub(super) fn body(
    entity: &WeakEntity<ScopeApp>,
    plan: &OperationPlan,
    selected: &[String],
) -> AnyElement {
    let mut content = div().flex().flex_col().gap(px(6.));
    // App files always go with the app, so only effects the user could not
    // otherwise expect are listed: other APT packages and tool notes.
    for effect in side_effects(plan) {
        content = content.child(note(effect).mt(px(10.)));
    }
    let selectable: Vec<_> = plan
        .cleanup
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.selectable())
        .collect();
    if !selectable.is_empty() {
        let chosen: u64 = selectable
            .iter()
            .filter(|(_, item)| selected.contains(&item.id))
            .filter_map(|(_, item)| item.bytes)
            .sum();
        let entity = entity.clone();
        let all = selected.len() == selectable.len();
        content = content.child(
            div()
                .mt(px(14.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    Checkbox::new("select-user-data")
                        .label("Move app data to Trash")
                        .checked(all)
                        .on_click(move |checked, _window, cx| {
                            entity
                                .update(cx, |app, cx| app.keep_app_data(!*checked, cx))
                                .ok();
                        }),
                )
                .when(chosen > 0, |this| {
                    this.child(
                        div()
                            .text_size(px(12.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme::text_dim())
                            .child(theme::format_size(chosen)),
                    )
                }),
        );
    }
    for (index, item) in selectable {
        let id = item.id.clone();
        let entity = entity.clone();
        let checkbox = Checkbox::new(("cleanup-item", index))
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
            item.label.clone(),
            (!item.path.as_os_str().is_empty())
                .then(|| item.location_label())
                .as_deref(),
            size,
            Some(checkbox),
            None,
        ));
    }
    if !plan.cleanup.disclosures.is_empty() {
        content = content.child(
            div()
                .mt(px(10.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(plan.cleanup.disclosures.iter().cloned().map(note)),
        );
    }
    content.into_any_element()
}

fn side_effects(plan: &OperationPlan) -> Vec<String> {
    match &plan.target {
        OperationTarget::Apt { package_id } => {
            plan.steps
                .iter()
                .filter_map(|step| {
                    if let Some(list) = step.description.strip_prefix(
                        "APT removes these packages and their package configuration: ",
                    ) {
                        let others: Vec<_> = list
                            .trim_end_matches('.')
                            .split(", ")
                            .filter(|name| *name != package_id && !name.is_empty())
                            .collect();
                        (!others.is_empty()).then(|| format!("Also removes {}.", others.join(", ")))
                    } else {
                        step.description
                            .starts_with("Additional APT effects")
                            .then(|| step.description.clone())
                    }
                })
                .collect()
        }
        OperationTarget::Tool { installation, .. } => installation.notes.clone(),
        _ => Vec::new(),
    }
}

fn note(text: String) -> gpui_kit::Div {
    div()
        .text_size(px(12.))
        .text_color(theme::text_dim())
        .child(text)
}
