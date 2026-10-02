//! Scope — GPUI rewrite of the Tauri desktop app.
//!
//! The backend domain modules are reused verbatim from `src-tauri/src` through
//! `#[path]`, so scanners, icon resolution, safety, and the preview/apply flows
//! have a single source of truth. Only the Tauri command layer and the webview
//! UI are replaced: `src/backend.rs` orchestrates the shared modules, and
//! `src/ui/*` renders the same screen with GPUI.

// ---- Shared backend (unmodified from the Tauri crate) ----------------------

#[path = "../../src-tauri/src/package.rs"]
mod package;
#[path = "../../src-tauri/src/scanner/mod.rs"]
mod scanner;
#[path = "../../src-tauri/src/desktop_entries/mod.rs"]
mod desktop_entries;
#[path = "../../src-tauri/src/icons/mod.rs"]
#[allow(dead_code)]
mod icons;
#[path = "../../src-tauri/src/safety/mod.rs"]
mod safety;
#[path = "../../src-tauri/src/system/mod.rs"]
mod system;
#[path = "../../src-tauri/src/operations/mod.rs"]
#[allow(dead_code)]
mod operations;

mod backend;
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
                    |_window, cx| cx.new(|cx| ScopeApp::new(cx)),
                )
                .expect("failed to open the Scope window");

            window
                .update(cx, |app, window, cx| app.focus_search(window, cx))
                .ok();
            cx.activate(true);
        });
}
