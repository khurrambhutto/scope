//! Scope — GPUI desktop app.
//!
//! The backend domain (`src/domain/`) is vendored from `src-tauri/src` so this
//! crate builds standalone: scanners, icon resolution, safety, and the
//! preview/apply flows live here. `src/backend.rs` orchestrates the domain,
//! and `src/ui/*` renders the screen with GPUI.

// ---- Backend domain (vendored copy, see `src/domain/mod.rs`) ----------------

mod backend;
mod domain;
mod theme;
mod ui;

use std::borrow::Cow;

use gpui::{
    px, size, App, AppContext, Application, AssetSource, Bounds, SharedString, TitlebarOptions,
    WindowBounds, WindowDecorations, WindowOptions,
};

use ui::app_view::ScopeApp;

/// Filesystem-backed assets so `svg()`/`img()` can load absolute paths (the
/// resolved app icons and the bundled brand mark).
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        std::fs::read(path)
            .map(Into::into)
            .map(Some)
            .map_err(Into::into)
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(std::fs::read_dir(path)?
            .filter_map(|entry| {
                entry
                    .ok()
                    .map(|entry| SharedString::from(entry.path().to_string_lossy().into_owned()))
            })
            .collect())
    }
}

fn main() {
    Application::new()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            cx.bind_keys(ui::text_input::key_bindings());
            cx.bind_keys(ui::app_view::key_bindings());

            // Single-window app: closing the window quits.
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(1120.), px(740.)), cx);
            let window = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        titlebar: Some(TitlebarOptions {
                            title: Some("Scope".into()),
                            ..Default::default()
                        }),
                        app_id: Some("scope".into()),
                        // GNOME on Wayland has no server-side decorations, so the
                        // app draws its own title bar (see `ui::app_view::title_bar`).
                        window_decorations: Some(WindowDecorations::Client),
                        focus: true,
                        ..Default::default()
                    },
                    |_window, cx| cx.new(ScopeApp::new),
                )
                .expect("failed to open the Scope window");

            window
                .update(cx, |app, window, cx| app.focus_search(window, cx))
                .ok();
            cx.activate(true);
        });
}
