# Scope

Linux desktop app that unifies installed packages and apps across APT, Snap, Flatpak, AppImage, locally discovered Steam games and JetBrains Toolbox IDEs, and verified user tools from uv, pipx, Cargo, npm, and Bun into one list with preview-first uninstall and update for supported package-manager sources. Product story and install steps are in [README.md](README.md). Primary target is Ubuntu (`.deb` first).

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
    scanner/              per-source scanners (apt.rs, snap.rs, flatpak.rs, appimage.rs, steam.rs, jetbrains_toolbox.rs)
    desktop_entries/      .desktop discovery and parsing
    icons/                icon theme resolution and scope-icon:// URLs
    operations/           preview/apply flows, APT simulation, PlanStore
    safety/               deny-lists for packages and paths
    system/               command execution, timeouts, pkexec
    snap_details/         read-only Snap revisions, snapshots and bounded data sizes
    package_details/      shared footprint DTOs, bounded measurement, and per-source inspection
    tool_managers/        uv/pipx/Cargo/npm/Bun receipts, rooted identity, update checks,
                          ownership validation, and shared preview/apply commands
    package.rs            InstalledPackage plus source/scope enums
  theme.rs                palette and formatting helpers
  ui/                     screen state and composition
    app_view.rs           header, filters, virtualized list, inline detail, dialogs
    filters.rs            search, view toggle, selects, rescan
    row.rs / detail.rs    list row + inline detail panel
    detail_transition.rs  measured inline detail expansion and collapse
    package_details.rs    shared lazy type/path/size breakdown and loading rows
    package_detail_state.rs inspection cancellation, request identity and short cache
    dialog.rs             uninstall/update preview and confirmation
    operation_footer.rs   inline footer progress and dismissible results
    settings.rs           app information, project links, self-update confirmation
    updater.rs            self-update notice and lifecycle
    title_bar.rs          client-side title bar
    resize_zones.rs       invisible window edge/corner bands that start resizing
    text_input.rs         search field
    widgets.rs            banners, empty/loading states
    package_list_model.rs scan replacement and stable-key list behavior
    operation_controller.rs request identity and apply exclusivity
assets/                   SVG icons (filter, refresh, window controls), title-bar
                          logo chip, app icon tile (scope-icon.svg, scope.png)
docs/                     GitHub Pages site
```

## Module rules

- Feature logic lives in domain modules. `main.rs` and `app_view.rs` only compose and wire.
- `backend.rs` is a thin wrapper: look up state, delegate to `domain/scanner/` / `domain/operations/` / `domain/safety/`, return DTOs.
- A feature that fits no existing module gets a new small module instead of growing a file past a few hundred lines.
- Keep screens operational and app-like, never marketing pages.
- Uninstall/update previews use a modal. Applying and results appear at the bottom only while an operation is active or has a dismissible result, so browsing stays available; only one package operation may apply at a time. There is no idle footer. The app count appears in the search bar only, and Show all sits to its right in the filter bar.
- The title-bar hamburger opens Settings with app/version information, project links, issue reporting, and an explicit update check. Self-updates from Settings or the update notice require confirmation before installation; unsupported installations link to a download. Check failures stay distinct from an up-to-date result.
- The brand mark is a single-colour pure red (`#e10600`) telescope with a white lens glint; `docs/scope-mark.svg` is its source of truth. `assets/scope-logo.svg` places it on a white disc for the title bar and site header, and `assets/scope-icon.svg` places it on a warm white tile for the installed app icon, README, favicon, and `assets/scope.png`. Public copy names the product "Scope for Linux" in titles, headings, metadata, and package descriptions; "Scope" alone is fine in running text.
- UI colors come from `theme.rs`: dark red surfaces, burgundy borders, coral emphasis. Kit-owned controls use `theme::init`; actions share the pill button in `ui/widgets.rs`. Keep source colors confined to app icons and source indicators.
- Package sources are exhaustive contracts: a `PackageSource` addition updates scanner registration, source availability/cache defaults, desktop ownership lookup, listing/filter/theme matches, safety, operation targets, probes, and update/uninstall handlers. Read-only sources must reject both operations in every route. All sources use the shared pill dropdown in `ui/filters.rs` and `ui/widgets.rs`. Its Others option groups discovery-only Steam, AppImage, Desktop, and JetBrains Toolbox apps; AppImage, Desktop, and JetBrains Toolbox also retain their individual filters.
- Use bundled Work Sans for all UI text, including paths and commands. Regular, Medium, Semibold, and Bold fonts and their OFL license live in `assets/fonts/`; `theme::init` registers them. Breakdown labels are semibold, paths are quieter, and sizes align right. Confirmation metadata shares the breakdown's label width, row padding, and spacing tokens in `theme.rs`.
- List app names use Work Sans Medium. Expanded removable rows show an estimated `Removes ~<size>` summary beside Uninstall, excluding retained, unverified, and informational items. Partial estimates carry `+`; unavailable measurements stay explicit. Omit the generic classic-Snap coverage note from the breakdown, while retaining actual inspection warnings and incomplete-size notes.
- Every source uses the same inline details panel, immediate source-appropriate placeholders, streaming measurements, 30-second cache, and measured-height easing. Expired complete details stay visible during refresh. Request identity includes the stable package key and raw version; Flatpak user/system keys remain distinct. Share a two-inspection worker limit, preserve cancellation and reduced motion, and keep Snap uninstall previews on fresh complete inspection.
- Inspect only attributable installed footprints. APT measures dpkg-listed files individually, groups them by location, and marks conffiles kept by normal remove; never walk their shared parent directories or guess home-directory data. Flatpak resolves the deployment with the DTO's explicit scope, excludes shared runtimes, and reports current-user `~/.var/app/<id>` data as kept. AppImage reports its file and existing exact `.home`/`.config` portable sidecars. Desktop reports its launcher and resolved executable. Local-source rows are informational and never enable uninstall/update. Unattributable data stays explicitly unknown; no inspection launches apps, elevates privileges, or changes package-manager state.

