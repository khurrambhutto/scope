# Scope

> **Note:** Scope was previously a TUI app. The TUI is no longer maintained — the last TUI release ([v0.1.4](https://github.com/khurrambhutto/scope/releases/tag/v0.1.4)) is still available to download.

**See, update, and uninstall every app on your Linux system — all in one place.**

Scope is a desktop app for Linux inspired by [Mole](https://github.com/tw93/Mole) for macOS. It gives you a unified view of packages from APT, Snap, Flatpak, and AppImage so you can manage them all without switching between terminals and tools.

## Features (Phase 1 — In Development)

- 📦 **Unified Package List** — See everything installed from APT, Snap, Flatpak, and AppImage in a single view.
- 🗑️ **One-Click Uninstall** — Remove any package directly from Scope with preview-first safety.
- 🔄 **Update from Scope** — Check for and apply updates across all package sources.

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
├── src-tauri/            # Tauri v2 + Rust backend
│   └── src/
├── docs/                 # Project website
└── AGENTS.md             # Agent development notes
```


## License

[MIT](LICENSE)
