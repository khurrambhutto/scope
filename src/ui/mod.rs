//! GPUI screen composition for Scope.
//!
//! [`app_view`] owns the window state and render loop; the sibling modules hold
//! the pieces it composes — rows, the inline detail panel, dialogs, filters,
//! the title bar, the search input, and shared widgets.
pub mod app_view;
pub mod detail;
mod detail_transition;
pub mod dialog;
pub mod filters;
mod operation_controller;
mod operation_footer;
mod package_list_model;
pub mod resize_zones;
pub mod row;
mod smooth_scroll;
pub mod title_bar;
pub mod updater;
pub mod widgets;

mod snap_detail_state;
mod snap_details;
