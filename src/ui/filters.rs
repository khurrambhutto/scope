//! Filter enums and the header filter bar: search, view toggle, selects, rescan.

use gpui_kit::prelude::*;
use gpui_kit::{div, img, px, rgba, AnyElement, Context, ImageSource, Resource};

use crate::domain::package::PackageSource;
use crate::theme::{accent, border};

use super::app_view::{act, ScopeApp};
use super::widgets::{menu_item, select_widget, view_toggle_button};

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
}

impl SourceFilter {
    pub(super) fn label(self) -> &'static str {
        match self {
            SourceFilter::All => "Any source",
            SourceFilter::Apt => "APT",
            SourceFilter::Snap => "Snap",
            SourceFilter::Flatpak => "Flatpak",
            SourceFilter::AppImage => "AppImage",
        }
    }

    pub(super) fn matches(self, source: PackageSource) -> bool {
        match self {
            SourceFilter::All => true,
            SourceFilter::Apt => source == PackageSource::Apt,
            SourceFilter::Snap => source == PackageSource::Snap,
            SourceFilter::Flatpak => source == PackageSource::Flatpak,
            SourceFilter::AppImage => source == PackageSource::AppImage,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenSelect {
    Source,
}

impl ScopeApp {
    pub(super) fn filters(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().downgrade();
        let open = self.open_select;

        let view_toggle = div()
            .flex_none()
            .flex()
            .items_center()
            .gap(px(2.))
            .p(px(2.))
            .rounded_full()
            .border_1()
            .border_color(border())
            .hover(|this| this.border_color(accent()))
            .child(
                view_toggle_button(
                    "view-uninstall",
                    "Uninstall",
                    self.view_mode == ViewMode::Uninstall,
                    act(&entity, |this, cx| {
                        this.view_mode = ViewMode::Uninstall;
                        cx.notify();
                    }),
                ),
            )
            .child(view_toggle_button(
                "view-updates",
                "Updates",
                self.view_mode == ViewMode::Updates,
                act(&entity, |this, cx| {
                    this.view_mode = ViewMode::Updates;
                    cx.notify();
                }),
            ));

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
            .child(self.search_input.clone())
            .child(view_toggle)
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
        assert!(SourceFilter::Apt.matches(PackageSource::Apt));
        assert!(!SourceFilter::Apt.matches(PackageSource::Snap));
        assert!(SourceFilter::Snap.matches(PackageSource::Snap));
        assert!(!SourceFilter::Snap.matches(PackageSource::Flatpak));
        assert!(SourceFilter::Flatpak.matches(PackageSource::Flatpak));
        assert!(!SourceFilter::Flatpak.matches(PackageSource::Apt));
        assert!(SourceFilter::AppImage.matches(PackageSource::AppImage));
        assert!(!SourceFilter::AppImage.matches(PackageSource::Snap));
    }
}
