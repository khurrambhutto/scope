# Scope

Linux desktop app that unifies installed packages and apps across APT, Snap, Flatpak, and AppImage into one list with preview-first uninstall and update. Product story and install steps are in [README.md](README.md). Primary target is Ubuntu (`.deb` first).

## Stack

- GPUI shell (Zed's GPUI, no webview). Rust binary in `src/`, `Cargo.toml` at root.
- `tokio` for async process execution with per-command timeouts. DTOs are `serde` in Rust.
- History: the domain was vendored from the old Tauri backend (`src-tauri/`, React in `src/`). Tauri is archived on tag `tauri-final` / branch `archive/tauri`.

## Commands

```bash
cargo run
cargo check
cargo test
cargo test operations::   # one module
cargo clippy
```

## Repository map

```
src/                      GPUI app
  main.rs                 composition and wiring only
  backend.rs              thin orchestration: scan + persistent cache,
                          plan preview/apply with streamed logs,
                          scope-icon:// URL decoding
  domain/                 backend domain (vendored from old src-tauri/src/)
    scanner/              per-source scanners (apt.rs, snap.rs, flatpak.rs, appimage.rs)
    desktop_entries/      .desktop discovery and parsing
    icons/                icon theme resolution and scope-icon:// URLs
    operations/           preview/apply flows: probe.rs, uninstall.rs, update.rs, PlanStore
    safety/               deny-lists for packages and paths
    system/               command execution, timeouts, pkexec
    package.rs            InstalledPackage plus source/scope enums
  theme.rs                palette and formatting helpers
  ui/                     screen state and composition
    app_view.rs           header, filters, virtualized list, inline detail, dialogs
    filters.rs            search, view toggle, selects, rescan
    row.rs / detail.rs    list row + inline detail panel
    dialog.rs             uninstall/update dialogs
    title_bar.rs          client-side title bar
    text_input.rs         search field
    widgets.rs            banners, empty/loading states
assets/                   SVG icons (filter, refresh, logo, window controls)
docs/                     GitHub Pages site
```

## Module rules

- Feature logic lives in domain modules. `main.rs` and `app_view.rs` only compose and wire.
- `backend.rs` is a thin wrapper: look up state, delegate to `domain/scanner/` / `domain/operations/` / `domain/safety/`, return DTOs.
- A feature that fits no existing module gets a new small module instead of growing a file past a few hundred lines.
- Keep screens operational and app-like, never marketing pages.

## GPUI notes

- Run `cargo run`. Linux needs GPUI's system deps; notably `libxkbcommon-x11-dev`, because GPUI links `-lxkbcommon-x11` on Linux even in Wayland-only builds.
- The crate pins `[lints]` in its `Cargo.toml` (`unsafe_code = "forbid"`, deny `dbg_macro`/`todo`/`unimplemented`).
- Not yet ported: the self-updater banner.

## Product rules

Scanner filters are deliberate product choices:

- The list shows user-relevant apps and tools, never OS internals.
- APT lists manual installs only (`apt-mark showmanual`). Auto-installed dependencies stay hidden.
- Snap hides runtimes and bases (`core*`, `snapd`, `bare`, `gtk-*`, `gnome-*`, `*-gtk3`). Snap entries never default to GUI; a matching non-terminal `.desktop` entry promotes them.
- Flatpak scans user and system as separate installs. Keys are `flatpak:user:<id>` and `flatpak:system:<id>`, and the DTO's `install_scope` is the single source of truth for which scope every later command uses. Never re-guess scope at preview or apply time.
- AppImage walks `/opt`, `/usr/local/bin`, `~/Applications`, `~/apps`, `~/AppImages`, `~/Downloads`, `~/.local/bin` for ELF+`AI` magic files so they appear in the unified list. AppImage uninstall and update are deliberately not supported yet: `safety::check_appimage` denies every AppImage path, so preview yields a protected plan and apply-time revalidation fails closed.

Enrichment is a layer on top of package data, not a replacement: `.desktop` entries supply display names, categories, and icons for GUI apps. Non-GUI packages show a source-colored initials icon and keep full source metadata.

Commands that previews display and apply runs:

| Source | Uninstall | Update |
| --- | --- | --- |
| APT | `pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y <pkg>` | `pkexec env DEBIAN_FRONTEND=noninteractive apt install -y <pkg>` |
| Snap | `pkexec snap remove <pkg>` | `pkexec snap refresh <pkg>` |
| Flatpak user | `flatpak uninstall -y --user <id>` | `flatpak update -y --user <id>` |
| Flatpak system | `pkexec flatpak uninstall -y --system <id>` | `pkexec flatpak update -y --system <id>` |
| AppImage | not supported yet — protected plan, no command runs | not supported yet — protected plan, no command runs |

Icon contract: icons load only from `scope-icon://localhost/<path>` URLs produced by `icons::icon_url`, decoded by `backend::icon_path` into on-disk paths GPUI renders directly.

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
cargo check
cargo test
cargo clippy
```

- All of the above pass and the diff contains only the requested change.
- Safety-sensitive backend changes (probe, revalidate, deny-list, PlanStore) ship with targeted Rust tests in the same change.
- `Cargo.lock` stays committed.
- Commit style is conventional: `feat:`, `fix:`, `chore:`, `docs:`, `style:`.
- Update this file in the same change as any convention it describes.
