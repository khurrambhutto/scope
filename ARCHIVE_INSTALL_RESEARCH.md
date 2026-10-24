# Portable app installation in Scope for Linux

Researched 23 October 2026. Proposal only; no installer has been implemented. Repository observations refer to commit `434eca0` and the inspected working tree. The existing modification to `src/domain/operations/update.rs` was left untouched.

## Recommendation

Add **Install from archive** for ready-to-run Linux apps. Treat the result as a Scope-managed **Portable app**, with a dedicated installation directory, a generated app-menu launcher, and a durable installation receipt. Reuse Scope's preview, single-use plan, progress footer, scan, and details conventions.

Start with `.tar.gz` and `.tgz` containing a native executable and ordinary files/directories. Support `.tar.xz` and `.zip` in later changes using the same installation model. This directly addresses the Diary release shown in the conversation while keeping the first extraction policy small. The actual Diary archive has not been inspected, so its compatibility remains unverified.

Install only for the current user, without elevation. Keep source builds, installation scripts, automatic dependency installation, automatic updates, and replacement of an existing installation outside the first version. A checksum identifies an archive; it does not establish that its publisher is trustworthy.

These are design recommendations, not requirements imposed by the Linux specifications. Facts and specifications supporting the choices are cited below.

## Why this approach

An archive does not supply a universal installation recipe. Archive readers provide entries and bytes, while Linux desktop integration is a separate specification with its own launcher format and command-line rules. Scope must define which archive layouts it supports rather than treating arbitrary contents as install instructions. [S1, S4, S7]

Generating a `.deb` would add a second package-building workflow and privileged installation. It would also require deciding dependency and maintainer-script behavior that the archive does not establish. For this feature, copying a verified bundle into a private user directory is the smaller implementation. Scope would own that installation; APT would not track it.

Gear Lever is a useful product reference: it integrates AppImages into the menu, keeps them organized, and trashes the AppImage, launcher, and icons on removal. Its documented format is AppImage, so it is a reference for the experience, not a general archive-installation backend. [S10]

Calling system `tar` or `unzip` would save parser integration initially, but Scope would still need entry-type validation, extraction limits, candidate detection, receipts, and recovery. Prefer maintained Rust readers with a single Scope-owned extraction policy. Do not implement a tar or ZIP parser from scratch.

## What Scope already provides

- `src/domain/operations/mod.rs`: `OperationPlan`, `OperationTarget`, and `PlanStore`. Plans expire after five minutes and are consumed by `take`. The store currently holds installed-package update/uninstall plans only.
- `src/backend.rs`: preview/apply dispatch and streamed `OpMsg` progress. The current apply path always probes an already installed package, so a new installation needs a separate verification branch.
- `src/ui/operation_controller.rs`: stale-request rejection and UI apply exclusivity. Keep these behaviors for installation, and add backend serialization so an alternate caller or second process cannot bypass exclusivity.
- `src/domain/scanner/mod.rs` and `desktop_entries/mod.rs`: source registration, enrichment, and synthesis of unmanaged desktop rows. A generated launcher must belong to the portable installation explicitly or it can appear a second time as an unmanaged Desktop app.
- `src/domain/package_details/`: shared size measurement, cancellation, and a two-inspection worker limit. Reuse these for installed portable files.
- GPUI's locked `gpui-pre` 0.3.7 exposes `App::prompt_for_paths` and `PathPromptOptions`. The installed source documents asynchronous selection, cancellation, and Linux picker errors. Use it through `gpui-kit`; no new UI framework is needed.
- This checkout has AppImage discovery and Scope's own updater, but no general local `.deb`, AppImage, or archive installation flow to extend.
- `AGENTS.md` requires preview-first actions, apply-time revalidation, Trash removal, exhaustive source integration, and small domain modules. Although it requires Trash, the inspected production domain code has no general Trash implementation to reuse yet.

## User flow

