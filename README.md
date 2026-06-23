# Scope

> **Note:** Scope was previously a TUI app. The TUI is no longer maintained — the last TUI release ([v0.1.4](https://github.com/khurrambhutto/scope/releases/tag/v0.1.4)) is still available to download.

**See, update, and uninstall every app on your Linux system — all in one place.**

Scope is a desktop app for Linux inspired by [Mole](https://github.com/tw93/Mole) for macOS. It gives you a unified view of packages from APT, Snap, Flatpak, and AppImage so you can manage them all without switching between terminals and tools.

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

## How It Works

### Scanning (`src-tauri/src/scanner/`)

Scope scans four package sources in parallel and merges them into one unified `InstalledPackage` list.

1. **APT/dpkg** (`apt.rs`) — lists *manually* installed packages via `apt-mark showmanual`, then fetches metadata (version, installed size, summary) for exactly those names in a single `dpkg-query` call. Reporting only manual installs keeps the list focused on apps the user chose, not thousands of pulled-in dependencies. Each package is classified GUI/CLI by checking for a `.desktop` file or a binary on `PATH`.
2. **Snap** (`snap.rs`) — parses `snap list` and skips runtime/base snaps (`core*`, `gtk-*`, `gnome-*`, `snapd`, `bare`). On-disk size is measured from `/snap/<name>/current`. Snap packages start as CLI/unknown based on `/snap/bin`, then the desktop-entry enrichment step promotes real GUI snaps to GUI.
3. **Flatpak** (`flatpak.rs`) — requires the `flatpak` binary, then scans `--user` and `--system` apps separately with explicit tab-delimited columns. Only applications are reported; runtimes are excluded. User/system installs are modeled as distinct package keys (`flatpak:user:<id>` / `flatpak:system:<id>`) so uninstall plans are unambiguous.
4. **AppImage** (`appimage.rs`) — walks well-known directories (`~/Applications`, `~/.local/bin`, `/opt`, etc.) for `.AppImage` files and **validates the magic bytes** (`ELF + "AI" + type byte`) before reporting, so renamed files don't pollute the list. Name/version are parsed from the filename.

After all scanners finish, a **merge/enrich step** runs on a blocking thread: each package is matched against a `DesktopIndex` (built from discovering `.desktop` files in `XDG_DATA_DIRS`) to fill in display name, icon, categories, and `terminal=`. Icons are resolved through the XDG icon-theme lookup in `icons/` and served to the webview via a custom `scope-icon://` URI scheme — the frontend never receives broad filesystem access. Results are sorted apps-first, by display name.

The whole scan is cached in Tauri state so filtering/search doesn't re-run the package managers.

### Uninstall (`src-tauri/src/operations/` + `safety/`)

Every removal is **preview-first and backend-validated**. Scope never handles passwords — privilege escalation goes through `pkexec` (Polkit), which shows the native Linux password dialog.

The flow:

1. **Preview** — the frontend calls `preview_uninstall(packageKey)`. The backend looks the package up in the cached scan, runs the **safety deny-list** (`safety/mod.rs`), and builds an `OperationPlan` (package id, install scope when relevant, version, the exact command that will run, whether a password is needed). The plan is stored server-side in a `PlanStore` with a 5-minute TTL. The frontend receives it only for display.
2. **Confirm** — the user reviews the plan in a modal and clicks Confirm. The frontend sends **only `plan_id`** back — it can never tamper with what gets executed.
3. **Apply** — the backend takes the plan from `PlanStore` (rejecting stale/missing/expired plans), **re-scans the system** and revalidates the package is still present and still passes safety, then runs the source-specific command with a timeout, capturing stdout/stderr as logs.

Per-source commands:

| Source | Command | Auth |
| --- | --- | --- |
| APT | `pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y <pkg>` | Polkit popup |
| Snap | `pkexec snap remove <pkg>` | Polkit popup |
| Flatpak (user) | `flatpak uninstall -y --user <id>` | None |
| Flatpak (system) | `pkexec flatpak uninstall -y --system <id>` | Polkit popup |
| AppImage | `gio trash -f <path>` (with manual Trash fallback) | None |

**Safety guarantees:**

- A backend **deny-list** blocks system-critical packages (`ubuntu-desktop`, `systemd`, `apt`, `dpkg`, kernels, `pkexec`, snap runtimes, shared libraries) and protected paths — enforced independently of the frontend, so a crafted `invoke` can't bypass it. (Unit-tested in `safety/mod.rs`.)
- No `sh -c` with frontend strings — only explicit argv.
- AppImages go to **Trash** (restorable), never permanently deleted.
- Every operation has a timeout and captured logs shown in the result dialog.
- On success, the package list automatically rescans to reflect the new state.

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
