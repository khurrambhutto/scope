# Scope Linux Tauri Plan

## Goal

Build Scope as a Linux-first Tauri desktop app inspired by Mole, with the same broad product surface:

- Safe cleanup with dry-run previews.
- App/package uninstall with leftover discovery.
- Disk analysis with recoverable deletion.
- Project artifact purge.
- Installer-file cleanup.
- System status dashboard.
- Optimization and maintenance tasks.
- History, logs, whitelists, update, and self-remove flows.

This should not be a line-by-line Mole port. Mole is macOS-specific and uses Bash plus Go. Scope should keep the useful product model and safety contracts, then implement them with Rust, Tauri v2, Linux package-manager APIs, XDG paths, systemd tools, and the Freedesktop Trash model.

## Sources Studied

### Mole codebase

- `mole`: top-level shell entrypoint, command dispatch, update check, version/system summary, interactive menu.
- `lib/core/common.sh`: core loader.
- `lib/core/base.sh`: shared constants, UI symbols, defaults, user/path helpers.
- `lib/core/log.sh`: session logs, debug logs, operations log.
- `lib/core/timeout.sh` and `lib/core/timeouts.sh`: centralized timeout wrapper and constants.
- `lib/core/file_ops.sh`: path validation, safe deletion, Trash/permanent modes, dry-run behavior, whitelists.
- `lib/core/app_protection.sh` and `lib/core/app_protection_data.sh`: app, bundle, data, and path protection policy.
- `bin/clean.sh` plus `lib/clean/*`: cleanup orchestration and category implementations.
- `bin/uninstall.sh` plus `lib/uninstall/*`: app scanning, previews, sensitive file handling, batch removal.
- `cmd/analyze/*`: Go disk analyzer, navigation, Trash-backed deletion.
- `cmd/status/*`: Go status dashboard and JSON output.
- `bin/purge.sh`, `lib/clean/project.sh`, `lib/clean/purge_shared.sh`: project artifact discovery and purge.
- `bin/installer.sh`: installer discovery, drift checks, delete plans.
- `bin/optimize.sh`, `lib/optimize/tasks.sh`: maintenance tasks and privileged actions.
- `bin/history.sh`, `lib/core/history.sh`: operation history.
- `docs/SECURITY_DESIGN.md` and `SECURITY_AUDIT.md`: safety model and known risk areas.

### Current Scope state

The old Scope root was a Rust TUI package manager for Linux. The repo has now been restructured as a root Tauri app, and the reusable package-manager logic lives in the Tauri backend:

- `src-tauri/src/package.rs`: package model and sort helpers.
- `src-tauri/src/scanner/*`: APT, Snap, Flatpak, and AppImage scanners.
- `src-tauri/src/lib.rs`: read-only Tauri commands for package and update scanning.
- `src/`: React frontend shell.

The terminal UI has been removed from the root product:

- no root `Cargo.toml` TUI crate.
- no `ratatui` or `crossterm` dependency.

The old nested `scope-gui/` scaffold has been moved to the root and renamed:

- Tauri v2 backend exposes `scan_packages` and `scan_packages_with_updates`.
- `tauri.conf.json` uses `Scope`, `com.khurram.scope`, and a non-null CSP.
- Tauri capabilities are explicit and do not expose shell/filesystem authority.

### External references

