//! Lazy inspection wiring and the Snap removal breakdown shared with previews.

use futures::channel::oneshot;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, Context, FontWeight};

use crate::backend;
use crate::domain::package::PackageSource;
use crate::domain::snap_details::{DetailItem, RemovalEffect, SnapDetails};
use crate::theme;

use super::app_view::ScopeApp;

impl ScopeApp {
    pub(super) fn load_snap_details(&mut self, cx: &mut Context<Self>) {
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
        let Some(pkg) = pkg.filter(|pkg| pkg.source == PackageSource::Snap) else {
            self.snap_details.cancel();
            return;
        };
        let Some(request) = self.snap_details.begin(&pkg) else {
            return;
        };
        cx.spawn(async move |this, cx| {
            // Rapid selection changes should not start filesystem work for every click.
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;
            if request.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            let (tx, rx) = oneshot::channel();
            backend::spawn_snap_details(pkg.package_id.clone(), request.cancel, tx);
            if let Ok(details) = rx.await {
                this.update(cx, |this, cx| {
                    if this.snap_details.accept(request.generation, &pkg, details) {
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
pub(super) fn breakdown(details: Option<&SnapDetails>) -> AnyElement {
    let mut content = div().flex().flex_col().gap(px(12.));
    if let Some(details) = details {
        for (effect, heading) in [
            (RemovalEffect::RemovedBySnap, "Ready to remove"),
            (RemovalEffect::Kept, "Kept after uninstall"),
            (RemovalEffect::Unverified, "Needs review"),
        ] {
            if details.items.iter().any(|item| item.effect == effect) {
                content = content.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(3.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .pb(px(4.))
                                .text_size(px(12.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(heading)
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .font_weight(FontWeight::NORMAL)
                                        .text_color(theme::text_faint())
                                        .child("Stored size"),
                                ),
                        )
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
                    .text_size(px(11.))
                    .line_height(px(16.))
                    .text_color(theme::text_dim())
                    .child(warning.clone()),
            );
        }
    } else {
        content = content.child(
            div()
                .text_size(px(12.))
                .text_color(theme::text_dim())
                .child("Reading revisions, data and recovery snapshots…"),
        );
    }
    content.child(
        div().pt(px(10.)).border_t_1().border_color(theme::border())
            .flex().flex_col().gap(px(4.)).text_size(px(11.)).line_height(px(16.))
            .text_color(theme::text_dim())
            .child("Snap may back up managed data before removal. Default retention: 31 days; system settings may differ.")
            .child(div().text_color(theme::text_faint())
                .child("Stored sizes are not space freed. Other users' data and files outside these paths are not measured."))
    ).into_any_element()
}

fn detail_row(item: &DetailItem) -> AnyElement {
    let size = match item.size.bytes {
        Some(bytes) if item.size.complete => measured_size(bytes),
        Some(bytes) => format!("{}+", measured_size(bytes)),
        None => "Unknown".into(),
    };
    let icon = match item.effect {
        RemovalEffect::RemovedBySnap => IconName::Check,
        RemovalEffect::Kept => IconName::Minus,
        RemovalEffect::Unverified => IconName::CircleAlert,
    };
    let location = item.location.as_deref().unwrap_or(&item.label);
    let location = if item.label.starts_with("Recovery snapshot") {
        format!("{location} · {}", item.label)
    } else {
        location.to_owned()
    };
    let mut path = div().flex_1().min_w(px(0.)).flex().flex_col().child(
        div()
            .font_family("monospace")
            .text_size(px(11.))
            .text_color(theme::text_dim())
            .child(location),
    );
    if !item.size.complete {
        if let Some(note) = &item.size.note {
            path = path.child(
                div()
                    .text_size(px(10.))
                    .text_color(theme::text_faint())
                    .child(note.clone()),
            );
        }
    } else if item.effect == RemovalEffect::Unverified {
        path = path.child(
            div()
                .text_size(px(10.))
                .text_color(theme::text_faint())
                .child("Removal coverage is not verified"),
        );
    }
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .px(px(4.))
        .py(px(6.))
        .text_size(px(12.))
        .line_height(px(16.))
        .child(
            div()
                .flex_none()
                .w(px(142.))
                .flex()
                .items_center()
                .gap(px(9.))
                .child(Icon::new(icon).size(px(12.)).text_color(
                    if item.effect == RemovalEffect::RemovedBySnap {
                        theme::accent_text()
                    } else {
                        theme::text_faint()
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(item_kind(item)),
                ),
        )
        .child(path)
        .child(
            div()
                .flex_none()
                .w(px(85.))
                .text_right()
                .text_size(px(11.))
                .text_color(theme::text_dim())
                .child(size),
        )
        .into_any_element()
}

fn item_kind(item: &DetailItem) -> &'static str {
    if item.label.starts_with("Active revision") {
        "Application"
    } else if item.label.starts_with("Older revision") {
        "Older revision"
    } else if item.label.starts_with("Component") {
        "Component"
    } else if item.label.starts_with("Recovery snapshot") {
        "Recovery snapshot"
    } else if item.label.starts_with("System") {
        "System data"
    } else {
        "User data"
    }
}

fn measured_size(bytes: u64) -> String {
    if bytes == 0 {
        "0 B".into()
    } else {
        theme::format_size(bytes)
    }
}