## GPUI notes

- Run `cargo run`. Linux needs GPUI's system deps; notably `libfontconfig1-dev` for font discovery and `libxkbcommon-x11-dev`, because GPUI links `-lxkbcommon-x11` on Linux even in Wayland-only builds.
- Depend on `gpui-kit` alone (it brings the pinned `gpui-pre` snapshot); never list `gpui` separately. Import UI types via `use gpui_kit::*` / `gpui_kit::prelude::*`. Call `gpui_kit::init` once at startup for the theme; open the window with plain `cx.open_window` (no Base `Root`) to stay borderless — kit's `Root` draws a 1px `WindowBorder` frame on Linux. Overlay components (dropdowns, popovers, toasts) need `Root` and are out until the frame question is revisited.
- The crate pins `[lints]` in its `Cargo.toml` (`unsafe_code = "forbid"`, deny `dbg_macro`/`todo`/`unimplemented`).

## Product rules

Scanner and list filters are deliberate product choices:

- The default list shows user-relevant apps and tools, not OS internals; `Show all` is the explicit inventory override.
- APT scans manual installs only (`apt-mark showmanual`), then the main list keeps recognizable GUI apps and public CLI tools from user-facing package sections. Libraries, development-library packages, metapackages, service-only packages, CLI-only admin/database components, and unclassified packages stay hidden.
- The main list omits protected APT/Snap packages. Detected AppImages and user-local desktop apps remain visible, but their uninstall actions are disabled until Scope manages their installation source. Safety checks still run independently at preview and apply time.
- The filter bar's `Show all` toggle bypasses app/tool classification and reveals every row returned by package scanners plus discovered user-local desktop apps. It never bypasses `safety/`; protected rows have a disabled uninstall action. APT still scans manual packages only and Snap/Flatpak runtimes remain excluded at scan time.
- Snap hides runtimes and bases (`core*`, `snapd`, `bare`, `gtk-*`, `gnome-*`, `*-gtk3`). A matching visible `.desktop` entry classifies a snap as GUI/CLI according to its `Terminal` flag; a `/snap/bin/<snap-name>` command can classify it as CLI.
- Flatpak scans user and system as separate installs. Keys are `flatpak:user:<id>` and `flatpak:system:<id>`, and the DTO's `install_scope` is the single source of truth for which scope every later command uses. Never re-guess scope at preview or apply time.
- Missing Flatpak scope is an error at preview, probe, and apply. Never default it to system scope.
- uv, pipx, Cargo, npm, and Bun scans represent whole tool installations, not one row per exposed command. Stable keys include source, root hash, and manager package/environment name. Capture an absolute manager executable and explicit roots/environment in `ToolInstallation`; retain proxy basenames for Cargo and mise. Preview and apply use that context, not a fresh PATH or root guess. Manage only verified user-owned roots inside HOME, without pkexec.
- npm and Bun scan only global installations for the currently selected manager, never project dependencies or a home-directory local `node_modules`. npm uses native global JSON inventory and its explicit prefix; Bun reads the configured global manifest and installed package manifests, never `bun pm ls -g`. Record npm's Node runtime and Bun's global package/bin directories. Scoped package names have source-specific validation; executable names remain strict. Reject conflicting/missing launchers, linked package directories, system-owned roots, and manager packages. Registry update checks use native npm `outdated` JSON and Bun `info <name>@<requirement> version --json`; preserve Bun constraints and trust, while explaining native npm global range loss. Detected Git/local/alias sources have unavailable registry updates. Revalidate global roots, runtime, settings, shared receipts, installed versions, and command ownership before apply. Inspect only the package's own directory; shared dependencies and caches stay unattributed.
- uv scans persistent tool receipts, excluding uvx caches and project environments. Update checks use read-only `uv pip compile --no-build` with stored requirements, constraints, overrides, and options; apply uses native `uv tool upgrade`, preserving installation constraints. An unavailable resolution is not an up-to-date result.
- pipx scans JSON metadata and retains suffixed environment identities and pip options. Updates upgrade the main package without requesting injected-package upgrades; dependency resolution and shared packaging maintenance still follow pipx behavior. Preview explains version-constraint removal and lists injected packages removed with the environment. Explicit pins block updates, not uninstall.
- Cargo scans tracked binary crates, cross-checking `.crates2.json` build settings against `.crates.toml` ownership. Never infer installs from `cargo --list` or update them with `cargo update`. Rebuilds retain origin, version range, selected binaries, features, profile, and target, and never force overwrite. Git revisions are included in raw `version`, with the package release in `display_version`, so unchanged release labels still invalidate stale plans and details. Update discovery supports crates.io and tracked Git branches; fixed revisions remain fixed.
- Git/local Python sources, local Cargo sources, and custom Cargo registries have explicit unavailable-update notes until their release checks are supported. Incomplete/untracked receipts and global pipx installs are not managed. Keep failed/unknown update checks distinct from up to date; bound source update checks to 30 seconds with two query workers. Re-probe after tool operations before reporting a changed version.
- Tool inspection measures dedicated uv/pipx environments, npm/Bun package directories, or individually owned Cargo binaries. Tool measurements stream partial byte totals with a 15-second/250,000-entry bound, share the two-inspection worker limit, and retain cancellation. Commands already counted in a package/environment show Included; retained shared data shows Not measured; unattributable dependencies and personal data show Unattributed. Actual read failures show Unavailable. Keep hard-linked storage caveats explicit and retain partial `+` estimates. Never walk shared bin directories, caches, interpreters, toolchains, or guessed personal data. Verify exposed command ownership, reject conflicting/symlinked environments, and deduplicate every owned launcher by executable path. Tool previews show the rooted command and removal effects; sensitive pip options are redacted.
- The optional `tool_managers::live_tests::disposable` tests install, update, and remove fixtures only in private temporary roots. Ordinary `cargo test` remains hermetic. User tool updates have a 30-minute command timeout; timeout stops the operation's unprivileged build process group.
- AppImage scans `/opt`, `/usr/local/bin`, `~/Applications`, `~/apps`, `~/AppImages`, `~/Downloads`, `~/.local/bin` for ELF+`AI` magic files. Valid AppImages appear in the main list, but uninstall and update actions remain unsupported; `safety::check_appimage` denies every AppImage path, so preview yields a protected plan and apply-time revalidation fails closed.
- Visible launchers in the user's applications directory that no APT/Snap/Flatpak/AppImage scanner owns appear as `Desktop` apps. They are discovery-only and protected from uninstall/update. APT package file lists associate `.desktop` launchers and icons with package IDs when their names differ.
- Steam reads installed manifests from native Steam aliases/XDG roots and Flatpak Steam roots, including every reachable `libraryfolders.vdf` library. It uses binary `appinfo` `common.type`: `game`/`demo` are `Game`, `application` is `Gui`, and `tool` or missing/unknown metadata stays out of the default list until **Show all**. Exact Steam desktop launchers are owned by Steam to prevent duplicate Desktop rows. Steam derives update state locally from a valid, nonzero numeric manifest `StateFlags`: `UpdateRequired` is mask `0x2` (decimal 2, bit offset 1); with that mask, paused takes precedence, then running, downloading, staging, or committing means in progress, while started alone remains pending. Missing, malformed, or zero flags are `Unknown`; valid flags without `UpdateRequired` are `NoUpdateRecorded`, not confirmation that a game is up to date. Do not infer a target build or fabricate `update_version`. `steam_update_status` must retain its serde default so cached packages without that field remain unknown. It refreshes only on a normal scan or rescan and never checks Steam online. Steam details use the shared inline panel, with informational unknown game-file and user-data sizes; Scope does not inspect Steam footprints. Steam remains read-only: Scope never updates or uninstalls it, and **Open in Steam** opens `steam://nav/games/details/<appid>`.
- JetBrains Toolbox discovers IDEs read-only from bounded `product-info.json` and channel metadata under XDG and HOME defaults, including legacy and custom roots. Stable keys use canonical installation roots; raw versions include release and build while display versions use the release. Exact installation executable paths own matching desktop launchers, preventing duplicate Desktop rows. Toolbox has a dedicated filter and is also in Others; every operation gate rejects it. **Open Toolbox** resolves a valid installed `jetbrains-toolbox.desktop` manager launcher in the user/system XDG applications directories and invokes `gio launch` with its absolute desktop-file path only after the explicit row click. Scanning never launches Toolbox; launch failures surface through the app error banner, and no elevation or mutation occurs. Inline details report unknown IDE files and user data without measurement.

