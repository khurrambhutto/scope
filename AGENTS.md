# Scope

Linux desktop app that unifies installed packages and apps across APT, Snap, Flatpak, and AppImage into one list with preview-first uninstall and update. Product story and install steps are in [README.md](README.md). Primary target is Ubuntu (`.deb` first).

## Stack

- Tauri v2 shell. Rust backend in `src-tauri/`, React 19 + TypeScript in `src/`, Vite 7 build. Webview is WebKitGTK.
- `tokio` for async process execution with per-command timeouts. DTOs are `serde` in Rust, mirrored field-for-field (snake_case) in `src/shared/types/`.
- App self-update via `tauri-plugin-updater` + `tauri-plugin-process`.

## Commands

```bash
npm install                                  # frontend deps
npm run tauri dev                            # run the app
npm run build                                # type-check + build frontend
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml operations::   # one module
```

## Repository map

```
src/                      React frontend
  app/                    app shell, top-level composition only
  features/packages/      unified list, filters, search, inline detail panel
  features/uninstall/     uninstall preview + confirmation dialog
  features/update/        per-package update preview + confirmation dialog
  features/updater/       Scope self-update banner (distinct from package updates)
  shared/api/             typed invoke wrappers, the only place that calls invoke
  shared/types/           TypeScript DTOs matching Rust
  shared/components/      reusable UI (AppIcon, Select, Logo)
src-tauri/src/            Rust backend
  commands/               thin Tauri command handlers, orchestration only
  scanner/                per-source scanners (apt.rs, snap.rs, flatpak.rs, appimage.rs)
  desktop_entries/        .desktop discovery and parsing
  icons/                  icon theme resolution and the scope-icon:// protocol
  operations/             preview/apply flows: probe.rs, uninstall.rs, update.rs, PlanStore
  safety/                 deny-lists for packages and paths
  system/                 command execution, timeouts, pkexec
  package.rs              InstalledPackage plus source/scope enums
docs/                     GitHub Pages site
```

## Module rules

- Feature logic lives in domain modules. `App.tsx`, `lib.rs`, and `main.rs` only compose and wire.
- Tauri commands are thin wrappers: look up state, delegate to `scanner/` / `operations/` / `safety/`, return DTOs.
- Frontend reaches the backend only through `shared/api/` wrappers. Changing a Rust DTO means updating its TypeScript twin in the same change.
- A feature that fits no existing module gets a new small module instead of growing a file past a few hundred lines.
- Keep screens operational and app-like, never marketing pages.

## Product rules

Scanner filters are deliberate product choices:

- The list shows user-relevant apps and tools, never OS internals.
- APT lists manual installs only (`apt-mark showmanual`). Auto-installed dependencies stay hidden.
- Snap hides runtimes and bases (`core*`, `snapd`, `bare`, `gtk-*`, `gnome-*`, `*-gtk3`). Snap entries never default to GUI; a matching non-terminal `.desktop` entry promotes them.
- Flatpak scans user and system as separate installs. Keys are `flatpak:user:<id>` and `flatpak:system:<id>`, and the DTO's `install_scope` is the single source of truth for which scope every later command uses. Never re-guess scope at preview or apply time.
- AppImage walks `/opt`, `/usr/local/bin`, `~/Applications`, `~/apps`, `~/AppImages`, `~/Downloads`, `~/.local/bin` for ELF+`AI` magic files. `safety::check_path` must allow exactly the same directories.

Enrichment is a layer on top of package data, not a replacement: `.desktop` entries supply display names, categories, and icons for GUI apps. Non-GUI packages show a source-colored initials icon and keep full source metadata.

Commands that previews display and apply runs:

| Source | Uninstall | Update |
| --- | --- | --- |
| APT | `pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y <pkg>` | `pkexec env DEBIAN_FRONTEND=noninteractive apt install -y <pkg>` |
| Snap | `pkexec snap remove <pkg>` | `pkexec snap refresh <pkg>` |
| Flatpak user | `flatpak uninstall -y --user <id>` | `flatpak update -y --user <id>` |
| Flatpak system | `pkexec flatpak uninstall -y --system <id>` | `pkexec flatpak update -y --system <id>` |
| AppImage | `gio trash <path>` with manual trash fallback | stub in `operations/update.rs` |

Icon contract: the webview renders icons only from `scope-icon://localhost/<path>` URLs produced by `icons::icon_url`. The frontend never touches the filesystem and holds no `fs`/`shell` Tauri permissions.

## Safety

Always:

- Preview first for every destructive action. Preview builds an `OperationPlan`; apply accepts only a `plan_id` from `PlanStore` (5-minute TTL, single-use).
- Revalidate before executing (`operations::probe` plus the operation's `revalidate`): package present, version unchanged for updates, deny-list still clear. Fail closed when state cannot be verified.
- Escalate with `pkexec` through `system::run_elevated`. Scope never handles passwords.
- Delete files through Trash (`gio trash`, manual `~/.local/share/Trash` fallback).

Never:

- Pass frontend-provided strings to `sh -c` or any shell. Typed `Command` argv only.
- Edit dpkg/apt databases directly. Always go through package-manager commands.
- Bypass `safety/`. The deny-list runs at preview and again at revalidation.
- Remove or update a package the deny-list protects (critical APT packages, kernel images, `lib*` heuristics, Snap runtimes, AppImage paths outside the allow-list).

Ask first:

- Any broad privileged command, arbitrary file deletion, or capability outside the phase fence below.

## Phase fence

Done: unified package view, uninstall (preview + apply, all sources), update (preview + apply for APT/Snap/Flatpak), self-updater. AppImage auto-update is an explicit stub.

Until updates are complete and tested, do not build whole-system clean, disk analyze, purge, leftover removal, or status dashboards. Those wait behind this fence.

## Definition of done

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
cargo test  --manifest-path src-tauri/Cargo.toml
```

- All three pass and the diff contains only the requested change.
- Safety-sensitive backend changes (probe, revalidate, deny-list, PlanStore) ship with targeted Rust tests in the same change.
- A changed command or DTO is reflected in `src/shared/types/`.
- `package.json`, `package-lock.json`, and `src-tauri/Cargo.lock` stay committed.
- Commit style is conventional: `feat:`, `fix:`, `chore:`, `docs:`, `style:`.
- Update this file in the same change as any convention it describes.