- Tauri v2 prerequisites: Linux builds need WebKitGTK, build tools, SSL, appindicator, librsvg, and related distro packages. Source: https://v2.tauri.app/start/prerequisites/
- Tauri v2 frontend-to-Rust calls: backend functions are exposed as `#[tauri::command]` and called from the frontend with `invoke`. Source: https://v2.tauri.app/develop/calling-rust/
- Tauri v2 capabilities: permissions should be declared in `src-tauri/capabilities`, and explicit capabilities should limit which windows can use which APIs. Source: https://v2.tauri.app/security/capabilities/
- XDG Base Directory Specification: default user data/config/cache locations are `~/.local/share`, `~/.config`, and related XDG directories. Source: https://specifications.freedesktop.org/basedir-spec/latest/
- APT maintenance commands: `apt-get clean`, `autoclean`, and `autoremove` are the correct package-manager operations instead of deleting package cache internals directly. Source: https://manpages.debian.org/bookworm/apt/apt-get.8.en.html
- Flatpak uninstall/update commands: `flatpak uninstall`, `--delete-data`, and `--unused` provide supported cleanup paths. Source: https://docs.flatpak.org/en/latest/flatpak-command-reference.html
- Snap update management: `snap refresh`, `snap refresh --list`, and refresh holds are managed through snapd commands. Source: https://snapcraft.io/docs/keeping-snaps-up-to-date
- systemd journal maintenance: `journalctl --disk-usage`, `--rotate`, `--vacuum-size`, and `--vacuum-time` are the safe supported paths. Source: https://man7.org/linux/man-pages/man1/journalctl.1.html
- Rust Trash crate: `trash` moves files to the OS Trash/Recycle Bin equivalent. Source: https://docs.rs/trash/latest/trash/

## Product Principle

Mole's most important feature is not deletion. It is constrained deletion.

Scope should make cleanup feel powerful, but the engine must be conservative:

1. Everything destructive starts as a preview plan.
2. Every file operation passes a shared path policy.
3. User files go to Trash by default.
4. Permanent deletion is opt-in and rare.
5. Package-manager state is changed only through package-manager commands.
6. Privileged work goes through a narrow auth layer.
7. Logs record what Scope planned and what Scope actually changed.
8. The frontend never receives raw arbitrary shell authority.

## Mole Feature Inventory and Scope Translation

| Mole surface | How Mole handles it | Scope Linux/Tauri equivalent |
| --- | --- | --- |
| Main command surface | `mole` dispatches shell commands and shows an interactive menu. | Tauri app with sidebar navigation. Optional `scope` CLI can call the same Rust core for automation. |
| Clean | `bin/clean.sh` orchestrates named sections with `safe_clean`, dry-run, whitelist, and operation logs. | `Clean` page with categories, scan preview, risk labels, per-item selection, and `apply_clean_plan`. |
| Uninstall | Scans apps, protects system/vendor apps, finds leftovers, previews everything, uses Trash by default. | Package manager aware uninstall for APT, Snap, Flatpak, and AppImage. Leftovers are preview-only until explicitly selected. |
| Analyze | Go TUI disk explorer with scanning, navigation, selection, Trash deletion. | GUI disk explorer with treemap/table, large-file view, path policy, and Trash-backed delete. |
| Status | Go dashboard with CPU, memory, disks, network, batteries, thermal, processes, and JSON output. | Linux dashboard using `sysinfo`, `/proc`, `/sys`, systemd, package manager health, disk usage, and JSON command output. |
| Purge | Finds project artifacts in configured roots and deletes selected artifacts. | Reuse the same concept for Linux dev projects with CACHEDIR.TAG and manifest-bound project roots. |
| Installer | Finds `.dmg`, `.pkg`, `.xip`, installer zips, snapshots before deletion, rejects drift. | Find `.deb`, `.rpm`, `.AppImage`, `.run`, installer `.sh`, `.flatpakref`, `.snap`, and archives containing installers. |
| Optimize | Runs macOS maintenance tasks with sudo/test guards. | Linux maintenance tasks via package managers, `journalctl`, `systemctl --user`, desktop database tools, and cache rebuild commands. |
| History | Reads operation and delete logs. | `History` page backed by JSONL logs under XDG state. |
| Whitelist | `~/.config/mole/whitelist` plus default patterns. | `~/.config/scope/whitelist.toml`, GUI editor, category toggles, exact/glob/path rules. |
| Update | Background GitHub/Homebrew version checks. | Respect install source. Use Tauri updater only for AppImage/tarball builds; do not self-update apt/snap/flatpak installs. |
| Remove self | Confirmation and dry-run for removing Mole itself. | `Remove Scope` flow with install-source detection, dry-run, and package-manager-specific instructions. |
| TouchID | macOS sudo convenience. | Linux Polkit integration and privilege status page. No persistent root helper in v1. |
| Completion | Shell completions. | Optional CLI completions if Scope ships a CLI companion. |

