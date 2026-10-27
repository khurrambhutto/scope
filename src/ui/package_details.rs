//! Shared lazy inspection and source-aware type/path/size breakdown.

use futures::{channel::mpsc, StreamExt};
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, Context, FontWeight};

use crate::backend;
use crate::domain::package::PackageSource;
use crate::domain::package_details::{DetailEffect, DetailItem, PackageDetails};
use crate::theme;

use super::app_view::ScopeApp;

impl ScopeApp {
    pub(super) fn load_package_details(&mut self, cx: &mut Context<Self>) {
        let pkg = self
            .selected_key
            .as_ref()
            .and_then(|key| {
                self.scan
                    .as_ref()?
                    .packages
                    .iter()
                    .find(|pkg| &pkg.key == key)
            })
            .cloned();
        let Some(pkg) = pkg else {
            self.package_details.cancel();
            return;
        };
        let Some(request) = self.package_details.begin(&pkg) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            if request.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let (tx, mut rx) = mpsc::unbounded();
            backend::spawn_package_details(pkg.clone(), request.cancel, tx);
            while let Some(update) = rx.next().await {
                this.update(cx, |this, cx| {
                    if this.package_details.accept(
                        request.generation,
                        &pkg,
                        update.details,
                        update.complete,
                    ) {
                        if let Some(index) = this
                            .packages
                            .entries
                            .iter()
                            .position(|entry| entry.key == pkg.key)
                        {
                            this.list_state.splice(index..index + 1, 1);
                        }
                        cx.notify();
                    }
                })
                .ok();
            }
        })
        .detach();
    }
}

/// Compact type/path/size rows. Markers describe effects and are not controls.
pub(super) fn breakdown(details: Option<&PackageDetails>, source: PackageSource) -> AnyElement {
    let mut content = div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .text_color(theme::text());
    if let Some(details) = details {
        for (effect, heading) in [
            (DetailEffect::Removed, ""),
            (DetailEffect::Kept, "Kept after uninstall"),
            (DetailEffect::Unverified, "Needs review"),
            (DetailEffect::Informational, ""),
        ] {
            if details.items.iter().any(|item| item.effect == effect) {
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .when(!heading.is_empty(), |this| {
                            this.child(
                                div()
                                    .flex()
                                    .items_center()
                                    .pb(px(6.))
                                    .text_size(px(13.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(heading),
                            )
                        })
                        .children(
                            details
                                .items
                                .iter()
                                .filter(|item| item.effect == effect)
                                .map(detail_row),
                        ),
                );
            }
        }
        for warning in &details.warnings {
            content = content.child(
                div()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(theme::text_dim())
                    .child(warning.clone()),
            );
        }
    } else {
        content = content.child(loading_rows(source));
    }
    content.into_any_element()
}

fn loading_rows(source: PackageSource) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(3.))
        .children(loading_labels(source).iter().copied().map(|label| {
            div()
                .flex()
                .items_center()
                .gap(px(theme::DETAIL_ROW_GAP))
                .py(px(theme::DETAIL_ROW_PADDING))
                .text_size(px(13.))
                .line_height(px(18.))
                .child(
                    div()
                        .flex_none()
                        .w(px(theme::DETAIL_LABEL_WIDTH))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(
                    div().flex_1().min_w(px(0.)).child(
                        div()
                            .h(px(8.))
                            .w_full()
                            .max_w(px(300.))
                            .rounded(px(4.))
                            .bg(theme::border()),
                    ),
                )
                .child(
                    div()
                        .flex_none()
                        .w(px(80.))
                        .text_right()
                        .text_size(px(12.))
                        .text_color(theme::text_dim())
                        .child("…"),
                )
        }))
        .into_any_element()
}

/// Compact estimate of managed items, not net disk space freed.
pub(super) fn removal_summary(details: Option<&PackageDetails>) -> String {
    let Some(details) = details else {
        return "Measuring removal…".into();
    };
    let size = details.removal_size();
    match size.bytes {
        Some(bytes) => format!(
            "Removes ~{}{}",
            measured_size(bytes),
            if size.complete { "" } else { "+" }
        ),
        None => "Removal size unknown".into(),
    }
}