Enrichment is a layer on top of package data, not a replacement: visible `.desktop` entries supply display names, categories, and icons for GUI apps. CLI tools use a source-colored initials icon; packages with no reliable app/tool classification stay out of the main list.

Snap version labels may use `display_version` from installed app metadata. VS Code reads a bounded `package.json` from the exact scanned revision, falling back to Snap's version when unavailable. Keep `version` unchanged for operation plans, probes, and revalidation; never launch apps to discover their version or invent release numbers for Git-based Snap versions.

Commands that previews display and apply runs:

| Source | Uninstall | Update |
| --- | --- | --- |
| APT | `pkexec env DEBIAN_FRONTEND=noninteractive apt remove -y <pkg>` | `pkexec env DEBIAN_FRONTEND=noninteractive apt install -y <pkg>` |
| Snap | `pkexec snap remove <pkg>` | `pkexec snap refresh <pkg>` |
| Flatpak user | `flatpak uninstall -y --user <id>` | `flatpak update -y --user <id>` |
| Flatpak system | `pkexec flatpak uninstall -y --system <id>` | `pkexec flatpak update -y --system <id>` |
| uv tool | `uv tool uninstall <pkg>` in the recorded tool/bin roots | `uv tool upgrade <pkg>` in those roots |
| pipx user | `pipx uninstall <environment>` in the recorded home/bin/man roots | `pipx upgrade <environment>` with recorded pip options |
| Cargo user | `cargo uninstall --root <root> <pkg>` | `cargo install --root <root>` with the original source and build settings |
| npm global user | `npm uninstall --global --prefix <prefix> <pkg>` with the recorded Node runtime | `npm update --global --prefix <prefix> <pkg>` |
| Bun global user | `bun remove --global <pkg>` in the recorded global/bin roots | `bun update --global <pkg>` in those roots |
| AppImage | not supported yet — protected plan, no command runs | not supported yet — protected plan, no command runs |
| Steam | none — read-only | none — use Open in Steam |
| JetBrains Toolbox | none — read-only | none — Open Toolbox launches the manager only on click |