## Safety Architecture

### Core Rust modules

Create a core crate or backend module tree that every feature uses:

```text
src-tauri/src/
  commands.rs
  core/
    auth.rs
    logs.rs
    paths.rs
    policy.rs
    plan.rs
    process.rs
    timeout.rs
    trash.rs
    whitelist.rs
  clean/
    mod.rs
    user.rs
    system.rs
    browsers.rs
    dev.rs
    flatpak.rs
    snap.rs
  packages/
    mod.rs
    apt.rs
    snap.rs
    flatpak.rs
    appimage.rs
    protection.rs
  analyze/
    mod.rs
    scanner.rs
  purge/
    mod.rs
  installers/
    mod.rs
  optimize/
    mod.rs
  status/
    mod.rs
  history.rs
```

If the repo is reorganized later, move reusable Rust into `crates/scope-core/` and let both the Tauri app and optional CLI depend on it.

### Plan model

Every destructive operation should return an explicit plan before applying it:

```rust
struct OperationPlan {
    id: String,
    kind: OperationKind,
    created_at: String,
    dry_run: bool,
    summary: PlanSummary,
    items: Vec<OperationItem>,
    warnings: Vec<PlanWarning>,
}

struct OperationItem {
    path: Option<PathBuf>,
    command: Option<Vec<String>>,
    label: String,
    source: ItemSource,
    size_bytes: Option<u64>,
    risk: RiskLevel,
    reversible: bool,
    requires_auth: bool,
    selected_by_default: bool,
    reason: String,
}
```

The UI should never send arbitrary paths straight to delete. It should send a plan id plus selected item ids. The backend revalidates the plan before applying it.

### Path policy

Port Mole's deletion policy to Linux, then make it stricter where Linux is more dangerous.

Reject:

- Empty paths.
- Relative paths.
- Paths with null/control characters.
- Paths containing `..` components.
- Symlinks whose resolved target is protected.
- Paths whose metadata changed between preview and apply.
- Paths outside known cleanup roots unless the user selected them from Analyze.

Always protect:

- `/`
- `/boot`
- `/dev`
- `/efi`
- `/etc`
- `/home` as a whole
- `/lib`, `/lib32`, `/lib64`, `/libx32`
- `/proc`
- `/root`
- `/run`
- `/sbin`
- `/sys`
- `/usr`
- `/var/lib`
- `/var/log/journal` for direct deletion
- package databases such as `/var/lib/dpkg`, `/var/lib/apt`, `/var/lib/snapd`, `/var/lib/flatpak`
- identity and credential directories such as `~/.ssh`, `~/.gnupg`, `~/.pki`, `~/.local/share/keyrings`
- browser profiles except cache directories
- password managers, wallets, keychains, 2FA stores, VPN configs, SSH agents, and secrets directories
- cloud-sync roots such as Dropbox, Nextcloud, OneDrive, Syncthing, and rclone mounts unless user selects individual cache files
- VM and container data roots unless a package-manager command or Docker/Podman prune plan owns the operation

Allow only specific known cleanup roots:

- `~/.cache`
- `$XDG_CACHE_HOME`
- `~/.local/state/*/logs` where safe
- `~/.local/share/Trash`
- `~/.var/app/<app>/cache`
- `~/snap/<snap>/common/.cache`
- `/tmp`
- `/var/tmp`
- `/var/cache/apt/archives` only via `apt-get clean/autoclean`
- systemd journals only via `journalctl --vacuum-*`

### Deletion modes

Default mode:

- Move user-owned files to Trash using the Rust `trash` crate.
- If Trash fails, stop and report the file. Do not silently fall back to permanent deletion.

Privileged/system mode:

- Prefer supported commands such as `apt-get clean`, `journalctl --vacuum-time`, `flatpak uninstall --unused`, or `snap remove`.
- If a root-owned file truly must be removed, require a named backend operation, a preview, Polkit auth, and another path-policy pass.

