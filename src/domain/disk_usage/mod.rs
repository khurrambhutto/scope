//! Read-only disk usage scan for the Disk tab.
//!
//! Walks HOME (and optionally `/`) with rayon, keeps directories plus the largest
//! files of each directory, and streams pruned snapshots while the scan runs.
//! Nothing here modifies the filesystem.

pub mod classify;
pub mod mounts;
pub mod tree;
pub mod walker;

pub use classify::DiskCategory;
pub use tree::{DiskTree, NodeId, NodeKind};
pub use walker::{scan, FsUsage, ScanOptions, ScanProgress};
