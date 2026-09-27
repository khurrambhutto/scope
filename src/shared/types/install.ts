// Mirrors the Rust `InstallKind` enum in `src-tauri/src/commands/install.rs`.
// Values come from the bundle type the updater uses to resolve its artifact,
// so "deb"/"rpm"/"appimage" can self-update; "unknown" (dev build, AUR,
// Flatpak) must use the Releases page.

export type InstallKind = "appimage" | "deb" | "rpm" | "unknown";