1. An **Install from archive** pill action opens the native file picker for one file. Cancellation closes it without an error. File type is checked from content as well as the suffix.
2. Scope copies the selected regular file into private staging while calculating SHA-256, then extracts and inspects that snapshot. Show a cancellable preparation state. Preparation never starts the downloaded app.
3. Suggest a main executable from native executable candidates. Prefer a unique plausible application binary, using bundled launcher metadata only as a hint. Never silently choose among several candidates. If no supported executable exists, explain that the archive may contain source code or an unsupported launcher.
4. Show a modal with editable display name, selected executable, optional icon, and an explicit **Run in terminal** choice. Native ELF metadata cannot determine GUI versus CLI. A bundled launcher can supply a suggestion, subject to confirmation.
5. Show the installation directory, launcher location, measured unpacked size, and that updates must be installed manually. State that publisher identity has not been verified when appropriate. Confirming stores the exact choices in a backend plan.
6. Apply accepts only the issued `plan_id`, revalidates staging and destination ownership, installs, and reports progress/results in the existing footer. Browsing remains available. Rescan after success.

The chosen executable must remain inside the staged bundle. Do not offer an arbitrary command string, external executable, shell arguments, automatic opening, file associations, or autostart in the first version. CLI apps may receive a terminal launcher; adding a command to `PATH` is a separate feature.

## Supported contents and extraction policy

The first version should deliberately reject layouts it cannot preserve safely. Some Electron, Java, Python, and shell-wrapper distributions will therefore be unsupported. Explain the reason rather than installing an incomplete bundle.

- Allow regular files and directories only. Reject symbolic links, hard links, devices, FIFOs, sparse entries, and unsupported extension records. Do not silently skip required files. Later support for internal relative links needs its own containment tests.
- Reject absolute paths, every `..` component, empty file names, NULs, control characters, duplicate normalized file paths, and file/directory conflicts. Handle the common `./` prefix consistently. Permit a repeated directory header only when it names the same directory and cannot change its permissions. Reject ambiguous collisions rather than renaming them.
- Validate the effective paths and sizes produced by the reader, including supported PAX/GNU overrides. Do not validate one parser's listing and extract with a different parser. Reject unsupported PAX features and conflicting size/path information.
- Create a new private `0700` staging directory. Use directory-relative filesystem access, exclusive file creation, and a no-follow policy. Never extract over an existing installation or into a shared Downloads directory.
- Normalize permissions. Use private readable files/directories, preserve required owner-execute bits on bundle files, and make the explicitly selected native executable runnable. Do not apply archived owner/group IDs, setuid/setgid bits, ACLs, capabilities, or xattrs. Avoid recursively making every file executable.
- Preserve the bundle's directory layout. Resources and libraries often need to remain next to the executable. Recognizing a single outer directory can simplify display, but must not discard siblings.
- Bound compressed bytes, total decoded bytes including metadata, emitted file bytes, entry count, path depth, parser memory, and elapsed processing time. Verify checksums and malformed/truncated streams. Define treatment of concatenated gzip members and trailing tar data explicitly; do not accept a partially parsed archive as a complete installation. `MultiGzDecoder` handles multiple gzip members, but does not itself enforce Scope's tar or resource policy. [S13]

Suggested initial budgets are 512 MiB input, 2 GiB decoded data, 50,000 entries, depth 32, and 60 seconds of preparation. These are proposed product limits, not measured safe defaults. Benchmark against representative bundles and malicious fixtures before choosing final constants. Offer a clear unsupported/too-large result instead of retrying with unlimited resources.

Use a killable archive worker process if enforcing the proposed hard deadline. The same Scope executable can expose an internal worker entry point before GUI initialization, keeping packaging to one binary. The parent owns the input snapshot, staging directory, deadline, and bounded progress/result channel; the worker only extracts and inspects. Bound worker memory with a reviewed safe platform wrapper. A separate process alone is not a filesystem or security sandbox.

Do not claim that wrapping `spawn_blocking` in a Tokio timeout stops extraction. Tokio explicitly documents that an already running blocking task cannot be aborted. If a cooperative worker is chosen instead, describe its deadline honestly and test cancellation inside all potentially long reader operations. [S12]

## Readers and filesystem access

