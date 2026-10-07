//! Filter enums and the header filter bar: search, view toggle, selects, rescan.

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{
        InputGroup, InputGroupAddon, InputGroupAddonAlignment, InputGroupInput, InputGroupText,
    },
    FocusableExt as _, Icon,
};
use gpui_kit::prelude::*;
use gpui_kit::{div, img, px, rems, rgba, AnyElement, Context, ImageSource, Resource};

use crate::domain::package::PackageSource;
use crate::theme::{accent, border, elev, elev2, text, text_dim, text_faint};

use super::app_view::{act, ScopeApp};
use super::widgets::{menu_item, select_widget};

const ICON_REFRESH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/refresh.svg");

// ---- Filters ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ViewMode {
    Uninstall,
    Updates,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceFilter {
    All,
    Apt,
    Snap,
    Flatpak,
    AppImage,
    Desktop,
}

impl SourceFilter {
    pub(super) fn label(self) -> &'static str {
        match self {
            SourceFilter::All => "Any source",
            SourceFilter::Apt => "APT",
            SourceFilter::Snap => "Snap",
            SourceFilter::Flatpak => "Flatpak",
            SourceFilter::AppImage => "AppImage",
            SourceFilter::Desktop => "Desktop",
        }
    }

    pub(super) fn matches(self, source: PackageSource) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Apt => source == PackageSource::Apt,
            SourceFilter::Snap => source == PackageSource::Snap,
            SourceFilter::Flatpak => source == PackageSource::Flatpak,
            SourceFilter::AppImage => source == PackageSource::AppImage,
            SourceFilter::Desktop => source == PackageSource::Desktop,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenSelect {
    Source,
}

impl ScopeApp {
    pub(super) fn view_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(3.))
            .rounded_full()
            .border_1()
            .border_color(border())
            .bg(elev2())
            .children(
                [
                    (
                        "view-apps",
                        "Apps",
                        IconName::LayoutDashboard,
                        ViewMode::Uninstall,
                    ),
                    (
                        "view-updates",
                        "Updates",
                        IconName::RefreshCw,
                        ViewMode::Updates,
                    ),
                ]
                .into_iter()
                .map(|(id, label, icon, mode)| {
                    let active = self.view_mode == mode;
                    Button::new(id)
                        .label(label)
                        .icon(icon)
                        .rounded(px(18.))
                        .h(px(30.))
                        .px(px(16.))
                        .text_size(px(13.))
                        .custom(
                            ButtonCustomVariant::new(cx)
                                .color(if active { elev() } else { elev2() })
                                .foreground(if active { accent() } else { text_dim() })
                                .hover(elev())
                                .active(elev()),
                        )
                        .toggled(active)
                        .on_click(act(&entity, move |this, cx| {
                            this.view_mode = mode;
                            this.open_select = None;
                            cx.notify();
                        }))
                }),
            )
    }

    /// Kit search field: magnifier prefix, cleanable input, live result
    /// count suffix. The query state lives in [`ScopeApp::search_input`];
    /// typing emits `InputEvent::Change`, which re-renders the list.
    fn search_box(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let shown = self.packages.entries.len();
        let count = format!("{shown} result{}", if shown == 1 { "" } else { "s" });
        InputGroup::new("scope-search")
            .focus_ring(false)
            .max_w(rems(20.))
            // Left edge lines up with the list icons below: filter bar
            // padding (32) + row padding (12).
            .ml(px(12.))
            .h(px(32.))
            .rounded_full()
            .bg(elev())
            .border_color(border())
            .text_color(text())
            .input(
                InputGroupInput::new(&self.search_input)
                    .aria_label("Search installed items")
                    .cleanable(true)
                    .text_size(px(13.))
                    .text_color(text()),
            )
            .addon(
                InputGroupAddon::new("scope-search-icon")
                    .child(Icon::new(IconName::Search).size_4().text_color(text_dim())),
            )
            .addon(
                InputGroupAddon::new("scope-search-count")
                    .align(InputGroupAddonAlignment::InlineEnd)
                    .child(InputGroupText::new().text_color(text_faint()).child(count)),
            )
    }

    pub(super) fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let open = self.open_select;

        let source_select = select_widget(
            "source-select",
            Some(self.source_filter.label()),
            None,
            open == Some(OpenSelect::Source),
            act(&entity, |this, cx| {
                this.open_select = match this.open_select {
                    Some(OpenSelect::Source) => None,
                    _ => Some(OpenSelect::Source),
                };
                cx.notify();
            }),
            {
                let entity = entity.clone();
                let current = self.source_filter;
                move || -> Vec<AnyElement> {
                    let options = [
                        (SourceFilter::All, "Any source"),
                        (SourceFilter::Apt, "APT"),
                        (SourceFilter::Snap, "Snap"),
                        (SourceFilter::Flatpak, "Flatpak"),
                        (SourceFilter::AppImage, "AppImage"),
                        (SourceFilter::Desktop, "Desktop"),
                    ];
                    options
                        .into_iter()
                        .enumerate()
                        .map(|(ix, (value, label))| {
                            menu_item(
                                ("source-item", ix).into(),
                                label,
                                value == current,
                                act(&entity, move |this, cx| {
                                    this.source_filter = value;
                                    this.open_select = None;
                                    cx.notify();
                                }),
                            )
                            .into_any_element()
                        })
                        .collect()
                }
            },
        );

        let rescan = div()
            .id("rescan")
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .size(px(34.))
            .rounded_full()
            .border_1()
            .border_color(border())
            .cursor_pointer()
            .opacity(if self.refreshing { 0.5 } else { 1.0 })
            .hover(|this| this.bg(rgba(0xffffff08)).border_color(accent()))
            .on_click(act(&entity, |this, cx| {
                if !this.refreshing {
                    this.start_scan(cx);
                }
            }))
            .child(
                img(ImageSource::Resource(Resource::Embedded(
                    ICON_REFRESH.into(),
                )))
                .size(px(14.))
                .flex_none(),
            );

        div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(32.))
            .pr(px(16.))
            .py(px(12.))
            .child(self.search_box(cx))
            .child(div().flex_1())
            .child(source_select)
            .child(rescan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_filter_matches_only_the_selected_source() {
        assert!(SourceFilter::All.matches(PackageSource::Apt));
        assert!(SourceFilter::All.matches(PackageSource::AppImage));
        assert!(SourceFilter::All.matches(PackageSource::Desktop));
        assert!(SourceFilter::Apt.matches(PackageSource::Apt));
        assert!(!SourceFilter::Apt.matches(PackageSource::Snap));
        assert!(SourceFilter::Snap.matches(PackageSource::Snap));
        assert!(!SourceFilter::Snap.matches(PackageSource::Flatpak));
        assert!(SourceFilter::Flatpak.matches(PackageSource::Flatpak));
        assert!(!SourceFilter::Flatpak.matches(PackageSource::Apt));
        assert!(SourceFilter::AppImage.matches(PackageSource::AppImage));
        assert!(!SourceFilter::AppImage.matches(PackageSource::Snap));
        assert!(SourceFilter::Desktop.matches(PackageSource::Desktop));
        assert!(!SourceFilter::Desktop.matches(PackageSource::Apt));
    }
}
