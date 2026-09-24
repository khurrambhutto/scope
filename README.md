<p align="center">
  <img src="public/scope-logo.svg" alt="Scope" width="96" />
</p>

# Scope

**See, update, and uninstall every app on your Linux system in one place.**

Linux spreads software across APT, Snap, Flatpak, and AppImage. Scope scans all four into a single list, adds desktop names and icons where a GUI app exists, and lets you update or uninstall without typing a package-manager command. Every destructive action shows a preview first. Privileged actions go through Polkit, so Scope never handles your password.

Built with Tauri v2, a Rust backend and a React 19 frontend. Architecture and module rules live in [AGENTS.md](AGENTS.md).

## Install

Download `.deb`, `.rpm`, or `.AppImage` from [GitHub Releases](https://github.com/khurrambhutto/scope/releases). The app self-updates after install.

```bash
# Ubuntu / Debian
sudo apt install ./scope_<version>_amd64.deb
```

Build from source (Rust stable, Node 20+, WebKitGTK dev headers):

```bash
sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
npm install
npm run tauri dev
```

## How it works

The main window lists every user-relevant package with its source, version, size, and update state. Search or filter by source and GUI/CLI, then select a row for full details.

Update and uninstall follow the same flow. Scope builds a plan showing exactly what will run and waits for your confirmation. The backend then revalidates the package against the live system and executes. Commands that need root trigger the standard system password dialog. System-critical packages are deny-listed in the backend and cannot be removed through Scope.

## Status

Works today:

- Unified package list across APT (manual installs), Snap (runtimes hidden), Flatpak (user and system scoped), and AppImage
- Desktop-entry enrichment and freedesktop icon theme resolution
- Uninstall with preview, Polkit auth, and a protected-package deny-list
- Update with preview for APT, Snap, and Flatpak
- Self-updater with download progress
- Release pipeline producing `.deb`, `.rpm`, and `.AppImage`

Not yet:

- AppImage auto-update and update detection
- Snap target version in the update preview
- Whole-system cleanup and disk usage views
- Fedora and Arch package-manager support

## Development

```bash
npm run build                                  # type-check and build frontend
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
npm run tauri dev                              # run the app
```

Run the build and cargo checks before submitting. Safety-sensitive backend changes need targeted Rust tests first.

Good first contributions are AppImage auto-update (`src-tauri/src/operations/update.rs`), Snap target versions (`src-tauri/src/scanner/snap.rs`), frontend tests (none exist yet), and new scanners through the `Scanner` trait (`src-tauri/src/scanner/`).

Keep business logic in the domain modules and keep Tauri command handlers thin. PRs welcome.

## License

[MIT](LICENSE)