Snap removal details use compact type/path/size rows grouped by removal effect inside the expanded app row and preview modal. Removed rows have no tick marks or "Ready to remove" heading. Inline details ease their measured height when opening and closing, including after lazy Snap inspection finishes, and respect reduced-motion settings. Other markers describe effects and are not file-selection controls. Snap details have no size-column header or empty heading row. Confirmation metadata and Snap details share label-column widths, row padding, and spacing, without per-row dividers. Omit the empty recovery-snapshot message and the generic backup and size footnotes; keep inspection warnings and incomplete-size notes. Snap removal details are informational. Load them lazily from the local snapd API, measure only managed data directories with bounded workers, and refresh them at uninstall preview. Show incomplete or unavailable sizes explicitly. Never sum them into guaranteed freed space, treat paths as deletion targets, or enable purge/snapshot deletion. Normal `snap remove` may create a recovery snapshot; existing snapshots remain.

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

Done: unified package view, uninstall (preview + apply, all sources), update (preview + apply for APT/Snap/Flatpak and verified user uv/pipx/Cargo/npm/Bun installs), self-updater. AppImage auto-update is an explicit stub.

Until updates are complete and tested, do not build whole-system clean, disk analyze, purge, leftover removal, or status dashboards. Those wait behind this fence.

## Definition of done

```bash
cargo check
cargo test
cargo clippy
```

- All of the above pass and the diff contains only the requested change.
- Contributor pull requests must leave the website under `docs/` unchanged; website contributions are not accepted. App documentation outside `docs/` remains open to contributions, as described in `CONTRIBUTING.md`.
- Safety-sensitive backend changes (probe, revalidate, deny-list, PlanStore) ship with targeted Rust tests in the same change.
- `Cargo.lock` stays committed.
- Releases use plain `v<version>` tags and `Scope v<version>` titles. Update Cargo metadata, the lockfile, README, and website version references together.
- The release workflow publishes amd64 `.deb` and x86-64 AppImage packages plus `SHA256SUMS` and `SHA256SUMS.minisig`. Pushing the version tag builds and publishes the release.
- Commit style is conventional: `feat:`, `fix:`, `chore:`, `docs:`, `style:`.
- Update this file in the same change as any convention it describes.
