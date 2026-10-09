# Scope

Linux desktop app that unifies installed packages and apps across APT, Snap, Flatpak, and AppImage into one list with preview-first uninstall and update. Product story and install steps are in [README.md](README.md). Primary target is Ubuntu (`.deb` first).

## Stack

- GPUI shell via the `gpui-kit 0.7` facade (Zed's GPUI snapshot, no webview). Rust binary in `src/`, `Cargo.toml` at root.
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
    operations/           preview/apply flows, APT simulation, PlanStore
    safety/               deny-lists for packages and paths
    system/               command execution, timeouts, pkexec
    snap_details/         read-only Snap revisions, snapshots and bounded data sizes
    package.rs            InstalledPackage plus source/scope enums
  theme.rs                palette and formatting helpers
  ui/                     screen state and composition
    app_view.rs           header, filters, virtualized list, inline detail, dialogs
    filters.rs            search, view toggle, selects, rescan
    row.rs / detail.rs    list row + inline detail panel
    snap_details.rs       lazy Snap breakdown shared with uninstall previews
    snap_detail_state.rs  inspection cancellation, request identity and short cache
    dialog.rs             uninstall/update preview and confirmation
    operation_footer.rs   inline footer progress and dismissible results
    title_bar.rs          client-side title bar
    text_input.rs         search field
    widgets.rs            banners, empty/loading states
    package_list_model.rs scan replacement and stable-key list behavior
    operation_controller.rs request identity and apply exclusivity
assets/                   SVG icons (filter, refresh, logo, window controls)
docs/                     GitHub Pages site
```

## Module rules

- Feature logic lives in domain modules. `main.rs` and `app_view.rs` only compose and wire.
- `backend.rs` is a thin wrapper: look up state, delegate to `domain/scanner/` / `domain/operations/` / `domain/safety/`, return DTOs.
- A feature that fits no existing module gets a new small module instead of growing a file past a few hundred lines.
- Keep screens operational and app-like, never marketing pages.
- Uninstall/update previews use a modal. Applying and results live between the app count and Show all control in the bottom footer so browsing stays available; only one package operation may apply at a time.
- UI colors come from `theme.rs`: dark red surfaces, burgundy borders, coral emphasis. Kit-owned controls use `theme::init`; actions share the pill button in `ui/widgets.rs`. Keep source colors confined to app icons and source indicators.

## GPUI notes

- Run `cargo run`. Linux needs GPUI's system deps; notably `libfontconfig1-dev` for font discovery and `libxkbcommon-x11-dev`, because GPUI links `-lxkbcommon-x11` on Linux even in Wayland-only builds.
- Depend on `gpui-kit` alone (it brings the pinned `gpui-pre` snapshot); never list `gpui` separately. Import UI types via `use gpui_kit::*` / `gpui_kit::prelude::*`. Call `gpui_kit::init` once at startup for the theme; open the window with plain `cx.open_window` (no Base `Root`) to stay borderless — kit's `Root` draws a 1px `WindowBorder` frame on Linux. Overlay components (dropdowns, popovers, toasts) need `Root` and are out until the frame question is revisited.
- The crate pins `[lints]` in its `Cargo.toml` (`unsafe_code = "forbid"`, deny `dbg_macro`/`todo`/`unimplemented`).

## Product rules

Scanner and list filters are deliberate product choices:

- The default list shows user-relevant apps and tools, not OS internals; `Show all` is the explicit inventory override.
- APT scans manual installs only (`apt-mark showmanual`), then the main list keeps recognizable GUI apps and public CLI tools from user-facing package sections. Libraries, development-library packages, metapackages, service-only packages, CLI-only admin/database components, and unclassified packages stay hidden.
- The main list omits protected APT/Snap packages. Detected AppImages and user-local desktop apps remain visible, but their uninstall actions are disabled until Scope manages their installation source. Safety checks still run independently at preview and apply time.
- The footer's `Show all` toggle bypasses app/tool classification and reveals every row returned by package scanners plus discovered user-local desktop apps. It never bypasses `safety/`; protected rows have a disabled uninstall action. APT still scans manual packages only and Snap/Flatpak runtimes remain excluded at scan time.
- Snap hides runtimes and bases (`core*`, `snapd`, `bare`, `gtk-*`, `gnome-*`, `*-gtk3`). A matching visible `.desktop` entry classifies a snap as GUI/CLI according to its `Terminal` flag; a `/snap/bin/<snap-name>` command can classify it as CLI.
- Flatpak scans user and system as separate installs. Keys are `flatpak:user:<id>` and `flatpak:system:<id>`, and the DTO's `install_scope` is the single source of truth for which scope every later command uses. Never re-guess scope at preview or apply time.
- Missing Flatpak scope is an error at preview, probe, and apply. Never default it to system scope.
- AppImage scans `/opt`, `/usr/local/bin`, `~/Applications`, `~/apps`, `~/AppImages`, `~/Downloads`, `~/.local/bin` for ELF+`AI` magic files. Valid AppImages appear in the main list, but uninstall and update actions remain unsupported; `safety::check_appimage` denies every AppImage path, so preview yields a protected plan and apply-time revalidation fails closed.
- Visible launchers in the user's applications directory that no APT/Snap/Flatpak/AppImage scanner owns appear as `Desktop` apps. They are discovery-only and protected from uninstall/update. APT package file lists associate `.desktop` launchers and icons with package IDs when their names differ.

Enrichment is a layer on top of package data, not a replacement: visible `.desktop` entries supply display names, categories, and icons for GUI apps. CLI tools use a source-colored initials icon; packages with no reliable app/tool classification stay out of the main list.

Commands that previews display and apply runs:

| Source | Uninstall | Update |
| --- | --- | --- |
| APT | `pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y <pkg>` | `pkexec env DEBIAN_FRONTEND=noninteractive apt install -y <pkg>` |
| Snap | `pkexec snap remove <pkg>` | `pkexec snap refresh <pkg>` |
| Flatpak user | `flatpak uninstall -y --user <id>` | `flatpak update -y --user <id>` |
| Flatpak system | `pkexec flatpak uninstall -y --system <id>` | `pkexec flatpak update -y --system <id>` |
| AppImage | not supported yet — protected plan, no command runs | not supported yet — protected plan, no command runs |

Snap removal details use compact type/path/size rows grouped by removal effect inside the expanded app row and preview modal. Markers describe effects and are not file-selection controls. Snap removal details are informational. Load them lazily from the local snapd API, measure only managed data directories with bounded workers, and refresh them at uninstall preview. Show incomplete or unavailable sizes explicitly. Never sum them into guaranteed freed space, treat paths as deletion targets, or enable purge/snapshot deletion. Normal `snap remove` may create a recovery snapshot; existing snapshots remain.

Icon contract: icons load only from `scope-icon://localhost/<path>` URLs produced by `icons::icon_url`, decoded by `backend::icon_path` into on-disk paths GPUI renders directly.

## Safety

Always:

- Preview first for every destructive action. Preview builds an `OperationPlan`; apply accepts only a `plan_id` from `PlanStore` (5-minute TTL, single-use).
- Revalidate before executing (`operations::probe` plus the operation's `revalidate`): package present, version unchanged for updates, deny-list still clear. Fail closed when state cannot be verified.
- Simulate every APT transaction with `LC_ALL=C`, reject indirect protected removals, store its fingerprint in the plan, and require the same simulation immediately before apply.
- Escalate with `pkexec` through `system::run_elevated`. Scope never handles passwords.
- Delete files through Trash (`gio trash`, manual `~/.local/share/Trash` fallback).
- Install self-updates only after verifying a minisign-signed SHA-256 manifest. Keep downloads in a private temporary directory.

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
- Releases use plain `v<version>` tags and `Scope v<version>` titles. Update Cargo metadata, the lockfile, README, and website version references together.
- Commit style is conventional: `feat:`, `fix:`, `chore:`, `docs:`, `style:`.
- Update this file in the same change as any convention it describes.
