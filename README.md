# Scope

Scope is a Linux-first Tauri desktop app for package visibility, cleanup planning, and safe maintenance workflows.

The current app is in phase one of the GUI migration:

- Tauri v2 desktop shell at the repository root.
- React frontend in `src/`.
- Rust backend in `src-tauri/src/`.
- Reused package scanners for APT, Snap, Flatpak, and AppImage.
- Read-only package and update scanning commands.
- No destructive cleanup or uninstall command is exposed yet.

## Run

Install the Tauri Linux prerequisites for your distro, then run:

```bash
npm install
npm run tauri dev
```

For a frontend-only build:

```bash
npm run build
```

For backend verification:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

## Project Structure

```text
scope/
├── package.json
├── src/                  # React frontend
├── src-tauri/            # Tauri v2 backend
│   └── src/
│       ├── lib.rs        # Tauri commands
│       ├── package.rs    # Package model
│       └── scanner/      # APT, Snap, Flatpak, AppImage scanners
├── plan.md               # GUI migration and safety roadmap
└── docs/                 # Static website/docs
```

## Safety Direction

Scope should use preview plans before any destructive operation. Package-manager state should be changed through package-manager commands, and file deletion should route through shared path policy plus Trash-backed operations by default.

See [plan.md](plan.md) for the phase roadmap.
