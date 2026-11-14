//! Focus panel beside the rings. It always sits at the rings' level: the list
//! shows what is inside the zoom focus, and the card shows the focus, or a
//! selected file inside it. A back row returns to the parent like the hub.

use std::path::Path;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, relative, transparent_black, AnyElement, FontWeight, Hsla, IntoElement, WeakEntity,
};

use super::render::{clip, disk_total, fraction, percent, size_text};
use super::view::DiskState;
use crate::backend;
use crate::domain::disk_usage::{DiskTree, NodeId, NodeKind};
use crate::theme;
use crate::ui::app_view::ScopeApp;
use crate::ui::widgets::{button, ButtonStyle};

const PANEL_WIDTH: f32 = 372.;

pub(super) fn focus_panel(
    state: &DiskState,
    tree: &DiskTree,
    entity: &WeakEntity<ScopeApp>,
) -> impl IntoElement {
    let id = state.card_node(tree);
    let node = tree.node(id);
    let focus = tree.node(state.focus);
    let partial = state.scanning;
    let total = disk_total(state);

    let (share, width) = match node.parent.map(|parent| tree.node(parent)) {
        Some(parent) => (
            format!(
                "{} of {}",
                percent(node.bytes, parent.bytes),
                clip(&parent.name, 24)
            ),
            fraction(node.bytes, parent.bytes),
        ),
        None => (
            format!("{} of disk", percent(node.bytes, total)),
            fraction(node.bytes, total),
        ),
    };
    let color = theme::disk_category_color(node.category);
    let size = if matches!(node.kind, NodeKind::Unreadable) {
        "Unavailable".to_string()
    } else {
        size_text(node.bytes, partial)
    };

    let mut tags: Vec<AnyElement> = Vec::new();
    if let Some(owner) = &node.owner {
        tags.push(tag(owner.clone()));
    }
    if node.rebuildable {
        tags.push(tag("Rebuildable".to_string()));
    }
    if node.read_only {
        tags.push(tag("Read-only".to_string()));
    }

    let mut actions: Vec<AnyElement> = Vec::new();
    if let Some(owner) = node.owner.clone() {
        let entity = entity.clone();
        actions.push(
            button(
                "disk-open-apps",
                "Open in Apps",
                ButtonStyle::Neutral,
                move |_, window, cx| {
                    let owner = owner.clone();
                    entity
                        .update(cx, |this, cx| this.open_in_apps(&owner, window, cx))
                        .ok();
                },
            )
            .icon(IconName::LayoutDashboard)
            .into_any_element(),
        );
    }
    if let Some(path) = tree.path(id) {
        actions.push(
            button(
                "disk-open-folder",
                "Open folder",
                ButtonStyle::Neutral,
                move |_, _, _| backend::spawn_open_folder(path.clone()),
            )
            .icon(IconName::FolderOpen)
            .into_any_element(),
        );
    }

    let back = focus.parent.map(|parent| {
        let entity = entity.clone();
        div()
            .id("disk-panel-back")
            .flex()
            .items_center()
            .gap(px(4.))
            .ml(px(-4.))
            .px(px(4.))
            .py(px(2.))
            .rounded(px(6.))
            .self_start()
            .cursor_pointer()
            .text_size(px(12.5))
            .text_color(theme::text_faint())
            .hover(|style| style.text_color(theme::text()).bg(theme::hover_surface()))
            .on_click(move |_, _, cx| {
                entity.update(cx, |this, cx| this.disk_zoom_up(cx)).ok();
            })
            .child(Icon::new(IconName::ChevronLeft).size(px(14.)))
            .child(clip(&tree.node(parent).name, 32))
    });

    let card = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(14.))
        .px(px(20.))
        .pt(px(if back.is_some() { 14. } else { 20. }))
        .pb(px(18.))
        .border_b_1()
        .border_color(theme::border())
        .when_some(back, |this, back| this.child(back))
        .child(
            div()
                .flex()
                .items_start()
                .gap(px(12.))
                .child(
                    div()
                        .flex_none()
                        .mt(px(6.))
                        .size(px(12.))
                        .rounded(px(4.))
                        .bg(color),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .text_size(px(18.))
                                .line_height(px(24.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(node.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme::text_faint())
                                .child(path_label(tree, id)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(size),
                )
                .child(
                    div()
                        .text_size(px(12.5))
                        .text_color(theme::text_faint())
                        .child(share),
                ),
        )
        .child(
            div()
                .h(px(6.))
                .rounded(px(3.))
                .bg(theme::hover_surface())
                .overflow_hidden()
                .child(div().h_full().rounded(px(3.)).w(relative(width)).bg(color)),
        )
        .when(!tags.is_empty(), |this| {
            this.child(div().flex().flex_wrap().gap(px(6.)).children(tags))
        })
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .children(actions),
        );

    let mut panel = div()
        .flex_none()
        .w(px(PANEL_WIDTH))
        .min_h(px(0.))
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(theme::border())
        .bg(theme::elev())
        .child(card);

    // The list always shows the rings' level: what is inside the zoom focus.
    if !focus.children.is_empty() {
        let list = focus;
        let title = "Inside".to_string();
        let rows = list.children.iter().map(|&child| {
            row(
                tree,
                child,
                list.bytes,
                state.selected == Some(child),
                partial,
                entity,
            )
        });
        panel = panel.child(
            div()
                .id("disk-inside")
                .flex_1()
                .min_h(px(0.))
                .overflow_y_scroll()
                .px(px(10.))
                .py(px(14.))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .px(px(10.))
                        .pb(px(8.))
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::text_faint())
                        .child(title)
                        .child(
                            div()
                                .font_weight(FontWeight::NORMAL)
                                .child(format!("{} items", list.children.len())),
                        ),
                )
                .children(rows),
        );
    }
    panel
}

