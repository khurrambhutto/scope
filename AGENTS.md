# Scope Agent Notes

## What Is Scope

Scope is a desktop app for Linux — inspired by [Mole](https://github.com/tw93/Mole) for macOS — that gives users a single place to see, update, and uninstall every app on their system regardless of how it was installed.

Linux distributions spread packages across APT, Snap, Flatpak, AppImage, and manual installs. Users have no unified view and must remember which tool installed what. Scope solves that.

**Target platform:** Ubuntu (`.deb` first), with plans to support other Debian-based distros and eventually Fedora/Arch families.

## Architecture

- **Frontend:** React + TypeScript in `src/`.
- **Desktop shell:** Tauri v2 + Rust in `src-tauri/`.
- **Package scanning & operations:** Rust backend in `src-tauri/src/`.
- **Website / docs:** static files in `docs/`.

## Phase 1 — Unified Package View, Update & Uninstall

Phase 1 is the current focus. It covers three capabilities:

### 1. See All Installed Packages in One Place

Scan every package source and display a unified list:

| Source   | How to list installed packages                  |
| -------- | ----------------------------------------------- |
| APT/dpkg | `dpkg-query -W -f '${Package}\t${Version}\t${Status}\n'` |
| Snap     | `snap list`                                     |
| Flatpak  | `flatpak list --app --columns=application,name,version,origin` |
| AppImage | Scan `~/Applications/` and `~/.local/bin/` for `.AppImage` files |

Each entry shows: name, version, source, installed size, and description.

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

## Future Phases (After Phase 1 Is Solid)

These features come later, inspired by Mole's broader toolkit:

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
- Package update/uninstall preview: allowed.
- Package update/uninstall apply: allowed only with preview-first flow, `pkexec` auth, package protection deny-list, timeout handling, and backend revalidation.
- Do **not** add clean, analyze, status, leftover removal, broad file deletion, or arbitrary privileged commands until phase 1 (view + uninstall + update) is correct and tested.

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