Permanent mode:

- Hidden behind advanced settings.
- Disabled for broad cleanup categories.
- Never used for Analyze-driven ad hoc cleanup by default.

### Logs

Use XDG locations:

- State: `$XDG_STATE_HOME/scope` or `~/.local/state/scope`.
- Config: `$XDG_CONFIG_HOME/scope` or `~/.config/scope`.
- Cache: `$XDG_CACHE_HOME/scope` or `~/.cache/scope`.

Log files:

- `scope.log`: user-facing app log.
- `debug.log`: debug details.
- `operations.jsonl`: operation sessions and results.
- `delete-log.jsonl`: per-path delete/trash records.

Allow a test-only `SCOPE_NO_OPLOG=1`, but do not expose it in normal UI.

### Auth

Use Polkit through `pkexec` or package-manager native auth prompts:

- `SCOPE_TEST_NO_AUTH=1` disables privileged execution in tests.
- `SCOPE_DRY_RUN=1` prevents apply paths.
- Never keep a long-running privileged daemon for v1.
- Never pass raw frontend strings to `sh -c`.
- Commands are argument vectors, not shell snippets.
- Use timeouts for every external process.

## Tauri Boundary

The frontend should call typed Rust commands with `invoke`. Do not expose raw shell or filesystem plugins to the webview.

Initial Tauri commands:

```rust
#[tauri::command]
async fn scan_packages(filter: PackageFilter) -> Result<Vec<Package>, ScopeError>;

#[tauri::command]
async fn preview_clean(request: CleanRequest) -> Result<OperationPlan, ScopeError>;

#[tauri::command]
async fn apply_plan(plan_id: String, selected_item_ids: Vec<String>) -> Result<OperationResult, ScopeError>;

#[tauri::command]
async fn scan_disk(request: DiskScanRequest) -> Result<DiskScanResult, ScopeError>;

#[tauri::command]
async fn status_snapshot() -> Result<StatusSnapshot, ScopeError>;

#[tauri::command]
async fn read_history(filter: HistoryFilter) -> Result<Vec<HistoryEntry>, ScopeError>;

#[tauri::command]
async fn save_whitelist(config: WhitelistConfig) -> Result<(), ScopeError>;
```

Tauri config changes:

- Rename product to `Scope`.
- Change identifier to `com.khurram.scope`.
- Replace `csp: null` with a real CSP.
- Add explicit capabilities under `src-tauri/capabilities`.
- Keep dialog permissions narrow if added later.
- Avoid shell plugin for v1 unless there is a specific command-level permission model.

## GUI Plan

Build the first screen as the actual app, not a landing page.

Primary navigation:

- Overview
- Clean
- Uninstall
- Analyze
- Optimize
- Purge
- Installers
- Status
- History
- Settings

### Overview

Show the operational state:

- Total reclaimable space from last scan.
- Package updates available.
- Largest safe cleanup categories.
- Journal/cache health.
- Trash size.
- Last operation result.
- Warnings that need user review.

### Clean

Use category rows and a right-side preview panel:

- Category name.
- Estimated reclaimable size.
- Risk label.
- Default selection state.
- Explanation of what is included and excluded.
- Button to inspect files before applying.

Categories:

- User cache
- Temporary files
- Browser caches
- App caches
- Flatpak caches
- Snap caches
- Developer caches
- Container caches
- Package caches
- Logs and journals
- Large-file clues
- System data clues

### Uninstall

Use a package/app table:

- Name
- Source: APT, Snap, Flatpak, AppImage
- Version
- Size
- App type
- Update available
- Installed path
- Risk/protection status

The details drawer should show:

- Exact package-manager command.
- Related data discovered.
- Protected leftovers skipped.
- Running process warning.
- Sensitive data warning.
- Config/data delete options, off by default.

### Analyze

Use a split disk explorer:

- Directory tree.
- Largest files table.
- Type breakdown.
- Breadcrumb navigation.
- Multi-select.
- Trash button.
- Protected-path indicators.

