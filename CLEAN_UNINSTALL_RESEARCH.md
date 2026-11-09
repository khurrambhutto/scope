# Complete app uninstall in Scope for Linux

Investigated 31 October 2026 using the reverse-engineer-anything skill and three GPT-6.1 Terra agents with high reasoning. This report recommends a design. It does not implement cleanup or change removal policy.

## Recommendation

Keep native package managers responsible for installed packages. Add one shared, source-aware model for reviewing associated app data, recording exactly what the user selected, and reporting what happened to each item. Fix the existing AppImage removal and result-reporting defects before extending cleanup to more sources.

A useful product promise is: remove the selected installation and the app data the user reviewed, and report anything retained, skipped, failed, or unknown. Linux apps have no universal manifest of files they create. A package uninstall, an empty search result, and an application's disappearance from the list cannot establish that every profile, cache, credential, or saved file is gone.

The biggest current gap is in ownership and outcome modeling. More directory-name patterns would increase coverage and mistakes together. Several sources already remove their managed installation correctly while deliberately preserving personal data.

## Evidence and investigation boundary

- Scope commit: `2dbdef6c99c31824ab5262e9394a13c48884da8b`.
- [Mole macOS CLI](https://github.com/tw93/Mole/tree/85162c24071d98560749e5c0a192876a252d7733), cloned to `/tmp/scope-mole-reference`.
- [Mole Windows CLI](https://github.com/tw93/Mole/tree/79641abf4cdd60e4126629d32e8713e40f73ae96), cloned to `/tmp/scope-mole-windows-reference`.
- The reverse-engineer skill directs complete source-repository investigations to normal source tools. No REA binary decompilation or runtime observation was needed or performed.
- Mole's [README distinguishes its open-source CLI from the separate native app](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/README.md#L19). The native Mac app's advertised catalog of 800+ apps was not inspected. This report makes no claim about those native apps' internal cleanup implementation.
- Both inspected Mole branches contain GPLv3 license text. Scope's [LICENSE](LICENSE) is MIT. Learn from behavior and independently design Scope's implementation. This investigation copies no Mole implementation or cleanup catalog and makes no legal assessment.

Source observations below describe inspected code. One directory failure was independently reproduced against unchanged Scope source in a temporary harness. Performance proposals are unmeasured. No real app uninstall, elevated command, Mole cleanup script, live update query, or scan of personal app data was run.

## Existing defects to fix first

### AppImage data directories are offered but cannot be removed

`appimage::app_data::find` accepts user-owned real directories and files in the XDG config, data, cache, and state roots. The preview offers them behind the unchecked app-data option. Apply passes the selected, rediscovered paths to `system::trash::trash`, which refuses every directory before attempting `gio trash`.

The executable is moved first. Consequently, profile and cache directories remain after the app itself disappears. This directly explains one class of incomplete uninstall.

Evidence: [candidate discovery](src/domain/appimage/app_data.rs#L161), [ownership accepts directories](src/domain/appimage/app_data.rs#L336), [apply order](src/domain/appimage/operations.rs#L103), and [directory refusal](src/domain/system/trash.rs#L17).

Verification: a temporary Cargo harness includes the unchanged `trash.rs` by absolute path and calls its public function on a temporary directory containing a settings file. It observes `Refusing to trash a directory` and verifies that the contents remain. External commands are disabled in the harness. This reproduces the public helper failure, not the entire GPUI uninstall flow.

Fix with an explicitly validated directory-capable Trash path. Preserve ownership, sharing, root, symlink, and fresh-state checks. Review cross-filesystem behavior and replacement races before extending the helper. Removing the guard alone is insufficient validation of that change.

### The UI hides partial-removal warnings

AppImage apply returns `success: true` with a message listing remaining paths when a launcher, icon, or selected data move fails. The footer drops `result.message` for every successful result and displays only that the app was uninstalled. Streamed operation log lines are also discarded by the current UI handler.

Evidence: [partial backend result](src/domain/appimage/operations.rs#L145), [success footer](src/ui/operation_footer.rs#L59), and [stream handler](src/ui/app_view.rs#L455).

Add a distinct partial outcome and show its remaining work in the dismissible footer. Preserve the fact that the executable was removed. A later cleanup failure must not imply the whole operation was undone. A compact result can say "App removed; 2 data folders remain" and expose the affected paths and reasons.

### Selected data can be silently omitted during apply

Apply reruns discovery and intersects planned items by path and kind. A worker failure becomes an empty list; unreadable discovery roots are skipped. Selected items that are no longer found receive no individual result. Keeping unsafe or ambiguous items is correct, but the operation must report why they were retained or why their state is unknown.

Evidence: [worker fallback](src/domain/appimage/operations.rs#L103), [unreadable roots](src/domain/appimage/app_data.rs#L187), and [`still_found`](src/domain/appimage/app_data.rs#L218).

Replace absence-only reporting with explicit discovery completeness and per-item skipped or unavailable outcomes. Evaluate selected data before removing the primary executable so relevant errors are known early. Revalidate again at each filesystem mutation.

### Path equality is weaker than reviewed-object identity

`AppData` stores path, kind, and optional bytes. `still_found` treats the same path and kind as unchanged. A replacement object at that path can qualify after preview. This is a source-observed hardening gap; no exploit or complete race reproduction was demonstrated.

Capture no-follow filesystem identity and relevant ancestor identity for selected data. Define how ordinary edits inside a profile differ from replacement of its root. A `stat` check followed by a pathname-based `gio trash` call still leaves a race. Investigate the actual safe-Rust mutation primitive before claiming that the reviewed object is guaranteed to be the moved object.

### Native manager completion has no dedicated uninstall postcondition

APT, Snap, and Flatpak return the command result directly. Scope rescans after completion, including failures, but this is separate from verifying the exact target before presenting success. User-tool operations already re-inventory their recorded installation and distinguish absent, still present, and verification failure.

Evidence: [native uninstall dispatch](src/domain/operations/uninstall.rs#L224), [tool verification](src/domain/tool_managers/operations.rs#L249), and [rescan lifecycle](src/ui/app_view.rs#L456).

Add a source-specific postcondition with absent, present, and unknown states. For APT, distinguish package absence from remaining conffiles when verifying a purge. For Flatpak, use the recorded scope. Reuse local manager state and avoid online update checks. This is a verification improvement, not evidence that these managers commonly report false success.

## What Mole teaches us

Mole's macOS uninstall flow resolves app identity, discovers candidate remnants, separates personal data for review, captures identities, checks surviving installations, invokes the native manager where applicable, removes reviewed remnants, and checks what remains. Its Windows branch uses registered uninstallers or Store removal and separately validates leftover install folders. The implementations differ in their preview guarantees and failure handling.

Useful lessons have direct source evidence:

- **Use the manager's uninstall and verify its outcome.** The macOS batch path calls Homebrew without `--zap` so the manager cannot expand removal to unreviewed personal paths. If uninstall fails, it refuses manual bundle fallback while the cask remains installed or its state is unknown. [Batch flow](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/batch.sh#L2721-L2796).
- **Treat incomplete ownership discovery as unknown.** Windows enumerates hidden and duplicate registered owners separately from display inventory. Incomplete enumeration, or another owner of an equal, parent, or child path, prevents leftover-folder removal. [Owner collection and guard](https://github.com/tw93/Mole/blob/79641abf4cdd60e4126629d32e8713e40f73ae96/bin/uninstall.ps1#L264-L322).
- **Bind review to identity.** The macOS flow snapshots device, inode, and modification time for the app and its identity file, then checks them against the preview. [Identity checks](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/batch.sh#L1832-L1877). Scope's existing expiring, single-use `PlanStore` is a good base to retain.
- **Shared app identity requires retention.** Sibling installs can use the same profiles, helpers, or data. Mole checks siblings again near removal and retains data when ownership becomes unknown. [Live leftover guard](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/batch.sh#L700-L738).
- **Personal content needs explicit review.** Even `--with-data` holds back known browser profiles, app groups, and game libraries. Its cache exceptions show that a cache label alone does not mean disposable. [Review classification](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/review.sh#L4-L100).
- **Verify absence separately from command exit.** The Windows uninstaller checks registration after the vendor command and treats unreadability as failure to establish absence. [Postcondition](https://github.com/tw93/Mole/blob/79641abf4cdd60e4126629d32e8713e40f73ae96/bin/uninstall.ps1#L773-L788). Registration absence still cannot prove every file is gone.

Do not import macOS or Windows paths, broad name variants, automatic cache deletion, process-name killing, or a manager's aggressive cleanup flags as Linux defaults. Mole also has limitations: its Windows confirmation is narrower than an immutable path-level plan, and its success accounting can omit a leftover recycle failure. Neither branch proves universal cleanup or an atomic uninstall transaction.

## Coverage across Scope's current sources

The table describes current Scope behavior. The last column is a design recommendation, not an implemented command or permission to broaden cleanup. Source paths: [uninstall](src/domain/operations/uninstall.rs), [tool commands and checks](src/domain/tool_managers/operations.rs), [JavaScript commands](src/domain/tool_managers/javascript_commands.rs), and [Flatpak details](src/domain/package_details/flatpak.rs).

| Source | Current removal | Retained or outside current authority | Recommended next step |
| --- | --- | --- | --- |
| APT, including opened `.deb` installs | Simulated, protected, fingerprinted `apt-get purge` with autoremove disabled | HOME profiles and caches; automatic orphan dependencies | Verify purge postconditions. Later associate app data through reliable desktop/AppStream identity and reviewed app-specific rules, never a package-name sweep. |
| Snap | Normal `snap remove`; fresh informational preview | Existing snapshots and any new recovery snapshot; unrelated HOME data | Explain retained effects and verify snapd state. Keep purge and snapshot deletion outside this change. |
| Flatpak user and system | Native uninstall with the recorded explicit scope | Current-user `~/.var/app/<id>`; shared runtimes | Best initial candidate for a future data-cleanup expansion because the data root has an exact app ID. Retain it when another scoped install claims it or ownership is unknown. |
| Scope/Gear Lever AppImage | Trash executable, complete launcher set, and integration icons | Portable `.home` and `.config` sidecars; unselected or unmatched data | Fix directory removal and partial reporting first. Keep exact-name association visibly weaker than a manager receipt. Review portable sidecars as a separate future choice. |
| AppImage outside verified integration | Discovery only | Executable and all data | Offer supported integration as a possible future workflow. Do not infer deletion authority from executable permission or filename. |
| uv | Native uninstall of the recorded tool installation | Shared Python, caches, personal data | Preserve rooted receipt validation and postprobe. Add data choices only with reliable app-specific evidence. |
| pipx | Native uninstall of recorded environment, including injected packages and commands | Shared Python and personal data | Keep environment identity and injected-package effects explicit. |
| Cargo | Native uninstall of tracked crate in the recorded root | Toolchains, build/registry caches, personal data | Retain receipt and binary ownership checks. Shared caches are not per-app cleanup targets. |
| npm global | Native uninstall with recorded prefix and Node runtime | Shared runtime, caches, personal data | Keep native dependency reconciliation and exact global context. Do not inspect project dependencies. |
| Bun global | Native removal with recorded global and bin roots | Shared runtime, caches, personal data | Preserve global manifest, ownership, and root revalidation. |
| Desktop | Discovery only | Launcher, executable, all data | A launcher does not establish ownership of its executable. Add management only through a separately verified installation contract. |
| Steam | Read-only; open the game in Steam | Game files, Proton prefixes, saves, workshop and other data | Continue manager handoff. Native removal integration and any save deletion need separate design and authorization. |
| JetBrains Toolbox | Read-only; explicitly open Toolbox | IDE and user data | Continue manager handoff. Never delete discovered IDE directories as a substitute for manager integration. |

These are all currently modeled sources, not all Linux packaging systems. RPM/DNF, pacman, Nix, and arbitrary installer scripts are not covered by this investigation. Unsupported installations need their own manager contracts rather than a generic recursive delete.

Flatpak user/system installs, multiple channels, APT and AppImage editions, and tool roots can share personal data. Installation identity and app-data identity must be separate concepts. UI visibility, filtering, or the absence of a visible launcher must not determine whether another installation owns data.

## A small shared cleanup design

Extend existing `OperationPlan` and source handlers. Keep orchestration in `backend.rs` and feature logic in domain modules. The UI renders the plan; it does not invent paths or commands.

One reviewed item needs an opaque ID, location, effect, evidence, bounded size status, and fresh identity where the operation acts directly on the filesystem. Effects distinguish manager-controlled removal, optional Trash, retained shared data, and unknown attribution. Evidence distinguishes a manager receipt, verified integration, a reviewed app rule, and a name association. User ownership is necessary for direct user-data cleanup but does not prove exclusive app ownership.

Use a common flow:

1. Resolve the exact installation from the scan, including Flatpak scope or tool root and recorded manager context.
2. Ask the source adapter for installed-file effects and associated-data candidates. Search only defined roots. Preserve an explicit completeness result when a root or ownership source cannot be read.
3. Check claims from every relevant installed source, independently of list filters. Retain ambiguous, shared, or insufficiently identified data. An app-specific declarative rule can improve coverage, but it must specify identities, bounded paths, sharing exclusions, and fixtures. It cannot execute shell code.
4. Show one compact uninstall preview with the mandatory package effect and optional data choices. Keep profiles, settings, caches, and state unchecked. Default conservatively when contents may include credentials, saves, offline work, or synced files.
5. Bind choices to the issued plan. A future per-item UI submits opaque IDs that must be a subset of the backend's reviewed list. Never accept client-supplied paths or silently add discoveries at apply. Preserve expiry and single use.
6. Revalidate manager context, protection, ownership, sharing, and selected object identities. Run the manager command, verify its recorded target, then revalidate and move only selected eligible data. A manager failure or unknown outcome prevents subsequent data cleanup unless a separately reviewed operation establishes its safety.
7. Produce structured outcomes for each effect and always rescan. Distinguish removed, moved to Trash, retained by choice, skipped because state changed, failed, and unknown. Keep remaining items reviewable after the installed row disappears. A retry requires a new preview.

Direct directory actions need no-follow handling for leaf and ancestors, mount-boundary checks, protection against replacing a reviewed path, and tested recovery behavior. Prefer reversible moves through Trash. Unsupported filesystems and uncertain validation retain data with an explicit reason. Never silently switch to permanent deletion.

Running apps can recreate data or write through open handles after a directory moves. Detect exact same-user executable relationships where possible and ask the user to close the app before direct profile cleanup. Preserve unknown process state. Never use broad name matching to kill processes, and do not claim a negative process check proves that no writer exists.

Trash removal is reversible storage relocation. It usually does not free disk space until Trash is emptied. Native package removal has different recovery properties. The UI should distinguish these effects and keep size estimates separate from guaranteed reclaimed space.

## Performance work with the highest expected value

No benchmark established that Mole is faster or that a proposed Scope design is optimal. Package-manager work, decompression, filesystem latency, and directory size can dominate; worker count alone is not a performance strategy.

Preserve what Scope already does well: source inventory before network update checks, one scan-local desktop index, streamed detail measurements, a two-inspection limit, short detail caching, bounded traversals, closed stdin, process-group timeouts, and one applying operation. Evidence: [scan pipeline](src/domain/scanner/pipeline.rs), [inspection permits](src/domain/package_details/inspection.rs#L25), [measurement bounds](src/domain/package_details/measurement.rs#L54), [detail cache](src/ui/package_detail_state.rs), and [command execution](src/domain/system/mod.rs).

Prioritize these experiments:

| Experiment | Source-observed opportunity | Safety and behavior that must remain equivalent |
| --- | --- | --- |
| Split candidate discovery from measurement | AppImage apply reruns `find_app_data`, including size walks, although `still_found` uses only path and kind | Fresh ownership, sharing, and identity checks remain mandatory. Preview sizes can stream independently. |
| Use operation-specific APT probes | Uninstall's `probe_apt` also invokes local `apt-cache policy`, although uninstall revalidation needs presence and protection | Update probes still check update state. APT simulation, version-bearing fingerprints, and purge postconditions remain. |
| Reuse discovery within a read-only request | AppImage candidate discovery rebuilds launcher information separately from the scan | Cache keys and completeness are explicit. A display cache never authorizes apply. Avoid stale ownership proof. |
| Investigate repeated manager verification | Tools verify inventory at preview, backend probe, direct apply, and after success | Direct apply must still fail closed. Remove repetition only by preserving or narrowing the actual API boundary; retain postconditions. |
| Cancel obsolete preview work | UI invalidates stale preview results, but the preview worker continues | Cancel read-only discovery cooperatively. Never treat dismissing a dialog as cancelling a package manager mid-transaction. |

Generic directory measurements are bounded to two seconds and 20,000 entries per call. Tool measurements allow 15 seconds and 250,000 entries per call. These are not end-to-end preview deadlines. AppImage data discovery measures several candidates sequentially, so add a request-wide budget and cancellation when separating discovery from size work. Incomplete ownership checks must retain data; incomplete size measurements can still display partial estimates.

Use release builds and deterministic private fixtures with fake manager executables. Count subprocesses, metadata calls, visited entries, and candidates so a faster run cannot pass by silently skipping work. Compare equivalent effects and safety checks. Measure first fresh inventory, preview-ready latency, apply verification latency, cancellation latency, CPU, and peak memory across repeated small and large fixtures. Record sample distribution and cache conditions. Do not call a run cold-cache without controlling cache state, or report a speedup before showing equivalent coverage and error behavior.

## Implementation order and acceptance tests

1. **Make existing outcomes truthful.** Show partial AppImage removal and skipped selections, including remaining paths. Add a regression fixture joining data discovery to the public Trash path and a test for partial-result presentation. No broader cleanup feature is needed for this step.
2. **Make the existing AppImage opt-in work.** Add validated directory-capable Trash behavior and object identity checks. Test both `gio` and fallback behavior in private roots, unavailable Trash, cross-filesystem failure, replaced paths, symlinked ancestors, shared names, inaccessible ownership sources, and partial moves. Preserve recovery information and never permanently delete as fallback.
3. **Verify manager outcomes.** Add local per-target postconditions for APT, Snap, and scoped Flatpak. Test still present, absent, remaining APT configuration, manager failure after partial work, and probe unavailable. Keep tool postverification and rescan on every apply outcome.
4. **Measure and reduce unnecessary work.** Start with apply-time size walks and APT uninstall update lookup. Prove equivalent ownership and protection behavior before comparing timings. Retain direct-call validation where it is required.
5. **Expand personal-data cleanup deliberately.** After the applicable product decision, generalize reviewed data items and start with exact Flatpak app-ID roots and shared-install checks. Add reviewed desktop/app-specific rules for APT and selected tools as coverage grows. Test each rule against an owned app, a sibling install, an unrelated app with a similar name, shared vendor directories, and missing metadata.
6. **Keep unsupported and read-only sources explicit.** Steam, Toolbox, Desktop, unmanaged AppImages, orphan dependency cleanup, snapshot deletion, and whole-system cleanup require their own contracts. They are not completed by adding another filesystem pattern.

Cross-cutting tests cover expired/replayed plans, forged or changed selections, fresh candidates appearing after preview, incomplete discovery, concurrent starts and writes, mount points, partial results, closed result streams, package/self-update exclusivity, cache invalidation, inspection limits, and cancellation. Native-manager destructive integration tests belong only in disposable environments. A native uninstall cannot be rolled back merely by restoring trashed profiles.

The current [AGENTS.md phase fence](AGENTS.md#phase-fence) limits generalized cleanup, while permitting normal per-app APT purge and the existing opt-in AppImage data workflow. This report proposes future capabilities without changing that boundary. Source hardening and fixes to already promised behavior can be scoped independently.

## Verification performed and remaining unknowns

Commands completed successfully:

```text
cargo test --locked domain::appimage::app_data::tests -- --nocapture
  5 passed
cargo test domain::system::trash::tests -- --nocapture
  3 passed, run by the all-source reviewer
cargo test --offline --manifest-path /tmp/scope-trash-audit/Cargo.toml -- --nocapture
  4 passed, including the new directory-refusal reproduction and the 3 included Trash tests
```

The temporary harness is at `/tmp/scope-trash-audit`. Its directory test exercises unchanged production `trash.rs`; its command wrapper is disabled. These checks do not establish GUI rendering, safe directory Trash after a future fix, an end-to-end uninstall, native-manager behavior on every distribution, or resistance to all concurrent filesystem races. No full build or benchmark was required for the documentation-only repository change, and none is claimed.

The named defects are explained by source and, for directory refusal, a direct fixture. Complete cleanup coverage, native Mole app internals, exact reclaimed bytes, reliable Trash recovery locations on every filesystem, and a proven fastest implementation remain unresolved. Scope's pre-existing `.gitignore` change was preserved.

## Research records

The three delegated reports contain additional source references and limits:

- `/tmp/scope-mole-findings.md`
- `/tmp/scope-uninstall-findings.md`
- `/tmp/scope-uninstall-performance.md`

Mole review thread: <t3-thread://v2/e6f0270a-dcac-41d8-9585-02722c8f1794/thread:delegated-task:command%3Amcp%3A6ca633f0-68dd-4cf3-b837-49c35318ef18%3Adelegate-task%3A14179606-06bf-4d0b-9f72-f87fcfb36b9b>

Scope review thread: <t3-thread://v2/e6f0270a-dcac-41d8-9585-02722c8f1794/thread:delegated-task:command%3Amcp%3A6ca633f0-68dd-4cf3-b837-49c35318ef18%3Adelegate-task%3Ac4d1c9ba-778d-402f-97ea-3c7ab075cc0d>

Performance review thread: <t3-thread://v2/e6f0270a-dcac-41d8-9585-02722c8f1794/thread:delegated-task:command%3Amcp%3A6ca633f0-68dd-4cf3-b837-49c35318ef18%3Adelegate-task%3Ae4860a71-58c1-4eff-b59f-bab486c25fa6>
