//! Disk tab: a read-only sunburst of HOME and the root filesystem.
//!
//! `view` owns the scan and zoom state, `render` composes the page and `panel`
//! draws the focus card and list. `geometry` lays out and hit-tests arcs,
//! `motion` tweens between layouts and `rings` paints them. Those three know
//! nothing about disks.

pub(crate) mod geometry;
pub(crate) mod motion;
mod panel;
mod render;
pub(crate) mod rings;
mod view;

pub(in crate::ui) use render::page;
pub(in crate::ui) use view::DiskState;
