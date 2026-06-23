# Scope

> **Note:** Scope was previously a TUI app. The TUI is no longer maintained — the last TUI release ([v0.1.4](https://github.com/khurrambhutto/scope/releases/tag/v0.1.4)) is still available to download.

**See, update, and uninstall every app on your Linux system — all in one place.**

Scope is a desktop app for Linux. It gives you a unified view of packages from APT, Snap, Flatpak, and AppImage so you can manage them all without switching between terminals and tools.

Inspired by [Mole](https://github.com/tw93/Mole) for macOS.

## Features

- 📦 **Unified Package List** — See user-relevant packages from APT, Snap, Flatpak, and AppImage in a single view, with icons and metadata resolved from `.desktop` entries. Scope intentionally hides low-level APT/Snap runtime packages from the main uninstall surface.
- 🗑️ **Uninstall from Scope** — Remove any package directly from Scope. Privileged removals (APT/Snap/system Flatpak) trigger the native Linux password dialog via Polkit (`pkexec`); AppImages go to Trash. System-critical packages are protected by a backend deny-list. Every removal is **preview-first**: you see the exact command before confirming.
- 🔄 **Update from Scope** — Check for and apply updates across all package sources. *(In progress.)*

## Future Phases

- 🧹 **Clean** — Deep-clean caches, orphaned dependencies, and leftover config files.
- 📊 **Analyze** — Disk usage visualization to find what is eating your space.
- 💻 **Status** — Real-time system health monitoring (CPU, RAM, disk, network).

## Run

Install the [Tauri v2 prerequisites](https://tauri.app/start/prerequisites/) for your distro, then:

```bash
npm install
npm run tauri dev
```

## Tech Stack

| Layer    | Technology          |
| -------- | ------------------- |
| Frontend | React + TypeScript  |
| Backend  | Tauri v2 + Rust     |
| Target   | Ubuntu (.deb) first |

## Project Structure

```text
scope/
├── package.json
├── src/                  # React frontend
│   ├── features/packages/   # unified list, detail, filters
│   ├── features/uninstall/  # uninstall preview/confirm dialog
│   └── shared/              # typed invoke wrappers, types, components
├── src-tauri/            # Tauri v2 + Rust backend
│   └── src/
│       ├── scanner/         # APT/Snap/Flatpak/AppImage scanners
│       ├── desktop_entries/ # .desktop discovery + parsing
│       ├── icons/           # XDG icon-theme resolution + scope-icon://
│       ├── operations/      # OperationPlan + uninstall preview/apply
│       ├── safety/          # protected packages/paths + deny-list
│       └── commands/        # typed Tauri command handlers
├── docs/                 # Project website
└── AGENTS.md             # Agent development notes
```


## License

[MIT](LICENSE)