Use the maintained `tar` crate with `flate2` for the first format. At research time the tar documentation resolves to **0.4.46**. RustSec reports both a symlink-related permission-change vulnerability and a PAX size interpretation vulnerability in versions through 0.4.44, patched in 0.4.45. Resolve a currently patched release, commit `Cargo.lock`, and recheck advisories at implementation time. These citations do not constitute a complete dependency audit. [S4, S5, S6]

Use tar for parsing, with Scope validating each effective entry and streaming accepted regular-file contents through bounded writers. Keep directory creation and permission changes under Scope's policy. Tar's `Entry::unpack` documentation explicitly warns that it does not prevent writing outside the destination. `unpack_in` provides more checks, but the crate documents concurrent mutation of the destination as outside its threat model. [S4]

Prefer `cap-std::fs::Dir` or an equivalent reviewed directory-relative interface for the new filesystem module. This fits the crate's `unsafe_code = "forbid"` rule and limits path resolution to an opened directory. Validate roots and reject links independently; capability-oriented filesystem access does not authenticate receipts or make downloaded apps trustworthy. [S4, S14]

For a later ZIP reader, use `enclosed_name` plus Scope's stricter rules and entry-type checks. Do not use `mangled_name` to repair unsafe paths. ZIP's documented `enclosed_name` allows some internal `..` paths; Scope's first-version policy can reject them. Its documentation also exposes `is_symlink`, which must be checked separately. Review the selected ZIP release and features when adding it rather than pinning a speculative dependency now. [S7]

For `.tar.xz`, add a reviewed maintained decoder behind the same bounded tar policy. Decoder selection, native-library packaging, and memory behavior remain implementation research for that increment.

## Detecting the executable and metadata

Read bounded ELF headers and necessary program headers. Check magic, class, byte order, machine architecture, executable/loadable structure, and interpreter information where present. Exclude obvious shared libraries and helper binaries from automatic selection. Native PIE executables can use `ET_DYN`, so accepting only `ET_EXEC` would reject legitimate apps. ELF defines these fields, but recognizing an ELF file does not establish runtime compatibility. [S8]

Reject PE/Mach-O files, incompatible architectures, and source-only bundles. Do not require `EI_OSABI` to equal Linux: the ELF specification permits an unspecified ABI value. Report the platform assessment as best effort and leave dependency compatibility unverified. [S8]

Never run `--version`, `--help`, a bundled installer, or `ldd` against the candidate. The Linux `ldd` manual explicitly warns against using it on untrusted executables because it can execute code. No automatic dependency repair follows inspection. [S9]

Read bundled `.desktop` files only as bounded metadata. Suggest a name, terminal setting, and an icon within the bundle; do not copy their `Exec`, `TryExec`, `Path`, actions, MIME registrations, environment assignments, or D-Bus activation settings wholesale. An absent icon should use Scope's existing fallback. Limit icon file size and decoded dimensions; an icon failure must not prevent installation.

Do not invent a release version from a filename. Show an unknown version when necessary. Store a separate opaque installation revision for stale-plan/detail identity and change it when the installed content or selected launcher changes. Unknown registry update support must remain distinct from an up-to-date result.

## Installed layout and ownership receipt

Use a validated, absolute `$XDG_DATA_HOME`, defaulting to `~/.local/share` when unset or empty, and ignore relative environment values as the XDG specification requires. Under the initial safety policy, require installation and launcher destinations to be verified user-owned locations inside HOME. If a valid custom XDG location lies outside that policy, explain the unsupported location instead of silently writing elsewhere. [S2]

Suggested layout:

```text
$XDG_DATA_HOME/scope/portable/.pending/<random-id>/
$XDG_DATA_HOME/scope/portable/apps/<install-id>/receipt.json
$XDG_DATA_HOME/scope/portable/apps/<install-id>/payload/<original-layout>
$XDG_DATA_HOME/scope/portable/transactions/<transaction-id>.json
$XDG_DATA_HOME/applications/<scope-owned-desktop-id>.desktop
```

Use an opaque random installation ID, not a user-supplied name, for paths and the stable package key `portable:<install-id>`. Allocate a launcher ID in Scope's application namespace, check for collisions, and never overwrite another entry. Desktop file identity and user precedence make collisions more than a cosmetic issue. [S3]

