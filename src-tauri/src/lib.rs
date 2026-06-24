//! Scope backend wiring.
//!
//! This module only wires submodules, registers Tauri commands (including the
//! `scope-icon://` URI-scheme protocol), and starts the app. Scanner/icon/
//! update/uninstall logic lives in dedicated modules.

mod commands;
mod desktop_entries;
mod icons;
mod operations;
mod package;
mod safety;
mod scanner;
mod system;

use commands::operations::{apply_uninstall, preview_uninstall, apply_update, preview_update};
use commands::packages::{get_cached_scan, scan_packages, scan_status, search_packages, ScanCache};
use operations::PlanStore;
use tauri::http::{header, Response, StatusCode};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(ScanCache::default())
        .manage(PlanStore::default())
        .register_uri_scheme_protocol("scope-icon", |_app, request| {
            // Serve only the specific local file the URI points at. The
            // frontend never picks arbitrary paths: every URL it sees is
            // produced by `icons::icon_url` from a resolved icon path, so this
            // protocol grants no broad filesystem access.
            let raw = request.uri().path();
            let path = percent_decode_path(raw);

            let (data, mime, status) = match std::fs::read(&path) {
                Ok(bytes) => (bytes, icons::mime_for_path(&path), StatusCode::OK),
                Err(_) => (Vec::new(), "text/plain", StatusCode::NOT_FOUND),
            };

            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=86400, immutable")
                .body(data)
                .expect("scope-icon response is always constructable")
        })
        .invoke_handler(tauri::generate_handler![
            scan_packages,
            get_cached_scan,
            scan_status,
            search_packages,
            preview_uninstall,
            apply_uninstall,
            preview_update,
            apply_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Decode the percent-escaped path produced by `icons::icon_url`.
fn percent_decode_path(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}