Deleting from Analyze should always route through Trash unless the user uses an advanced permanent action.

### Optimize

Use maintenance cards with explicit commands:

- APT clean/autoclean/autoremove.
- Flatpak unused runtimes.
- Snap refresh status and manual refresh.
- systemd journal vacuum.
- Failed user unit inspection.
- Thumbnail/icon/font cache rebuilds.
- Desktop database refresh.
- Docker/Podman prune, opt-in only.

### Status

Linux status snapshot:

- Distribution and kernel.
- CPU load.
- Memory and swap.
- Disk usage.
- Filesystem types.
- Battery.
- Thermal sensors if available.
- Network interfaces.
- DNS/proxy status.
- Top processes.
- Failed systemd units.
- Package manager health.

### History

Show operation sessions:

- Time.
- Feature.
- Dry-run or applied.
- Items changed.
- Space reclaimed.
- Errors.
- Per-item details.

## Linux Feature Details

### APT

Scan:

- `dpkg-query` for installed packages.
- `apt-mark showmanual` for manually installed packages.
- Desktop files to classify GUI apps.
- `apt list --upgradable` or apt cache policy for updates.

Protect:

- `Essential: yes`.
- Priority `required`.
- `apt`, `dpkg`, `systemd`, kernel, bootloader, desktop session, display manager, network manager, libc, sudo/polkit, package-manager dependencies.

Actions:

- Uninstall with `apt remove` or `apt purge` only after preview.
- Cleanup with `apt-get autoclean`, `apt-get clean`, and `apt-get autoremove`.
- Do not delete `/var/lib/dpkg` or package database files directly.

### Snap

Scan:

- `snap list`.
- `snap refresh --list`.
- `snap refresh --time`.
- Size by snap metadata and mounted revisions where reliable.

Protect:

- `snapd`, `core*`, `bare`, base snaps currently used by installed snaps.

Actions:

- `snap remove <name>`.
- `snap refresh <name>` or global refresh.
- Revision cleanup only through supported snapd settings or commands.

### Flatpak

Scan:

- `flatpak list --app --columns=name,application,version,size,description`.
- `flatpak remote-ls --updates`.
- Distinguish system and user installations.

Protect:

- Runtimes unless unused.
- Pinned runtimes.
- Shared platform runtimes used by apps.

Actions:

- `flatpak uninstall <app>`.
- `flatpak uninstall --delete-data <app>` only if the user explicitly opts into data removal.
- `flatpak uninstall --unused` for unused runtimes.
- `flatpak update` for updates.

### AppImage

Scan:

- `~/Applications`
- `~/AppImages`
- `~/.local/bin`
- `~/Downloads`
- `/opt`
- `/usr/local/bin`

Detect:

- `.AppImage` extension.
- ELF header and AppImage signature where possible.
- Matching desktop entries in `~/.local/share/applications`.
- Matching icons/metainfo if created by AppImage integration tools.

Actions:

- Move AppImage file to Trash.
- Move matching desktop entries/icons to Trash only after preview.
- Never delete arbitrary files by name similarity outside known integration paths.

### Browser caches

Clean only cache directories, not profiles:

- Chromium/Chrome/Brave/Vivaldi cache trees under XDG config/cache.
- Firefox cache2 under profile cache paths.
- Thumbnail/media/GPU caches where safe.

Protect:

- Cookies.
- History.
- Sessions.
- Login data.
- Extensions.
- Bookmarks.
- Profiles.

### Developer caches

Supported categories:

- npm, pnpm, yarn, bun, corepack.
- pip, uv, poetry, conda.
- Cargo registry/git cache, not toolchains by default.
- Go build cache and module download cache.
- Gradle and Maven caches.
- Docker and Podman builder caches, opt-in.
- JetBrains, VS Code, Zed, Cursor cache/logs, not settings.
- AI tool caches, conservative by default.

Protect:

- Credentials.
- Config.
- SSH/GPG.
- API keys.
- Ollama models unless explicitly selected in a model manager.
- Active agent sessions.
- Worktrees unless an advanced cleanup says they are disposable.