/// One child of the listed node. Folded and unreadable entries are shown but
/// are not interactive.
fn row(
    tree: &DiskTree,
    child: NodeId,
    list_bytes: u64,
    selected: bool,
    partial: bool,
    entity: &WeakEntity<ScopeApp>,
) -> AnyElement {
    let node = tree.node(child);
    let clickable = matches!(node.kind, NodeKind::Directory | NodeKind::File);
    let color = theme::disk_category_color(node.category);
    let size = if matches!(node.kind, NodeKind::Unreadable) {
        "Unavailable".to_string()
    } else {
        size_text(node.bytes, partial)
    };

    let mut name = div()
        .flex()
        .flex_1()
        .min_w(px(0.))
        .items_center()
        .gap(px(9.))
        .child(swatch(color, 8.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(node.name.clone()),
        );
    if let Some(owner) = &node.owner {
        name = name.child(
            div()
                .flex_none()
                .text_size(px(11.))
                .text_color(theme::text_faint())
                .child(owner.clone()),
        );
    }
    if !node.children.is_empty() {
        name = name.child(
            div().flex_none().child(
                Icon::new(IconName::ChevronRight)
                    .size(px(14.))
                    .text_color(theme::text_faint()),
            ),
        );
    }

    let row = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(7.))
        .px(px(10.))
        .pt(px(9.))
        .pb(px(10.))
        .rounded(px(10.))
        .border_1()
        .border_color(if selected {
            theme::border()
        } else {
            transparent_black()
        })
        .bg(if selected {
            theme::hover_surface()
        } else {
            transparent_black()
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(name)
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_color(if clickable {
                            theme::text_dim()
                        } else {
                            theme::text_faint()
                        })
                        .child(size),
                ),
        )
        .child(
            div()
                .h(px(3.))
                .rounded(px(2.))
                .bg(theme::hover_surface())
                .overflow_hidden()
                .child(
                    div()
                        .h_full()
                        .rounded(px(2.))
                        .w(relative(fraction(node.bytes, list_bytes).max(0.015)))
                        .bg(color),
                ),
        );
    if !clickable {
        return row.into_any_element();
    }

    let click_entity = entity.clone();
    let hover_entity = entity.clone();
    row.id(("disk-row", child as usize))
        .cursor_pointer()
        .hover(|style| style.bg(theme::elev2()))
        .on_click(move |_, _, cx| {
            click_entity
                .update(cx, |this, cx| this.disk_activate(child, cx))
                .ok();
        })
        .on_hover(move |inside: &bool, _, cx| {
            let hover = inside.then_some(child);
            hover_entity
                .update(cx, |this, cx| this.disk_hover(hover, cx))
                .ok();
        })
        .into_any_element()
}

fn swatch(color: Hsla, size: f32) -> impl IntoElement {
    div().flex_none().size(px(size)).rounded(px(3.)).bg(color)
}

fn tag(label: String) -> AnyElement {
    div()
        .flex_none()
        .whitespace_nowrap()
        .px(px(9.))
        .py(px(3.))
        .rounded_full()
        .border_1()
        .border_color(theme::border())
        .bg(theme::elev())
        .text_size(px(11.5))
        .text_color(theme::text_faint())
        .child(label)
        .into_any_element()
}

/// Real path of a node with the home folder shortened to `~`. Folded entries
/// show their parent's folder. The root reads "Whole disk".
fn path_label(tree: &DiskTree, id: NodeId) -> String {
    if id == tree.root {
        return "Whole disk".to_string();
    }
    let path = tree
        .path(id)
        .or_else(|| tree.node(id).parent.and_then(|parent| tree.path(parent)));
    path.map(|path| tilde(&path, &tree.home_path))
        .unwrap_or_default()
}

fn tilde(path: &Path, home: &Path) -> String {
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Ok(rest) => format!("~/{}", rest.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_is_shortened_and_other_paths_are_kept() {
        let home = Path::new("/home/tester");
        assert_eq!(tilde(Path::new("/home/tester"), home), "~");
        assert_eq!(
            tilde(Path::new("/home/tester/code/app"), home),
            "~/code/app"
        );
        assert_eq!(tilde(Path::new("/usr/lib"), home), "/usr/lib");
    }
}
