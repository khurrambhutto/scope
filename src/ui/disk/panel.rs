//! Focus panel beside the rings. It always sits at the rings' level: the list
//! shows what is inside the zoom focus, and the card shows the focus, or a
//! selected file inside it. A back row returns to the parent like the hub.

use std::path::Path;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::*;
use gpui_kit::{
    div, px, relative, transparent_black, AnyElement, App, ClickEvent, FontWeight, Hsla,
    IntoElement, Pixels, WeakEntity, Window,
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
    corner_radius: Pixels,
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

    // Header: back to the parent level on the left, where you are in the
    // middle, and opening the folder on the right.
    let back = focus.parent.map(|_| {
        let entity = entity.clone();
        icon_button("disk-panel-back", IconName::ChevronLeft, move |_, _, cx| {
            entity.update(cx, |this, cx| this.disk_zoom_up(cx)).ok();
        })
    });
    let open_folder = tree.path(id).map(|path| {
        icon_button("disk-open-folder", IconName::FolderOpen, move |_, _, _| {
            backend::spawn_open_folder(path.clone())
        })
    });
    let path = path_label(tree, id);
    let show_path = path != "~" && path != "Whole disk" && path != node.name;
    let header = div()
        .flex()
        .items_center()
        .gap(px(10.))
        .when_some(back, |this, back| this.child(back))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(swatch(color, 10.))
                        .child(
                            div()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(17.))
                                .line_height(px(22.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(node.name.clone()),
                        ),
                )
                .when(show_path, |this| {
                    this.child(
                        div()
                            .truncate()
                            .text_size(px(12.))
                            .text_color(theme::text_faint())
                            .child(path),
                    )
                }),
        )
        .when_some(open_folder, |this, open| this.child(open));

    // Size, share of the parent level, and the share bar read as one block.
    let stats = div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(14.))
        .rounded(px(12.))
        .bg(theme::elev2())
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(26.))
                        .line_height(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(size),
                )
                .child(
                    div()
                        .pb(px(3.))
                        .text_size(px(12.5))
                        .text_color(theme::text_faint())
                        .child(share),
                ),
        )
        .child(
            div()
                .h(px(4.))
                .rounded(px(2.))
                .bg(theme::hover_surface())
                .overflow_hidden()
                .child(div().h_full().rounded(px(2.)).w(relative(width)).bg(color)),
        );

    let open_in_apps = node.owner.clone().map(|owner| {
        let entity = entity.clone();
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
    });

    let card = div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(14.))
        .px(px(16.))
        .pt(px(16.))
        .pb(px(16.))
        .border_b_1()
        .border_color(theme::border())
        .child(header)
        .child(stats)
        .when(!tags.is_empty() || open_in_apps.is_some(), |this| {
            this.child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .children(tags)
                    .when_some(open_in_apps, |this, open| this.child(open)),
            )
        });

    let mut panel = div()
        .flex_none()
        .w(px(PANEL_WIDTH))
        .min_h(px(0.))
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(theme::border())
        .bg(theme::elev())
        // The panel reaches the window's bottom-right corner, and GPUI does
        // not clip children to the window's rounded corners.
        .rounded_br(corner_radius)
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

/// Square icon-only control used in the card header.
fn icon_button(
    id: &'static str,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .flex_none()
        .size(px(32.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(9.))
        .border_1()
        .border_color(theme::border())
        .text_color(theme::text_dim())
        .cursor_pointer()
        .hover(|style| {
            style
                .border_color(theme::border_hover())
                .text_color(theme::text())
        })
        .on_click(on_click)
        .child(Icon::new(icon).size(px(16.)))
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
