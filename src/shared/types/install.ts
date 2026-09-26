// Mirrors the Rust `InstallKind` enum in `src-tauri/src/commands/install.rs`
// (`appimage` = running from an AppImage, `system` = .deb/.rpm/… install).

export type InstallKind = "appimage" | "system";