The versioned receipt should contain the installation ID, selected executable and working directory relative to the payload, display metadata, terminal choice, archive SHA-256, installed content manifest/fingerprint, launcher name and expected content hash, icon reference, architecture assessment, installation revision, and transaction state. Paths in a receipt are untrusted data: validate them and derive all removal destinations from verified roots, never from arbitrary absolute strings.

Store the receipt with the installation, independently of `scan-cache.json`. A cache is not proof of ownership. The portable scanner should enumerate bounded receipts, validate their roots and required files, and surface damaged installations as protected with a clear reason. Do not adopt arbitrary Desktop or AppImage rows because a filename looks familiar.

## Plans, commit, and recovery

Keep the external domain interface small: prepare an archive, preview a chosen installation, apply an issued plan, scan receipts, and inspect a managed installation. Candidate selection must reference a backend preparation ID and candidate ID; the UI must not supply an arbitrary source or destination path at apply time.

Preparation owns the staged snapshot and extracted manifest. The final preview binds their fingerprints, user choices, installation ID, launcher bytes, intended destinations, and limits. A changed choice requires a new preview. Before apply, reacquire the backend mutation guard and revalidate staging, manifest, selected executable, root ownership, destination absence, and launcher collision state. Never re-open the Downloads path as the authoritative payload after preview.

Extend the store to hold a typed action such as `PlannedAction::Package(OperationPlan)` or `PlannedAction::PortableInstall(PortableInstallPlan)`, sharing the five-minute TTL and single-use consumption. Preserve existing package-operation behavior. Do not fabricate an installed package to satisfy today's probe path. Install verification checks absence and ownership; package update/uninstall verification checks presence and the recorded revision.

Attach staging lifetime to the preparation/plan owner. Add explicit cancellation, expiry cleanup, size accounting, and a small concurrent-preparation limit. The existing PlanStore sweeps expired entries only on issue/take; that alone is insufficient for timely cleanup of large pending bundles. Crash recovery may clean up only proven Scope-owned staging, never arbitrary paths found in a journal.

Commit sequence:

1. Write and sync a transaction journal and the prepared receipt. Take a filesystem-backed mutation lock as well as the in-process guard. Recheck absence under that lock.
2. Move the prepared installation into its final directory on the same filesystem. Use no-clobber commit semantics; ordinary `rename` must not be assumed to guarantee no replacement.
3. Publish a freshly generated launcher with no-clobber semantics, after payload and receipt are durable. This makes the app visible only when its target exists.
4. Mark the transaction committed, sync affected directories as appropriate, report success, and rescan.

The directory move, launcher publication, and journal update are separate writes. Do not describe the whole installation as atomic. Recovery must reconcile each interruption point, including a published launcher before the final journal update, without overwriting or deleting user-modified files. Repeated recovery must be safe. On an unresolvable conflict, retain the installed files and report what remains.

If a previous installation has the same archive hash, offer to show it rather than silently duplicating it. A different hash with the same display name is not evidence of an update. First-version replacement remains unsupported.

## Launcher generation

Generate a small desktop entry from verified values: `Type=Application`, escaped `Name`, an absolute `Exec` referring only to the selected installed executable, an optional absolute `Icon`, the confirmed `Terminal` setting, and optionally a verified working-directory `Path`. Use an internal ownership marker, but never treat that marker alone as proof of ownership.

Serialize general desktop-entry string escapes and the additional Exec quoting layer correctly. Exec uses double-quote rules, special escaping for backslash, quote, dollar sign and backtick, and `%%` for a literal percent. The specification prohibits an equals sign in the executable path. Unsupported paths must fail explicitly rather than producing a launcher that starts the wrong program. Never substitute shell quoting or emit `sh -c`. [S1]

The current `exec_program` and `exec_binary` helpers split quoted strings at the next quote and do not implement all these rules. Add a shared, tested tokenizer for the supported generated subset, or use exact receipt-based launcher ownership without reparsing Exec. Verify discovery still handles the generated entry and does not synthesize a duplicate unmanaged row. Avoid a broad rewrite of third-party launcher parsing in this feature.

