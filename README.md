<p align="center">
  <img src="assets/scope-icon.svg" alt="Scope for Linux logo" width="96" />
</p>

<h1 align="center">Scope for Linux</h1>

<p align="center">Browse, update, and uninstall Linux apps in one place.</p>

<p align="center">
  <a href="https://khurrambhutto.github.io/scope/">Website</a> ·
  <a href="https://github.com/khurrambhutto/scope/releases">Downloads</a> ·
  <a href="https://github.com/khurrambhutto/scope/issues">Report an issue</a>
</p>

Scope for Linux brings installed apps, command-line tools, and Steam games into one searchable list. Inspect app details and preview updates or removals before confirming. Ubuntu on x86-64 is the primary target.

<p align="center">
  <img src="docs/image.png" alt="Scope app list with search and source filters" width="49%" />
  <img src="docs/app-details.png" alt="Expanded app details showing files and storage sizes" width="49%" />
</p>

## Install

Download the `.deb` or AppImage from [Releases](https://github.com/khurrambhutto/scope/releases). For the v0.3.8 `.deb`, run:

```bash
sudo apt install ./scope_0.3.8_amd64.deb
```

For the portable AppImage:

```bash
chmod +x Scope-0.3.8-x86_64.AppImage
./Scope-0.3.8-x86_64.AppImage
```

Official `.deb` builds support verified self-updates.

## Usage

Search or filter by source, select an app, then choose **Update** or **Uninstall**. Review the preview and confirm. Operations that need administrator access prompt for authentication.

Use **Updates** to see available updates and **Show all** to include packages hidden from the default app list. Protected packages remain locked.

APT apps with visible app-menu launchers appear even when installed automatically. They use the same update and uninstall previews as other APT apps. Uninstall previews warn when other apps or desktop features may depend on them.

Uninstall previews show app files, app data, and anything kept or unverified in one place. Reviewed app-data rules can select settings, profiles, caches and state for removal; exact-name associations and mixed-content folders require separate review. Choose **Keep app data** to retain it. Files inside selected folders move to Trash, including any personal content there, and remain recoverable until Trash is emptied. Shared or unverifiable data stays in place and appears in the result.

APT uninstall, including apps installed from `.deb` files, purges the package and its package configuration without autoremove. APT, Snap and Flatpak removal is checked before selected app-data cleanup starts. Flatpak profiles and saved permissions have separate choices. Snap recovery snapshots remain; normal Snap removal may create a new snapshot.

To install a downloaded `.deb` or AppImage, choose **Open With → Scope** and review the installation preview. AppImages are added to your app menu and stored in `~/AppImages`.

## Supported sources

| Source | Uninstall | Update |
| --- | --- | --- |
| APT, Snap, Flatpak | Yes | Yes |
| uv tools, pipx, Cargo | Yes | Yes |
| npm global, Bun global | Yes | Yes |
| AppImage in `~/AppImages` (installed by Scope or Gear Lever) | Yes | Yes, when the AppImage publishes update information |
| Verified extracted AppDir under `~/.local/opt` | Yes, after ownership and launcher checks | No |
| Other AppImages, unmanaged desktop launchers | Discovery only | Discovery only |
| Steam games | Read-only | Open in Steam |
| JetBrains Toolbox IDEs | Read-only | Open Toolbox |

Tool management supports verified installations in your home directory. Update availability depends on the package source and installation method. AppImage integration requires `squashfs-tools`; most AppImages also need `libfuse2t64` to run.

App-data coverage is evidence-based, not a guarantee that every file an app ever created is known. Current reviewed identities include Paperling's retained `com.saqla.marklite` profile, Handy's app namespace, VS Code's configuration and Ruff's global cache. Other managed apps can offer exact package/command/launcher-name matches for review. Projects, shared manager caches, cloud data and other users' data are outside that cleanup coverage. A known running app or incomplete ownership check blocks selected cleanup. If Linux hides background-process metadata, the preview warns and leaves data unchecked; deliberate selections acknowledge only the displayed gaps, and changed process state requires a fresh preview. Background writers may recreate data; remaining paths stay visible after the app leaves the list.

JetBrains Toolbox IDEs are discovered read-only from local Toolbox metadata. **Open Toolbox** starts the installed Toolbox launcher only when selected; IDE updates, uninstalls, and footprint measurement are unavailable in Scope.

## Build from source

Install Rust stable and the Linux dependencies, then run:

```bash
sudo apt install build-essential pkg-config libfontconfig1-dev libx11-dev libx11-xcb-dev \
  libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev wayland-protocols
git clone https://github.com/khurrambhutto/scope.git
cd scope
cargo run
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and pull request guidelines. [AGENTS.md](AGENTS.md) describes the architecture and safety rules. Report bugs through [Issues](https://github.com/khurrambhutto/scope/issues) and security vulnerabilities through [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE)