### Project purge

Targets:

- `node_modules`
- `target`
- `build`
- `dist`
- `.next`
- `.nuxt`
- `.output`
- `.svelte-kit`
- `.astro`
- `.turbo`
- `.parcel-cache`
- `coverage`
- `.venv`
- `venv`
- `.pytest_cache`
- `.mypy_cache`
- `.ruff_cache`
- `.tox`
- `.nox`
- `__pycache__`
- `.gradle`
- `vendor`
- `bin`
- `obj`
- `.dart_tool`
- `.zig-cache`
- `zig-out`

Rules:

- Detect project roots by manifests.
- Do not cross project boundaries accidentally.
- Respect `CACHEDIR.TAG`.
- Use age thresholds.
- Preview everything.
- Trash by default.

### Installer cleanup

Extensions:

- `.deb`
- `.rpm`
- `.AppImage`
- `.run`
- installer `.sh`
- `.flatpakref`
- `.snap`
- archives containing installers

Roots:

- `~/Downloads`
- `~/Desktop`
- `~/Documents`
- browser download directories
- Telegram/Discord download directories where discoverable

Safety:

- Snapshot path, size, inode, and mtime before apply.
- Refuse if changed.
- Trash by default.
- Skip unreadable archives.

### Optimize

Safe default tasks:

- APT autoclean.
- APT autoremove preview and explicit apply.
- Flatpak unused runtime cleanup.
- systemd journal vacuum by time/size.
- Failed user unit report.
- Thumbnail cache refresh.
- Font cache refresh if `fc-cache` exists.
- Desktop database refresh if `update-desktop-database` exists.

Opt-in tasks:

- Docker prune.
- Podman prune.
- Package manager full upgrade.
- Kernel cleanup.
- Removing old Snap revisions.

## Frontend Implementation Shape

Recommended stack:

- Tauri v2 backend in Rust.
- React or Svelte frontend.
- TypeScript for strict command payloads.
- TanStack Table or equivalent for package/file tables.
- Zustand, Jotai, Svelte stores, or another small state layer.
- CSS variables for theme tokens.
- Icons through a normal icon library.

Important UI behaviors:

- Every cleanup page has `Scan`, `Preview`, and `Apply` states.
- Applying a plan locks the selection and shows streaming progress.
- Destructive buttons include an icon and a short command label.
- Risk labels are visible in tables and details drawers.
- Protected items are shown as skipped, not hidden.
- Confirmation modals quote counts and sizes, not generic warnings.
- Errors should include the failed path/command and the safety reason.

## Implementation Roadmap

### Phase 1: Rename and bridge the Tauri app

- Move the Tauri app to the repository root.
- Rename product metadata to `Scope`.
- Replace the starter command with read-only package scan commands.
- Move or share package models and scanners into the Tauri backend.
- Add frontend shell with sidebar and package table.

Exit criteria:

- GUI can list APT/Snap/Flatpak/AppImage packages.
- No destructive command exists yet.
- Tauri capabilities are explicit.
- CSP is not null.

### Phase 2: Safety foundation

- Add `core::policy`.
- Add `core::trash`.
- Add `core::logs`.
- Add `core::timeout`.
- Add `core::auth`.
- Add `OperationPlan` and `OperationResult`.
- Add tests for dangerous paths and dry-run behavior.

Exit criteria:

- No feature can delete without a plan.
- Path validator rejects protected Linux roots.
- Trash deletion works in a temp-home test.
- Logs are written under XDG state.

### Phase 3: Package uninstall and updates

- Implement protected package lists.
- Add uninstall preview for each package source.
- Add running-process detection.
- Add leftover discovery, off by default.
- Add update flow.

Exit criteria:

- APT essential packages are blocked.
- Snap base packages are blocked.
- Flatpak data removal is opt-in.
- AppImage deletion goes to Trash.

### Phase 4: Clean

- Implement user cache, browser cache, Flatpak cache, Snap cache, dev cache, temp files, and package cache previews.
- Add category defaults and risk levels.
- Add whitelist config and GUI editor.