No executable bit or file-manager trust metadata is required by this proposal for a launcher installed into the applications directory; validate normal menu behavior on target desktops. `update-desktop-database` builds a MIME-handler cache. It is not a general application-menu refresh command, so do not require it for a launcher with no MIME declarations. Test menu discovery on GNOME and KDE rather than promising synchronous refresh everywhere. [S3, S11]

## Removal, updates, and details

Ship removal of receipt-backed portable apps with installation. Preview the dedicated installation directory and owned launcher, revalidate both immediately before apply, and move them to Trash. Add the repository-required `gio trash` flow and a specification-compliant fallback. Do not use recursive permanent deletion as a fallback for installed apps. Trash metadata and failure handling are specified independently from package installation. [S15]

Removal of the directory and launcher is not a single transaction either. Journal the steps and retain enough ownership information for a safe retry if only one move succeeds. Never claim complete success after a partial failure, and never overwrite a changed launcher while attempting rollback.

Protect invalid receipts, replaced roots, missing ownership evidence, and changed launchers. Recheck the installation revision and relevant filesystem state between preview and apply. User modifications to the dedicated payload must invalidate an older removal preview; a fresh preview must describe any additional content it would move. Do not follow links out of the bundle or remove guessed `~/.config`, `~/.local/share`, caches, or personal documents. Explain that files created inside the dedicated installation directory are included if the user confirms its removal.

Use the shared inline details panel and its worker limit, 30-second cache, request identity, streaming measurements, and reduced-motion behavior. Measure only the owned directory and launcher. Keep external app-created data explicitly unattributed. Bind detail identity to the installation revision, and never use size rows as deletion instructions.

Reject portable updates in every backend route until a verified replacement flow exists. Show “Manual updates” or “Update checks unavailable”, not “Up to date”. Generic archives lack a standard trusted update source; automatic release lookup should not be guessed from a filename or GitHub-looking string.

## Module changes and implementation order

Create `src/domain/portable_apps/` with private modules for archive reading, executable candidates, roots/receipts, launcher generation, transaction/recovery, and removal. Export a narrow interface; keep GPUI and backend wrappers concerned with orchestration. Put the scanner adapter in `src/domain/scanner/portable.rs` and UI selection/confirmation in a new small `src/ui/install.rs`.

Add `PackageSource::Portable` and a corresponding managed target/DTO. Update scanner registration, availability defaults and serde/cache compatibility, exact desktop ownership, listing, shared filters, theme indicators, safety, probes, uninstall, update rejection, and detail dispatch. Group Portable under the existing Others filter initially. Keep unmanaged Desktop and AppImage protections intact.

Recommended delivery order:

1. Implement the bounded `.tar.gz` reader/worker, candidate detection, and hostile-archive tests in temporary roots. Confirm the exact supported layouts before adding a visible action.
2. Implement receipts, generated launchers, transaction recovery, exact scanner ownership, and the managed-source contract together.
3. Connect typed installation plans and backend mutation serialization; preserve existing package-plan behavior and TTL/single-use tests.
4. Add the picker, preparation/candidate modal, final preview, footer progress, and rescan. Include receipt-backed preview/uninstall before releasing the feature.
5. Add ZIP and tar.xz separately after the first format passes compatibility and adversarial tests. Later consider safe internal links and explicit manual replacement. Automatic updates remain a separate decision.

Implementation expands the current phase fence. Before implementation, explicitly agree that scope and update `AGENTS.md` in the same change as the new convention. Research alone does not require changing the fence. Keep the website under `docs/` unchanged.

## Verification and remaining uncertainties

Safety tests must exercise the public prepare/preview/apply and removal interfaces in temporary roots, including:

- traversal, absolute paths, duplicate/file-directory collisions, symlink and hard-link escapes, permission bits, PAX overrides, sparse entries, corrupt/truncated gzip/tar, huge metadata, resource exhaustion, cancellation, and worker termination;
- proof that inspection never executes downloaded content or invokes package managers, including fake executables that would write a sentinel if started;
- compatible native executable, PIE, wrong architecture, shared libraries, ambiguous binaries, source-only input, and missing icon;
- spaces, Unicode, percent, quotes, dollar signs, backticks, backslashes, and unsupported equals signs in paths; validate launcher syntax and actual argument interpretation with trusted test helpers;
- changed staging, expired or replayed plans, destination collisions, concurrent installs, modified receipts/launchers, modified installed files, and second-process lock contention;
- injected failure/crash at each commit and Trash step, repeated recovery, and refusal to affect unrelated files;
- exactly one listed portable app, no duplicate Desktop row, stale-cache compatibility, blocked update routes, and shared details cancellation/identity.

