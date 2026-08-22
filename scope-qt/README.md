# Scope — Qt Edition

Native Qt6/C++ rewrite of the Scope desktop app (originally Tauri v2 + React).
Mole for Linux: see, update, and uninstall every app on your system from one place.

## Feature parity with the Tauri app

| Capability | Status |
|---|---|
| Unified package list (APT/Snap/Flatpak/AppImage/Manual) | done |
| Icon resolution (freedesktop icon theme + whitelist image provider) | done |
| Desktop-entry enrichment (display names, GUI classification) | done |
| Uninstall preview + apply (OperationPlan, PlanStore, stale rejection) | done |
| Update preview + apply | done |
| Safety deny-lists (critical APT packages, snap runtimes, path guards) | done |
| Non-blocking background operations with task center | done |
| pkexec privilege escalation (no password handling) | done |

## Architecture

```
src/
  core/                  # UI-free library (scope_core), fully unit-testable
    package/Package.*    # InstalledPackage, sources, scopes, keys
    system/System.*      # QProcess runner: capture/which/runElevated, timeouts
    safety/Safety.*      # deny-lists, protected-path checks (preview + reapply)
    desktop/DesktopEntry.* # .desktop discovery, INI parser, matching index
    icons/IconResolver.* # freedesktop icon theme lookup, memoized; served-path whitelist
    scanner/             # AptScanner / SnapScanner / FlatpakScanner /
                         # AppImageScanner + ScanService (concurrent scans)
    operations/          # OperationPlan, PlanStore (TTL one-shot),
                         # UninstallOp / UpdateOp (preview/revalidate/apply)
  models/                # PackageListModel (virtualized, in-model filter/search)
  app/                   # PackagesController, OperationController, TaskModel
  icons/IconProvider.*   # image://scopeicon/ provider — serves ONLY backend-resolved paths
qml/                     # views only (Main, PackagesPage, DetailDrawer,
                         #  PreviewDialog, TaskCenterPage)
tests/                   # QtTest suites: safety, parsers, plan store + smoke_scan harness
```

Ported invariants from the Rust/Tauri version:

- The UI never touches the filesystem or shells out. All destructive actions go
  through `operations/` with preview → confirm → revalidate → apply.
- Plans are single-use with a 5-minute TTL (`PlanStore::take`); apply always
  re-runs a fresh scan and revalidates before executing anything.
- Protected plans are shown to the UI but never issued, so they can never be applied.
- Icons are only served through the whitelisted `image://scopeicon/` provider.

## Build

Requires Qt 6.5+ (tested on 6.11), CMake 3.24+, Ninja. A local toolchain lives
in `.conda-env/` (micromamba env with qt6-main) so no root is needed:

```bash
.conda-env/bin/cmake -B build -S . -G Ninja \
    -DCMAKE_PREFIX_PATH=$PWD/.conda-env -DCMAKE_BUILD_TYPE=Release
.conda-env/bin/cmake --build build
```

Run:

```bash
./build/scope-app
```

Headless screenshot (CI/testing):

```bash
QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software ./build/scope-app --screenshot ui.png
```

## Tests

```bash
.conda-env/bin/ctest --test-dir build --output-on-failure
```

- `test_safety` — critical-package deny-list, kernel/lib rules, snap runtime
  rules, protected-path guards (allowed roots, `.app` bundles, user desktop files).
- `test_parsers` — desktop Exec parsing (env prefixes, quotes, field codes),
  .desktop parsing/locale fallback, flatpak size strings, percent encoding.
- `planstore` — one-shot take, TTL staleness, pruning.
- `smoke_scan` — real scan + preview plans against the live machine (manual).

## Safety model (unchanged)

- APT: `pkexec env DEBIAN_FRONTEND=noninteractive apt remove/install -y <pkg>`
- Snap: `pkexec snap remove <snap>` / `pkexec snap refresh <snap>`
- Flatpak: scope comes from the package DTO — user installs run unprivileged
  (`flatpak uninstall/update -y --user`), system installs via pkexec.
- AppImage: `gio trash` (with manual trash fallback); Manual targets are
  re-checked against path guards at apply time.
- Deny-list: ubuntu-desktop/systemd/apt/dpkg/kernel images/gtk libs…,
  `linux-image-*`, shared libraries, and snap runtime/base snaps are blocked.
