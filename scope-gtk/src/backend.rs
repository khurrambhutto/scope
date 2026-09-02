//! Facade over the vendored backend core.
//!
//! The core modules (scanners, safety, operations, icons, desktop entries)
//! live in `core/` and are mounted at the crate root in `main.rs` so their
//! internal `crate::<module>` references resolve. This module re-exports them
//! under `crate::backend::*` so UI code reads consistently.

#[allow(unused_imports)]
pub use crate::{desktop_entries, icons, operations, package, safety, scanner, system};
