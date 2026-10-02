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
/// resolved app icons).
///
/// The six bundled brand/control SVGs are embedded in the binary: the UI
/// refers to them via `concat!(env!("CARGO_MANIFEST_DIR"), "/assets/...")`,
/// which bakes the *build* machine's source dir into the path. That path does
/// not exist on user machines (or matches a different checkout locally), so a
/// pure `fs::read` leaves the logo, window controls, and filter/refresh icons
/// invisible in the installed `.deb`. Matching on the `/assets/<name>` suffix
/// serves the embedded bytes regardless of which prefix was baked in, while
/// real app-icon paths (theme dirs, `~/.local`, etc.) still read from disk.
struct Assets;

fn embedded_asset(path: &str) -> Option<&'static [u8]> {
    let suffix = path
        .find("/assets/")
        .map(|i| &path[i + "/assets/".len()..])?;
    match suffix {
        "scope-logo.svg" => Some(include_bytes!("../assets/scope-logo.svg")),
        "win-min.svg" => Some(include_bytes!("../assets/win-min.svg")),
        "win-max.svg" => Some(include_bytes!("../assets/win-max.svg")),
        "win-close.svg" => Some(include_bytes!("../assets/win-close.svg")),
        "filter.svg" => Some(include_bytes!("../assets/filter.svg")),
        "refresh.svg" => Some(include_bytes!("../assets/refresh.svg")),
        _ => None,
    }
}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = embedded_asset(path) {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_svgs_resolve_regardless_of_baked_prefix() {
        // `concat!(env!("CARGO_MANIFEST_DIR"), ...)` bakes the build machine's
        // checkout path (e.g. CI's `/home/runner/...`) into the binary. The
        // installed app must still find the icons.
        for name in [
            "scope-logo.svg",
            "win-min.svg",
            "win-max.svg",
            "win-close.svg",
            "filter.svg",
            "refresh.svg",
        ] {
            let ci_path = format!("/home/runner/work/scope/scope/assets/{name}");
            let bytes = embedded_asset(&ci_path);
            assert!(bytes.is_some_and(|b| !b.is_empty()), "{name} missing");
        }
        // Real app-icon paths still fall through to the filesystem.
        assert!(embedded_asset("/usr/share/icons/hicolor/48x48/apps/firefox.png").is_none());
        assert!(embedded_asset(&format!(
            "{}/assets/not-an-icon.svg",
            env!("CARGO_MANIFEST_DIR")
        ))
        .is_none());
    }
}
