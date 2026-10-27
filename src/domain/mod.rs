//! Backend domain vendored from `src-tauri/src`.
//!
//! Single source of truth for the GPUI shell lives here now — no `#[path]`
//! imports. The Tauri shell keeps its own copy under `src-tauri/src/`.
//! Sync intentionally when a scanner, DTO, or safety rule changes.

pub mod appimage;
pub mod desktop_entries;
pub mod icons;
pub mod install_metadata;
pub mod jetbrains_toolbox;
pub mod listing;
pub mod operations;
pub mod package;
pub mod package_details;
pub mod package_files;
pub mod safety;
pub mod scanner;
pub mod snap_details;
pub mod system;
pub mod updater;

pub mod tool_managers;
