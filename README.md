<p align="center">
  <img src="assets/scope-logo.svg" alt="Scope" width="96" />
</p>

<h1 align="center">Scope</h1>

<p align="center"><strong>See, update, and uninstall every app on your Linux system in one place.</strong></p>

Scope brings APT, Snap, Flatpak, AppImages, and user-local desktop apps into one searchable list. Update or uninstall supported apps after reviewing a preview of the operation. Privileged actions use the standard Polkit dialog, so Scope never handles your password.

<p align="center">
  <img src="docs/image.png" alt="Scope v0.3.3 showing the Apps and Updates toggle, installed apps, search, source filter, and Show all control in its dark red interface" width="960" />
</p>

Built with Zed's GPUI (Rust binary in `src/`, no webview). Architecture and module rules live in [AGENTS.md](AGENTS.md).

## Install

Current release: [v0.3.3](https://github.com/khurrambhutto/scope/releases/tag/v0.3.3).

Download [scope_0.3.3_amd64.deb](https://github.com/khurrambhutto/scope/releases/download/v0.3.3/scope_0.3.3_amd64.deb) for Ubuntu on x86-64, then run this command from the download directory:

```bash
sudo apt install ./scope_0.3.3_amd64.deb
```

New releases include an x86-64 AppImage. Download it from the [releases page](https://github.com/khurrambhutto/scope/releases), make it executable, and launch it directly. Replace `VERSION` with the release version:

```bash
APPIMAGE=Scope-VERSION-x86_64.AppImage
chmod +x "$APPIMAGE"
"./$APPIMAGE"
```

Scope checks GitHub Releases on startup. Official `.deb` builds from v0.3.3 can install updates after verifying the signed checksum manifest. Install v0.3.3 manually when upgrading from an earlier version without an embedded verification key. Other install types open the Releases page when a compatible verified package is unavailable.

To build from source, install Rust stable and the Linux dependencies, then clone the repository:

```bash
sudo apt install build-essential pkg-config libfontconfig1-dev libx11-dev libx11-xcb-dev \
  libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev wayland-protocols
git clone https://github.com/khurrambhutto/scope.git
cd scope
cargo run
```

## How it works

Use the centered **Apps / Updates** toggle to switch between installed apps and available updates. Search by app or package name, filter by source, or refresh to rescan. Select a row for details and supported actions.

The default list shows recognizable GUI apps and command-line tools. AppImages and user-local desktop apps appear for discovery, with update and uninstall disabled. **Show all** in the footer reveals every row returned by the scanners, including libraries and system components. APT scans manual installs only, and Snap and Flatpak runtimes remain excluded. Protected packages cannot be uninstalled, even with Show all enabled.

Update and uninstall follow the same flow. Scope builds a plan showing exactly what will run and waits for your confirmation. The backend then revalidates the package against the live system and executes. Commands that need root trigger the standard system password dialog. System-critical packages are deny-listed in the backend and cannot be removed through Scope.

## Status

Works today:

- App/tool list across APT (manual app/tool installs), Snap (runtimes hidden), Flatpak (user and system scoped), detected AppImages, and unmanaged user desktop launchers (uninstall disabled)
- Desktop-entry enrichment and freedesktop icon theme resolution
- Uninstall with preview, Polkit auth, and a protected-package deny-list
- Update with preview for APT, Snap, and Flatpak
- Rounded window with a centered Apps / Updates toggle and consistent dark red controls
- Self-updater with signed checksum verification for official `.deb` builds
- `.deb` releases with a signed SHA-256 manifest, targeting Ubuntu first
- signed x86-64 AppImage releases with bundled dependencies and desktop metadata

Not yet:

- `.rpm` releases (installs of that kind fall back to manual download)
- AppImage uninstall and update
- Snap target version in the update preview
- Whole-system cleanup and disk usage views
- Fedora and Arch package-manager support

## Development

```bash
cargo run
cargo check
cargo test
cargo clippy
```

Run all three checks before submitting. Safety-sensitive backend changes need targeted Rust tests first.

Good first contributions are AppImage auto-update (`src/domain/operations/update.rs`), Snap target versions (`src/domain/scanner/snap.rs`), `.rpm` release artifacts (`.github/workflows/release.yml`), and new scanners through the `Scanner` trait (`src/domain/scanner/`).

## Release signing

The release job refuses tags that do not match `Cargo.toml`, hashes every package artifact, and signs `SHA256SUMS` with minisign. Repository variable `SCOPE_UPDATE_PUBLIC_KEY` is compiled into release builds. Repository secret `SCOPE_UPDATE_SECRET_KEY` signs the manifest in GitHub Actions. Never commit the private key.

To rotate keys, generate a new pair, replace both GitHub values before the next release, and publish that release through the normal workflow. Existing builds trust their embedded old key and must update manually across a key rotation.

APT previews run a package-manager simulation and show the transaction impact. Scope rejects any indirect protected removal and repeats the simulation before applying. Flatpak operations fail if the scanner did not provide an explicit user or system scope.

Keep feature logic in domain modules; `main.rs` and `app_view.rs` only compose and wire, `backend.rs` stays a thin orchestration layer. PRs welcome.

## License

[MIT](LICENSE)