Run `cargo check`, `cargo test`, and `cargo clippy` for the implementation. Validate real GNOME/KDE menu integration and a small representative set of release bundles without making live installations part of ordinary tests. No Rust checks were run for this research-only document.

Open items before implementation: inspect the actual Diary release; benchmark resource budgets and the worker's memory limit; confirm root/no-clobber/Trash behavior on supported filesystems; choose and test the generated Exec tokenizer; check icon decoding limits; and test desktop menu refresh. The design does not promise sandboxing of installed apps, dependency completeness, publisher verification, or compatibility with arbitrary archives.

## Primary sources

- **S1.** [freedesktop.org Desktop Entry Specification, Exec](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html). Command parsing, quoting, prohibited executable-path character, and field codes.
- **S2.** [freedesktop.org XDG Base Directory Specification](https://specifications.freedesktop.org/basedir/latest/). Data paths, defaults, absolute-path requirement, and user executable conventions.
- **S3.** [freedesktop.org Desktop Entry Specification, file naming and IDs](https://specifications.freedesktop.org/desktop-entry/latest/file-naming.html). Applications directories, identity, and precedence.
- **S4.** [tar 0.4.46 security documentation](https://docs.rs/tar/0.4.46/tar/index.html#security), [Entry](https://docs.rs/tar/0.4.46/tar/struct.Entry.html), and [Archive](https://docs.rs/tar/0.4.46/tar/struct.Archive.html). Streaming readers, extraction checks, permission controls, and concurrent-mutation limitations.
- **S5.** [RustSec RUSTSEC-2026-0067](https://rustsec.org/advisories/RUSTSEC-2026-0067.html). Symlink-related directory permission changes; patched in tar 0.4.45.
- **S6.** [RustSec RUSTSEC-2026-0068](https://rustsec.org/advisories/RUSTSEC-2026-0068.html). Inconsistent PAX size interpretation; patched in tar 0.4.45.
- **S7.** [zip 9.0.5 ZipFile documentation](https://docs.rs/zip/9.0.5/zip/read/struct.ZipFile.html). Unsafe raw paths, `enclosed_name`, `mangled_name`, and symlink checks. Version documents the research snapshot, not a dependency selection.
- **S8.** [System V ABI, ELF header](https://gabi.xinuos.com/elf/02-eheader.html). File types, architecture, class, byte order, and OS ABI semantics.
- **S9.** [Linux man-pages, ldd(1)](https://man7.org/linux/man-pages/man1/ldd.1.html). Explicit warning about executing untrusted binaries.
- **S10.** [Gear Lever first-party repository](https://github.com/mijorus/gearlever). AppImage integration, organization, updates, and Trash-based removal.
- **S11.** [Debian desktop-file-utils, update-desktop-database(1)](https://manpages.debian.org/trixie/desktop-file-utils/update-desktop-database.1.en.html). MIME-handler cache semantics.
- **S12.** [Tokio spawn_blocking documentation](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html). Already running blocking tasks cannot be aborted.
- **S13.** [flate2 MultiGzDecoder documentation](https://docs.rs/flate2/latest/flate2/read/struct.MultiGzDecoder.html). Streaming and concatenated gzip members.
- **S14.** [cap-std Dir documentation](https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html). Directory-relative filesystem operations and explicit ambient authority.
- **S15.** [freedesktop.org Trash Specification](https://specifications.freedesktop.org/trash/latest/). Trash directories, metadata, and recovery information.

Sources were read through Firecrawl. Research output was kept in temporary files; no downloaded application was installed or executed. The background research attempt failed because its provider reached a usage limit; the source verification and this proposal were completed in the parent task.
