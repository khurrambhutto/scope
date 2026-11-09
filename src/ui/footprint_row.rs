//! Shared compact label / path / size layout for details and uninstall review.
use crate::theme;
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, FontWeight};

pub(super) fn row(
    label: impl Into<String>,
    location: Option<&str>,
    size: String,
    control: Option<AnyElement>,
    note: Option<&str>,
) -> AnyElement {
    let mut name = div()
        .flex_none()
        .w(px(theme::DETAIL_LABEL_WIDTH))
        .flex()
        .items_center()
        .gap(px(8.));
    if let Some(control) = control {
        name = name.child(control);
    }
    let label = label.into();
    if !label.is_empty() {
        name = name.child(
            div()
                .min_w(px(0.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(label),
        );
    }
    div()
        .flex()
        .items_start()
        .gap(px(theme::DETAIL_ROW_GAP))
        .py(px(theme::DETAIL_ROW_PADDING))
        .text_size(px(13.))
        .line_height(px(18.))
        .child(name)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(theme::text_dim())
                        .child(location.map(compact_path).unwrap_or_default()),
                )
                .when_some(note, |this, note| {
                    this.child(
                        div()
                            .text_size(px(11.))
                            .text_color(theme::text_dim())
                            .child(note.to_string()),
                    )
                }),
        )
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

fn compact_path(location: &str) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return location.into();
    };
    let path = std::path::Path::new(location);
    match path.strip_prefix(std::path::Path::new(&home)) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".into(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => location.into(),
    }
}
