# Scope Agent Notes

## What Is Scope

Scope is a desktop app for Linux — inspired by [Mole](https://github.com/tw93/Mole) for macOS — that gives users a single place to see, update, and uninstall every app on their system regardless of how it was installed.

Linux distributions spread packages across APT, Snap, Flatpak, AppImage, and manual installs. Users have no unified view and must remember which tool installed what. Scope solves that.

**Target platform:** Ubuntu (`.deb` first), with plans to support other Debian-based distros and eventually Fedora/Arch families.

## Architecture

- **Frontend:** React + TypeScript in `src/`.
- **Desktop shell:** Tauri v2 + Rust in `src-tauri/`.
- **Package scanning & operations:** Rust backend in `src-tauri/src/`.
- **Package scanning logic:** source-specific modules under `src-tauri/src/scanner/`.
- **Package/app models:** shared typed models in `src-tauri/src/package.rs`.
- **Website / docs:** static files in `docs/`.

Do not restore the old Rust TUI architecture. Scope is now a root Tauri v2 desktop app.

## Modular Codebase Rules

Scope must be developed as a modular app. Do not put feature logic directly in `App.tsx`, `lib.rs`, `main.rs`, or one large catch-all file.

Backend code should be split by responsibility:

- `src-tauri/src/commands/` — small typed Tauri command handlers only.
- `src-tauri/src/package.rs` — shared package/app models and source enums.
- `src-tauri/src/scanner/` — package-source scanners such as APT/dpkg, Snap, Flatpak, AppImage, and future package managers.
- `src-tauri/src/desktop_entries/` — Linux `.desktop` entry discovery and parsing.
- `src-tauri/src/icons/` — Linux icon theme lookup, icon resolution, icon protocol support, fallbacks, and caching.
- `src-tauri/src/operations/` — update/uninstall preview and apply flows.
- `src-tauri/src/safety/` — protected packages, protected paths, stale-plan checks, and backend validation.
- `src-tauri/src/system/` — shared command execution, timeouts, environment detection, and process helpers.

`lib.rs` should wire modules, manage Tauri state, register commands, and start Tauri. It must not contain scanner, icon, update, uninstall, or UI-specific logic.

Frontend code should be split by feature:

- `src/app/` — app shell, layout, providers, and top-level composition.
- `src/features/packages/` — unified installed package list, package rows/cards, filters, search, and package details.
- `src/features/apps/` — GUI app metadata and icon presentation when needed.
- `src/features/updates/` — update preview and update flow.
- `src/features/uninstall/` — uninstall preview and confirmation flow.
- `src/shared/api/` — typed Tauri `invoke` wrappers.
- `src/shared/components/` — reusable UI components.
- `src/shared/types/` — TypeScript models matching backend DTOs.

`App.tsx` should only compose the main shell. It must not contain scanner calls, command strings scattered inline, large UI sections, or business logic.

Every new feature must enter through an appropriate module boundary. If a feature does not clearly belong in one of the existing modules, add a small focused module rather than expanding a large file.

## Current Phase — Unified Package View & Uninstall

Phase one (unified package view with icon resolution) and the uninstall capability are complete. The current focus is phase two remainder: make package **updates** work correctly (preview + apply) before building whole-system cleanup.

The active product surface covers three capabilities:

### 1. See All Installed Packages And Apps In One Place

Scope is not only a launcher. It must show everything installed that the supported package sources can report, including GUI apps, CLI tools, libraries, services, runtimes, and system packages.

Scan every package source and display a unified package list:

| Source   | How to list installed packages                  |
| -------- | ----------------------------------------------- |
| APT/dpkg | `dpkg-query -W -f '${Package}\t${Version}\t${Status}\n'` |
| Snap     | `snap list`                                     |
| Flatpak  | `flatpak list --app --columns=application,name,version,origin` |
| AppImage | Scan `~/Applications/` and `~/.local/bin/` for `.AppImage` files |

Each entry should show the best available metadata: name, package id, version, source, installed size, description, update status, protection status, and uninstall capability.

GUI app metadata is an enrichment layer, not the source of truth. Use Linux `.desktop` entries to add display names, launch metadata, categories, and icons for packages that have a GUI app, but do not hide non-GUI packages from the unified package list.

Use `/home/khurram/Projects/klauncher` as the local reference for Linux GUI app discovery and icon rendering:

- Borrow the architecture of `src-tauri/src/platform/linux/desktop_entries.rs` for `.desktop` discovery.
- Borrow the architecture of `src-tauri/src/platform/linux/icon_resolver.rs` for Linux icon theme resolution.
- Borrow the idea of a narrow provider/service layer from `src-tauri/src/providers/apps.rs`.
- Borrow the idea of small typed Tauri commands from `src-tauri/src/commands/launcher.rs`.
- Borrow the custom icon protocol approach from `src-tauri/src/app.rs` so frontend image tags can render validated local icon paths without broad filesystem access.

Adapt this for Scope instead of copying launcher behavior:

- Package scanners find all installed packages/items from supported sources.
- Desktop-entry scanning finds visible GUI app metadata and icons.
- A merge layer combines package-manager data with desktop-entry data into one `InstalledPackage` or `InstalledApp` DTO.
- Non-GUI packages must still appear with a generic package/source icon and clear source metadata.
- The frontend must never scan the filesystem directly and must never receive broad filesystem permissions.

### 2. Uninstall Packages Directly from Scope

Each package source has its own uninstall command:

| Source   | Uninstall command                              | Auth required |
| -------- | ---------------------------------------------- | ------------- |
| APT/dpkg | `sudo apt remove -y <package>`                 | Yes (sudo)    |
| Snap     | `sudo snap remove <package>`                   | Yes (sudo)    |
| Flatpak  | `flatpak uninstall -y <application-id>`        | No (user)     |
| AppImage | Move `.AppImage` file to trash                 | No            |

**Implementation approach:**

- The Rust backend runs the appropriate package-manager command for the source.
- Commands that need `sudo` use `pkexec` (Polkit) to request graphical privilege escalation — the user sees a standard system password dialog. We never store or handle passwords ourselves.
- Every uninstall goes through a **preview step** first: the user sees exactly what will be removed before confirming.
- Essential / system-critical packages (like `ubuntu-desktop`, `systemd`, `linux-image-*`) are protected by a deny-list and cannot be removed through Scope.

### 3. Update Packages from Scope

| Source   | Check for updates                  | Apply update                        |
| -------- | ---------------------------------- | ----------------------------------- |
| APT/dpkg | `apt list --upgradable`            | `sudo apt install -y <package>`     |
| Snap     | `snap refresh --list`              | `sudo snap refresh <package>`       |
| Flatpak  | `flatpak update --appstream && flatpak remote-ls --updates` | `flatpak update -y <application-id>` |
| AppImage | Check upstream URL / AppImageUpdate if available | Download new `.AppImage` and replace |

Updates also use `pkexec` for privilege escalation where needed, and show a preview of version changes before applying.

## Future Phases

These features come later, after package view, update preview/apply, and uninstall preview/apply are correct and tested:

- **Clean:** Deep-clean system caches, orphaned dependencies, old kernels, browser caches, and leftover config from uninstalled apps.
- **Analyze:** Disk usage visualization — find what is eating space.
- **Status:** Real-time system health — CPU, RAM, disk, network.
- **Leftover Removal:** After uninstall, scan for and remove orphan config files, cache dirs, `~/.config/<app>/`, `~/.local/share/<app>/`, etc.

## Safety Rules

- The frontend calls typed Tauri commands via `invoke`. No raw shell access from the webview.
- Never pass frontend-provided strings directly to `sh -c`.
- Package-manager changes go through package-manager commands — never edit dpkg/apt databases directly.
- Privilege escalation uses `pkexec` (Polkit) — Scope never handles passwords.
- File deletion (e.g. AppImage removal) routes through Trash by default.
- A deny-list of protected packages and paths is enforced in the backend before any destructive operation.
- Every destructive action is preview-first: scan → show plan → user confirms → execute.

## Phase Guardrails

- Package scanning: allowed.
- Update availability scanning: allowed.
- Package uninstall previews + apply: **implemented** — backed by `OperationPlan`, `PlanStore` (stale-plan rejection), `safety` deny-list, `pkexec` auth boundaries, timeouts, logs, and backend revalidation. Keep extending only through `operations/` + `commands/operations.rs` + `safety/`.
- Package update previews: allowed once backed by `OperationPlan`.
- Package update apply: allowed only after `OperationPlan`, logs, timeout handling, auth boundaries, stale-plan rejection, and backend revalidation exist.
- Do **not** add whole-system clean, analyze deletion, purge, installer cleanup, arbitrary file deletion, broad privileged commands, status dashboards, or leftover removal until updates are correct and tested.
- Any destructive behavior must be preview-first and backend-validated.

## Commands

Install frontend dependencies:

```bash
npm install
```

Run the desktop app (frontend + backend together):

```bash
npm run tauri dev
```

Build frontend only:

```bash
npm run build
```

Check Rust backend:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## Verification Before Handoff

For normal app changes:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

For safety-sensitive backend changes, add targeted Rust tests before exposing any new command to the frontend.

## Editing Notes

- Keep generated folders out of git: `node_modules/`, `dist/`, and Rust `target/`.
- Keep root `package.json`, `package-lock.json`, and `src-tauri/Cargo.lock` committed.
- Prefer small, typed Tauri commands over broad generic command handlers.
- Keep UI screens operational and app-like; do not turn the first screen into a marketing page.
- Keep docs aligned with the GUI direction.