Exit criteria:

- `Scan Clean` returns a complete plan.
- Apply revalidates every item.
- Browser profile data is protected.
- Package caches use package-manager commands.

### Phase 5: Analyze

- Replace basic disk scan with cancellable, streaming scan.
- Add largest files and directory tree.
- Add protected-path markers.
- Add Trash action for selected files.

Exit criteria:

- Analyzer can scan `$HOME`.
- Deletion from Analyze routes through Trash.
- Protected roots cannot be selected for delete.

### Phase 6: Purge and installers

- Implement project root discovery.
- Add purge target definitions.
- Add installer scan and drift checks.
- Add configurable scan roots.

Exit criteria:

- Project artifacts are grouped by project.
- Installers are deleted only if unchanged since preview.
- Both pages support dry-run and history.

### Phase 7: Status and optimize

- Add Linux status snapshot.
- Add package manager health checks.
- Add journal and package cache maintenance.
- Add optional Docker/Podman prune previews.

Exit criteria:

- Status page works without root.
- Optimize tasks explain exact commands.
- Privileged tasks fail closed in `SCOPE_TEST_NO_AUTH=1`.

### Phase 8: Polish, settings, release

- Add settings for whitelists, retention, Trash/permanent mode, scan roots, and auth behavior.
- Add operation history page.
- Add update logic based on install source.
- Add packaging for AppImage, deb, and rpm.

Exit criteria:

- AppImage build works.
- Deb/rpm install paths are documented.
- Self-update is disabled for package-manager installs.
- Full test suite passes.

## Test Strategy

Rust unit tests:

- Path policy corpus.
- Whitelist exact/glob/child behavior.
- Plan serialization.
- Package command parsing from fixtures.
- AppImage detector.
- Browser/dev cache detectors.

Integration tests:

- Temporary XDG home.
- Stub `apt`, `dpkg-query`, `snap`, `flatpak`, `pkexec`, `journalctl`, `gio`.
- Dry-run apply must not mutate files.
- Trash mode must move into test Trash.
- Protected paths must remain untouched.

Frontend tests:

- Package table render.
- Clean preview selection.
- Confirmation modal counts.
- History rendering.
- Error display.

Static checks:

- Deny direct recursive deletion outside `core::trash` and approved package-manager commands.
- Deny raw `sh -c` command construction.
- Deny frontend shell plugin permissions unless explicitly reviewed.

Manual verification:

- Run on a disposable Linux VM first.
- Test with APT-only, Flatpak-only, Snap-enabled, and AppImage-heavy setups.
- Test GNOME and KDE Trash behavior.
- Test non-root user, admin user, and missing `pkexec`.

## Biggest Risks

- Treating Linux package-manager state like normal files. Avoid this by using package-manager commands.
- Deleting application data when the user expected cache cleanup. Keep data deletion off by default.
- Following symlinks into protected areas. Resolve and validate before deletion.
- Giving the frontend broad shell permissions. Keep OS authority in typed Rust commands.
- Cleaning active developer tools or AI agents. Skip running tools and protect credentials/session state.
- Overbroad AppImage leftover matching. Only remove exact integration artifacts after preview.
- Direct journal deletion. Use `journalctl --vacuum-*`.
- Polkit prompts blocking tests. Add `SCOPE_TEST_NO_AUTH=1` and command stubs.

## First Concrete Tasks

1. Finish verifying the root Tauri phase-one bridge.
2. Implement `OperationPlan` before adding any destructive command.
3. Implement Linux path policy and tests.
4. Implement Trash-backed AppImage uninstall preview first because it is the simplest destructive flow.
5. Add APT/Snap/Flatpak uninstall previews only after package protection is in place.

## Non-goals for v1

- No automatic unattended cleanup.
- No root daemon.
- No direct deletion of package databases.
- No registry-like deep leftover deletion by fuzzy name.
- No cleaning cloud-sync folders by default.
- No deleting browser profiles, credentials, sessions, or cookies.
- No removing Docker volumes/images without explicit opt-in.
- No permanent deletion as the default.
