//! Small typed Tauri command handlers.
//!
//! Each command is a thin wrapper over backend logic; no scanner/icon/update
//! business logic lives here.

pub mod install;
pub mod operations;
pub mod packages;
