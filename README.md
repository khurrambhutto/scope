<p align="center">
  <img src="assets/scope-logo.svg" alt="Scope" width="96" />
</p>

<h1 align="center">Scope</h1>

<p align="center"><strong>See, update, and uninstall every app on your Linux system in one place.</strong></p>

Linux spreads software across APT, Snap, Flatpak, and AppImage. Scope scans all four into a single list, adds desktop names and icons where a GUI app exists, and lets you update or uninstall without typing a package-manager command. Every destructive action shows a preview first. Privileged actions go through Polkit, so Scope never handles your password.

<p align="center">
  <img src="docs/image.png" alt="Scope showing installed apps from APT, Snap, Flatpak, and AppImage in one list, with an inline detail panel and hover actions" width="720" />
</p>

Built with Zed's GPUI (Rust binary in `src/`, no webview). Architecture and module rules live in [AGENTS.md](AGENTS.md).

## Install

Download the `.deb` from [GitHub Releases](https://github.com/khurrambhutto/scope/releases). The app checks the Releases API on startup. Packaged builds offer one-click updates only when the release contains the exact architecture-specific artifact plus a minisign-signed `SHA256SUMS` manifest. Other installs open the Releases page.

```bash
# Ubuntu / Debian
sudo apt install ./scope_<version>_amd64.deb
```

Build from source (Rust stable; Linux needs GPUI system deps, notably `libxkbcommon-x11-dev`):

```bash
sudo apt install libxkbcommon-x11-dev
cargo run
```

## How it works

The main window defaults to recognizable GUI apps and command-line tools, with their source, version, size, and update state. AppImages and user-local desktop apps appear with uninstall disabled. Use **Show all** in the footer to include every package row Scope scanned, including libraries and system components; protected packages remain visible there with uninstall disabled. Search or filter by source, then select a row for full details.

Update and uninstall follow the same flow. Scope builds a plan showing exactly what will run and waits for your confirmation. The backend then revalidates the package against the live system and executes. Commands that need root trigger the standard system password dialog. System-critical packages are deny-listed in the backend and cannot be removed through Scope.

## Status

Works today:

- App/tool list across APT (manual app/tool installs), Snap (runtimes hidden), Flatpak (user and system scoped), detected AppImages, and unmanaged user desktop launchers (uninstall disabled)
- Desktop-entry enrichment and freedesktop icon theme resolution
- Uninstall with preview, Polkit auth, and a protected-package deny-list
- Update with preview for APT, Snap, and Flatpak
- Self-updater banner (checks GitHub Releases, one-click install for self-updatable installs)
- Release pipeline producing `.deb` (Ubuntu first)

Not yet:

- `.rpm` and `.AppImage` release artifacts (installs of those kinds fall back to manual download)
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

Good first contributions are AppImage auto-update (`src/domain/operations/update.rs`), Snap target versions (`src/domain/scanner/snap.rs`), `.rpm`/AppImage release artifacts (`.github/workflows/release.yml`), and new scanners through the `Scanner` trait (`src/domain/scanner/`).

## Release signing

The release job refuses tags that do not match `Cargo.toml`, hashes every package artifact, and signs `SHA256SUMS` with minisign. Repository variable `SCOPE_UPDATE_PUBLIC_KEY` is compiled into release builds. Repository secret `SCOPE_UPDATE_SECRET_KEY` signs the manifest in GitHub Actions. Never commit the private key.

To rotate keys, generate a new pair, replace both GitHub values before the next release, and publish that release through the normal workflow. Existing builds trust their embedded old key and must update manually across a key rotation.

APT previews run a package-manager simulation and show the transaction impact. Scope rejects any indirect protected removal and repeats the simulation before applying. Flatpak operations fail if the scanner did not provide an explicit user or system scope.

Keep feature logic in domain modules; `main.rs` and `app_view.rs` only compose and wire, `backend.rs` stays a thin orchestration layer. PRs welcome.

## License

[MIT](LICENSE)
