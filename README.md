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

Scope for Linux is a desktop app written in Rust with GPUI. It combines installed apps, command-line tools, and locally installed Steam games into one searchable list, with package details and a confirmation preview before every update or uninstall. Ubuntu on x86-64 is the primary target.

<p align="center">
  <img src="docs/image.png" alt="Scope app list with search and source filters" width="49%" />
  <img src="docs/app-details.png" alt="Expanded app details showing files and storage sizes" width="49%" />
</p>

## Install

Download the `.deb` or AppImage from [Releases](https://github.com/khurrambhutto/scope/releases). For the v0.3.6 `.deb`, run:

```bash
sudo apt install ./scope_0.3.6_amd64.deb
```

For the portable AppImage:

```bash
chmod +x Scope-0.3.6-x86_64.AppImage
./Scope-0.3.6-x86_64.AppImage
```

Official `.deb` builds support self-updates with signed checksum verification.

## Usage

Search or filter by source, select an app to inspect its details, then choose **Update** or **Uninstall**. Review the command preview and confirm. Scope rechecks the installation before running the package manager and streams its output. Privileged operations use the standard Polkit dialog.

Switch to **Updates** to see available updates. **Show all** reveals the scanned inventory while keeping protected packages locked.

## Supported sources

| Source | Uninstall | Update |
| --- | --- | --- |
| APT, Snap, Flatpak | Yes | Yes |
| uv tools, pipx | Yes | Yes |
| Cargo | Yes | Yes |
| npm global, Bun global | Yes | Yes |
| AppImage, unmanaged desktop launchers | Discovery only | Discovery only |
| Steam games | Read-only | Open in Steam |
| JetBrains Toolbox IDEs | Read-only | Open Toolbox |

APT scans manual installs; Snap and Flatpak exclude runtimes. Steam games are discovered from native and Flatpak Steam libraries; Scope is read-only and **Open in Steam** hands management to Steam. JetBrains Toolbox IDEs are discovered read-only from Toolbox metadata in default, legacy, and custom roots. **Open Toolbox** explicitly launches the resolved, valid installed `jetbrains-toolbox.desktop` manager launcher only when clicked; scanning never launches Toolbox, launch errors appear in Scope, and IDE updates and uninstalls remain unavailable there. Scope does not promise measured IDE or user-data sizes.

Tool management requires verified user-owned installations inside your home directory. Global pipx installs and untracked Cargo binaries are unsupported. Git/local Python sources and local/custom-registry Cargo sources have no automatic updates. Pins and supported installation constraints are respected; pipx upgrades may drop original version constraints. Shared caches, interpreters, toolchains, and personal data are kept.

npm and Bun scan global packages only, using the currently selected manager and its global root. Project and home-directory local dependencies are excluded. System-owned roots and conflicting command links are protected; npm, Bun, Node, and Corepack cannot be removed or updated through these tool operations. Bun updates preserve declared version ranges and existing script trust; npm global updates follow native npm registry behavior because global installs do not retain an original version range. Detected Git/local/linked/aliased JavaScript sources have no automatic registry updates.

Tool footprint details stream sizes for uv/pipx environments, Cargo binaries, and npm/Bun package directories. Commands already included in those sizes are labelled Included. Retained shared data is not measured, and data the manager cannot attribute stays explicit. Large measurements remain bounded; `+` marks partial totals. Sizes describe installed footprints, not guaranteed freed space, especially when files share hard-linked storage.

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
