//! Filter enums and the header filter bar: search, view toggle, selects, rescan.

use std::path::PathBuf;

use gpui::prelude::*;
use gpui::{div, img, px, AnyElement, Context};

use crate::package::{AppKind, PackageSource};
use crate::theme::{border, elev2};

use super::app_view::{act, ScopeApp};
use super::widgets::{menu_item, select_widget, view_toggle_button};

const ICON_FILTER: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/filter.svg");
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
pub(super) enum KindFilter {
    All,
    Gui,
    Cli,
    Unknown,
}

impl KindFilter {
    pub(super) fn matches(self, kind: AppKind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Gui => kind == AppKind::Gui,
            KindFilter::Cli => kind == AppKind::Cli,
            KindFilter::Unknown => kind == AppKind::Unknown,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OpenSelect {
    Source,
    Kind,
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

        let kind_select = select_widget(
            "kind-select",
            None,
            Some(ICON_FILTER),
            open == Some(OpenSelect::Kind),
            act(&entity, |this, cx| {
                this.open_select = match this.open_select {
                    Some(OpenSelect::Kind) => None,
                    _ => Some(OpenSelect::Kind),
                };
                cx.notify();
            }),
            {
                let entity = entity.clone();
                let current = self.kind_filter;
                move || -> Vec<AnyElement> {
                    let options = [
                        (KindFilter::All, "Any kind"),
                        (KindFilter::Gui, "GUI"),
                        (KindFilter::Cli, "CLI"),
                        (KindFilter::Unknown, "Other"),
                    ];
                    options
                        .into_iter()
                        .enumerate()
                        .map(|(ix, (value, label))| {
                            menu_item(
                                ("kind-item", ix).into(),
                                label,
                                value == current,
                                act(&entity, move |this, cx| {
                                    this.kind_filter = value;
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
            .p(px(8.))
            .rounded(px(12.))
            .border_1()
            .border_color(border())
            .cursor_pointer()
            .opacity(if self.refreshing { 0.5 } else { 1.0 })
            .hover(|this| this.bg(elev2()))
            .on_click(act(&entity, |this, cx| {
                if !this.refreshing {
                    this.start_scan(cx);
                }
            }))
            .child(img(PathBuf::from(ICON_REFRESH)).size(px(16.)).flex_none());

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
            .child(kind_select)
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

    #[test]
    fn kind_filter_matches_only_the_selected_kind() {
        assert!(KindFilter::All.matches(AppKind::Gui));
        assert!(KindFilter::All.matches(AppKind::Unknown));
        assert!(KindFilter::Gui.matches(AppKind::Gui));
        assert!(!KindFilter::Gui.matches(AppKind::Cli));
        assert!(KindFilter::Cli.matches(AppKind::Cli));
        assert!(!KindFilter::Cli.matches(AppKind::Gui));
        assert!(KindFilter::Unknown.matches(AppKind::Unknown));
        assert!(!KindFilter::Unknown.matches(AppKind::Cli));
    }
}
