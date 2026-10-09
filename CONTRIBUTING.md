# Contributing to Scope

Thanks for helping improve Scope. Please read the [Code of Conduct](CODE_OF_CONDUCT.md) before taking part.

## Before you start

- Check the [open issues](https://github.com/khurrambhutto/scope/issues) to see whether the bug or idea is already tracked.
- For larger changes, open an issue first so we can agree on the approach.
- Read [AGENTS.md](AGENTS.md) for the module layout, safety rules, and product rules. Scanner and list filters are deliberate choices, so changes to them need a clear reason.

## Development setup

Scope is a Rust GPUI app for Linux. Install Rust stable and the system dependencies listed in the [README](README.md#install), then run:

```bash
cargo run
```

## Checks

Run all three before opening a pull request:

```bash
cargo check
cargo test
cargo clippy
```

Safety-sensitive backend changes (probe, revalidate, deny-list, PlanStore) must ship with targeted Rust tests in the same change.

## Pull requests

- Create a branch from `main` and keep each pull request focused on one change.
- Use conventional commit messages: `feat:`, `fix:`, `chore:`, `docs:`, or `style:`.
- Fill in the pull request template, including how you tested the change.
- Do not commit build output or local tooling files.
- Keep `Cargo.lock` committed when dependencies change.

## Reporting bugs and security issues

- Bugs and feature requests: use the issue templates.
- Security vulnerabilities: do not open a public issue. Follow [SECURITY.md](SECURITY.md).
