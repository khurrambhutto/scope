//! Backend domain vendored from `src-tauri/src`.
//!
//! Single source of truth for the GPUI shell lives here now — no `#[path]`
//! imports. The Tauri shell keeps its own copy under `src-tauri/src/`.
//! Sync intentionally when a scanner, DTO, or safety rule changes.

pub mod desktop_entries;
pub mod icons;
pub mod listing;
pub mod operations;
pub mod package;
pub mod safety;
pub mod scanner;
pub mod system;
pub mod updater;
