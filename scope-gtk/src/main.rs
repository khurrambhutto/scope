//! Scope — GTK4/libadwaita native frontend.
//!
//! The UI lives in this crate; package scanning, safety rules, preview/apply
//! plans, and icon resolution live in `core/` — a vendored copy of the backend
//! modules (originally shared verbatim with the Tauri app). GTK is now
//! canonical: `core/` is the source of truth, edited here.
//!
//! The core modules are mounted at the crate root because they reference each
//! other through `crate::<module>` paths. `backend.rs` re-exports them under
//! `crate::backend::*` for the UI code.

#[path = "core/package.rs"]
pub mod package;

#[path = "core/system/mod.rs"]
pub mod system;

#[path = "core/safety/mod.rs"]
pub mod safety;

#[path = "core/desktop_entries/mod.rs"]
pub mod desktop_entries;

#[path = "core/icons/mod.rs"]
pub mod icons;

#[path = "core/operations/mod.rs"]
pub mod operations;

#[path = "core/scanner/mod.rs"]
pub mod scanner;

mod app;
mod backend;
mod bridge;
mod detail;
mod ops;
mod packages;
mod tasks;
mod window;

use gtk::prelude::*;

fn main() {
    let application = adw::Application::builder()
        .application_id("com.khurram.scope.gtk")
        .build();

    application.connect_activate(|gtk_app| {
        window::build(gtk_app);
    });

    application.run();
}
