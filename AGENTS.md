# Scope Agent Notes

## Current Direction

Scope is now a root Tauri v2 desktop app. Do not restore the old Rust TUI architecture.

- Frontend: React + TypeScript in `src/`.
- Desktop/backend: Tauri v2 + Rust in `src-tauri/`.
- Package scanning logic lives under `src-tauri/src/scanner/`.
- Package models live in `src-tauri/src/package.rs`.
- The roadmap and safety model live in `plan.md`.

## Phase Guardrails

The app is currently in phase one of the GUI migration:

- Package scanning is allowed.
- Update availability scanning is allowed.
- Destructive commands must not be exposed from the frontend yet.
- Do not add uninstall, cleanup, delete, purge, or privileged commands until `OperationPlan`, path policy, logs, timeout handling, and auth boundaries exist.
- If destructive behavior is implemented later, it must be preview-first and backend-validated.

## Safety Rules

- The frontend must call typed Tauri commands with `invoke`.
- Do not expose raw shell, filesystem, or broad Tauri plugin authority to the webview.
- Do not pass frontend-provided strings to `sh -c`.
- Package-manager changes must go through package-manager commands, not direct package database edits.
- User-owned file deletion should route through Trash by default once deletion exists.
- Protected Linux roots and credentials directories must be rejected by shared path policy before any apply path exists.

## Commands

Install frontend dependencies:

```bash
npm install
```

Build frontend:

```bash
npm run build
```

Check Rust backend:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
```

Run the desktop app:

```bash
npm run tauri dev
```

## Editing Notes

- Keep generated folders out of git: `node_modules/`, `dist/`, and Rust `target/`.
- Keep root `package.json`, `package-lock.json`, and `src-tauri/Cargo.lock` committed for reproducible app builds.
- Prefer small, typed Tauri commands over broad generic command handlers.
- Keep UI screens operational and app-like; do not turn the first screen into a marketing page.
- Keep docs aligned with the GUI direction. Avoid describing Scope as a TUI.

## Verification Before Handoff

For normal app changes, run:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

For safety-sensitive backend changes, add targeted Rust tests before exposing any new command to the frontend.