fn detail_row(item: &DetailItem) -> AnyElement {
    let pending = item.size.note.as_deref() == Some(crate::domain::package_details::MEASURING_NOTE);
    let size = size_label(&item.size);
    let icon = match item.effect {
        DetailEffect::Removed | DetailEffect::Informational => None,
        DetailEffect::Kept => Some(IconName::Minus),
        DetailEffect::Unverified => Some(IconName::CircleAlert),
    };
    let mut path = div().flex_1().min_w(px(0.)).flex().flex_col();
    if let Some(location) = &item.location {
        path = path.child(
            div()
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(theme::text_dim())
                .child(location.clone()),
        );
    }
    let note = item.size.note.as_ref().filter(|_| {
        !pending
            && (!item.size.complete
                || item.size.status
                    == crate::domain::package_details::MeasurementStatus::SharedStorage)
    });
    if let Some(note) = note {
        path = path.child(
            div()
                .text_size(px(11.))
                .line_height(px(16.))
                .text_color(theme::text_dim())
                .child(note.clone()),
        );
    } else if item.effect == DetailEffect::Unverified {
        path = path.child(
            div()
                .text_size(px(11.))
                .line_height(px(16.))
                .text_color(theme::text_dim())
                .child("Removal coverage is not verified"),
        );
    }
    div()
        .flex()
        .items_start()
        .gap(px(theme::DETAIL_ROW_GAP))
        .py(px(theme::DETAIL_ROW_PADDING))
        .text_size(px(13.))
        .line_height(px(18.))
        .child(
            div()
                .flex_none()
                .w(px(theme::DETAIL_LABEL_WIDTH))
                .flex()
                .items_center()
                .gap(px(9.))
                .children(icon.map(|icon| {
                    Icon::new(icon)
                        .size(px(12.))
                        .text_color(theme::text_faint())
                }))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(item.label.clone()),
                ),
        )
        .child(path)
        .child(
            div()
                .flex_none()
                .w(px(80.))
                .text_right()
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme::text_dim())
                .child(size),
        )
        .into_any_element()
}

fn size_label(size: &crate::domain::package_details::Measurement) -> String {
    use crate::domain::package_details::MeasurementStatus;
    match size.status {
        MeasurementStatus::Included => "Included".into(),
        MeasurementStatus::NotMeasured => "Not measured".into(),
        MeasurementStatus::Unattributed => "Unattributed".into(),
        MeasurementStatus::Size | MeasurementStatus::SharedStorage => match size.bytes {
            Some(bytes) if size.complete => measured_size(bytes),
            Some(bytes) => format!("{}+", measured_size(bytes)),
            None if size.note.as_deref()
                == Some(crate::domain::package_details::MEASURING_NOTE) =>
            {
                "Measuring…".into()
            }
            None => "Unavailable".into(),
        },
    }
}

fn measured_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".into()
    } else {
        theme::format_size(bytes)
    }
}

fn loading_labels(source: PackageSource) -> &'static [&'static str] {
    match source {
        PackageSource::Uv | PackageSource::Pipx => {
            &["Environment", "Commands", "Shared data", "User data"]
        }
        PackageSource::Cargo => &["Binaries", "Shared data", "User data"],
        PackageSource::Npm | PackageSource::Bun => &[
            "Package",
            "Commands",
            "Shared dependencies",
            "Shared data",
            "User data",
        ],
        PackageSource::Snap => &["Application", "Older revisions", "User data", "System data"],
        PackageSource::Apt => &["Application", "Configuration", "User data"],
        PackageSource::Flatpak => &["Application", "User data"],
        PackageSource::AppImage => &["AppImage", "User data"],
        PackageSource::Desktop => &["Launcher", "Executable", "User data"],
        PackageSource::Steam => &["Game files", "User data"],
        PackageSource::JetBrainsToolbox => &["IDE files", "User data"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::package_details::{Measurement, MeasurementStatus};

    #[test]
    fn size_labels_distinguish_included_kept_unattributed_and_unavailable() {
        for (status, label) in [
            (MeasurementStatus::Included, "Included"),
            (MeasurementStatus::NotMeasured, "Not measured"),
            (MeasurementStatus::Unattributed, "Unattributed"),
        ] {
            assert_eq!(
                size_label(&Measurement::descriptive(status, "explanation")),
                label
            );
        }
        assert_eq!(
            size_label(&Measurement::unknown("Could not read files")),
            "Unavailable"
        );
        assert_eq!(
            size_label(&Measurement::unknown(
                crate::domain::package_details::MEASURING_NOTE
            )),
            "Measuring…"
        );
        assert_eq!(
            size_label(&Measurement {
                status: MeasurementStatus::Size,
                bytes: Some(0),
                complete: true,
                note: None
            }),
            "0 B"
        );
        assert!(size_label(&Measurement {
            status: MeasurementStatus::Size,
            bytes: Some(1024),
            complete: false,
            note: None
        })
        .ends_with('+'));
    }
}
