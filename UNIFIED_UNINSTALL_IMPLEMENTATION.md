# Unified uninstall implementation and verification

Branch: `feat/unified-uninstall-data`. Implements the shared managed-source uninstall workflow from [the research](UNIFIED_UNINSTALL_RESEARCH.md). This record separates implemented behavior from unsupported capabilities and verification limits.

## Implementation

- `domain/app_data/` owns reviewed identities, exact namespace rules, current native and hidden-launcher ownership claims, bounded discovery, selected IDs, typed native actions, running-writer checks and per-item outcomes. Package names are compared exactly; a similarly named package cannot acquire a legacy profile by stripping a suffix.
- `OperationPlan.cleanup` replaces the AppImage-only selection list. The backend validates the selected subset before any native action. App files remain native manager effects; optional data is a separate reviewed authority.
- APT, Snap, scoped Flatpak, verified uv/pipx/Cargo/npm/Bun and managed AppImage uninstalls use the same preview and result structure. Exact name associations require separate review; incomplete or shared ownership does not authorize removal. Flatpak permission reset is a separate native action with an absence query afterward.
- APT purge, Snap removal and exact scoped Flatpak removal have explicit postconditions. AppImage and extracted removal verify that the installation path is absent before data cleanup. A native failure, residual APT configuration or unknown outcome keeps selected data and rescans. Tool receipts retain their existing postverification.
- Paperling's exact legacy `com.saqla.marklite` roots are found through a reviewed identity rule. Handy, VS Code and Ruff have bounded rules; generic managed-source package/owned-command/launcher associations remain unchecked. Unknown locations are stated rather than treated as empty.
- Dedicated, owned extracted AppDirs directly under `~/.local/opt`, such as Handy's installation, have an explicit scan/preview/probe/apply contract. Other Desktop executables do not gain removal authority. Shared launchers, linked roots, mount boundaries, changed identities and running applications reject this contract.
- The common preview shows native effects and the actual reviewed local installation paths, app data, separate review choices, and kept/unverified items. Long review content scrolls while confirmation remains available. Source-specific update/install confirmation behavior stays separate.
- Checked Trash stages an object privately on its source filesystem, rechecks the stored identity, and moves without overwrite. Trash records preserve the original path. Failure restores the original or reports an explicit retained recovery path; temporary-directory teardown never deletes staged app data.
- Process privacy gaps are recorded by PID and kernel start time before confirmation. They leave data unchecked and require deliberate selection; a known running app always blocks, and new gaps/PID reuse require a fresh preview. Such acknowledged gaps remain warnings in the final result and never prove that no background writer exists.
- Extracted-app file removal uses that same process inspection. Its preview explicitly states that Uninstall acknowledges the displayed gaps for the app files; data still requires separate selection. File ownership inspection remains available while an app runs, and apply checks writers again immediately before moving the installation.
- Running-writer checks use APT-owned executable files, owned launcher targets, recorded tool executables/environments, AppImage identity, exact Snap revision roots and Flatpak application IDs. Interpreter-launched commands are checked by their exact argument paths.
- Results remain available after the installed row disappears. Removed native resources, trashed paths, included children, kept/shared/changed/failed/unknown data remain distinct. Trash relocation is not immediate space reclamation.

## Safety review findings closed

| Finding | Correction and evidence |
| --- | --- |
| A version-like suffix could make `paperling-plugin2` match Paperling | Exact native identities; regressions cover unrelated IDs and deceptive display names. |
| Device IDs alone missed bind mounts | Bounded mountinfo checks and same-device bind-mount fixtures. Shared XDG roots and their ancestors cannot be selected as app data. |
| A path could be replaced between verification and Trash | Stored identity checked before and after private staging; no-overwrite rollback and recovery tests. This is not a claim of complete protection against every concurrent filesystem/mount mutation. |
| Mixed-content folders were described as preserving all documents | Preview states that every file inside a selected folder moves, including saved content, and separates lower-confidence/mixed selections. |
| A valid snapd background launcher had no Exec | Preserve named hidden Snap/DBus background entries as ownership claims, with a regression for prompting-client metadata. The live Paperling preview then offers both legacy roots without that false ownership failure. |
| Disposable manager lifecycle overflowed a normal test stack | GDB localized large nested inventory states in tool verification. Inventory APIs own their futures on the heap; ordinary-stack disposable lifecycle tests pass. No runtime stack-size override is required. |
| Extracted Handy removal could be blocked by unrelated protected processes | Reuse the common previewed PID/start-time process review instead of a second incompatible check. A live read-only preflight now succeeds with visible warnings; missing review, new gaps, PID reuse and known running apps have regressions. |

## Verification

Run from the repository root:

```text
cargo check --locked
cargo build --locked
cargo test --locked --quiet
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo fmt --check
cargo test --locked disposable_ -- --ignored --nocapture
```

The standard suite is hermetic. The optional disposable tests install/update/remove local fixture tools in private roots: Cargo from a dependency-free local Git repository, npm/Bun through a loopback fixture registry, and uv/pipx through fixture wheels. Python fixtures now use a private HOME/cache and pin underlying configured runtimes rather than requiring a mise shim to resolve in an empty HOME. No existing installed user tool is selected by these fixtures.

Final standard verification: **502 passed, 17 ignored, 0 failed**. Check, build, strict all-target/all-feature Clippy, formatting and whitespace checks pass. The three optional disposable lifecycle tests also pass on the normal test-thread stack.

Additional checks:

- A disposable Paperling-profile fixture joins discovery and reviewed selection to the checked production Trash mover, then recreates the profile and verifies that old history is absent while documents and recoverable Trash content remain.
- Live **read-only** metadata checks find the real managed Paperling's two eligible legacy profile roots and recognize Handy's extracted root with its verified launcher. Paperling was running during inspection; that correctly blocked selected cleanup. Protected background-process gaps left its two choices unchecked and were shown as warnings. These checks do not remove those installations or profiles.
- Handy's live read-only removal preflight succeeds after reviewing four protected-process gaps. The check does not move any Handy installation or profile files.
- The optional AppImage install/remove round trip uses a copy of a real AppImage under a temporary HOME. It never executes the AppImage or modifies its original.

Verification logs are retained in `/tmp/scope-unified-disposable-tests.log`, `/tmp/scope-unified-python-tools.log` and `/tmp/scope-unified-appimage-lifecycle.log`. Stack diagnosis is retained in `/tmp/scope-unified-gdb-current.log`; a regression guards against inlining manager inventories back into the worker futures.

## Explicit limits

- Steam and JetBrains Toolbox remain manager handoffs. Unmanaged AppImages and unverified Desktop apps remain discovery-only. Their management contracts were not invented from directory names.
- Snap recovery snapshots are retained; normal removal can create a new one. There is no snapshot deletion, Snap purge, autoremove, whole-system cleaning or cloud/account-data deletion.
- Ownership checks cover current configured native inventories, recorded tool context and reachable desktop claims; arbitrary undiscovered/custom installations and app-created paths cannot be exhaustively enumerated. Exact names are never treated as a universal manifest.
- GUI interaction and privileged APT/Snap/system-Flatpak destructive integration were not exercised against the user's system. Native query/command behavior has injected fixtures; real native tool and disposable AppImage lifecycles are separate checks. The clean-profile fixture is not a launch-and-reinstall test of the actual Paperling GUI.
- Existing AppImage update transport/digest behavior was preserved; a live network AppImage update was not part of this cleanup verification.

These limits must remain visible in product copy and release notes. Passing a package-removal check alone does not establish that every app-created file is gone.
