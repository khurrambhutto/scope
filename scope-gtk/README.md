# Scope — GTK4 Native Frontend

A native GTK4/libadwaita version of Scope. Same features as the Tauri app:
one unified list of every app and package on the system (APT, Snap, Flatpak,
AppImage, Manual), search and filters, update preview + apply, uninstall
preview + apply, safety deny-list, and a non-blocking background task center.

## Architecture

This frontend does **not** depend on any other folder. The package scanners,
safety deny-list, `OperationPlan`/`PlanStore` preview-apply pipeline, desktop
entry discovery, and icon resolution live in `src/core/` — vendored from the
original Tauri backend, now canonical here. `src/backend.rs` re-exports them
so UI code reads consistently.

```
scope-gtk/
├── src/
│   ├── main.rs        entry point (AdwApplication)
│   ├── backend.rs     #[path] includes of the shared Tauri-free core
│   ├── bridge.rs      tokio→GLib bridge + Backend service (mirrors commands/)
│   ├── app.rs         shared App handle + UI state
│   ├── window.rs      shell, sidebar, apps/tasks pages, wiring
│   ├── packages.rs    package list: scan flow, filters, search, rows
│   ├── detail.rs      package detail page
│   ├── ops.rs         preview → confirm → background task flow
│   ├── tasks.rs       task center model + page
│   └── style.css      small style additions over Adwaita
└── data/
    └── com.khurram.scope.gtk.desktop
```

Key guarantees carried over from the Tauri app:

- Every destructive action is preview-first: the dialog shows the exact
  backend-issued plan (versions, privilege, steps) before confirmation.
- Apply only accepts a plan id the backend issued; plans expire after 5
  minutes and are revalidated against a fresh scan before execution.
- Protected packages (kernel, systemd, ubuntu-desktop, …) are blocked.
- Privilege escalation goes through `pkexec` (Polkit) — no passwords handled.
- No raw shell access: all package-manager commands are explicit argv.
- Operations never block the UI: confirm → dialog closes → task runs in the
  background → list refreshes on success.

## Build

Requires GTK4 ≥ 4.12 and libadwaita ≥ 1.4 development packages:

```bash
sudo apt install -y build-essential pkg-config libgtk-4-dev libadwaita-1-dev
```

Then from this folder:

```bash
cargo run --release
```

## Install

```bash
cargo build --release
sudo cp target/release/scope-gtk /usr/local/bin/
sudo cp data/com.khurram.scope.gtk.desktop /usr/local/share/applications/
```

## Not carried over

- App self-update: the Tauri version uses the Tauri updater plugin; the GTK
  build updates through your normal package channels.
- AppImage *update* is a deliberate stub in the shared backend (same as the
  Tauri version) — preview shows the plan, apply reports "not yet
  implemented".

## Tests

```bash
cargo test
```

Covers the bridge helpers (icon URL decoding, Manual id unpacking) plus every
test in the shared core modules (safety deny-list, icon protocol, desktop
entry parsing, scanner smoke test).
