# Unified full uninstall for Scope for Linux

Research date: 31 October 2026. This is a proposed product and implementation design, not an implemented cleanup capability. It builds on [the first investigation](CLEAN_UNINSTALL_RESEARCH.md), an independent audit of every current source, fresh official documentation, and a fresh clone of Mole. Scope HEAD is `2dbdef6c99c31824ab5262e9394a13c48884da8b` plus the current uncommitted AppImage fixes. No installed application or profile was modified during this research.

## Decision

Make **Uninstall** a unified, preview-first removal of the selected installation and its verified associated data. The preview must cover app files, configuration, profiles, caches, state, integration files, and relevant retained effects. Package removal alone is an incomplete result when reviewed app-data removal was requested.

Use one shared model across APT, Snap, Flatpak, AppImage, uv, pipx, Cargo, npm and Bun. Represent Desktop, Steam and JetBrains Toolbox in the same capability model, while explicitly showing that their removal adapters are not implemented. Supporting every source in a view does not establish authority to remove every installation.

The target promise is: **remove this installation and the app data selected in this preview, then report every retained, skipped, failed or unverified item**. There is no reliable universal inventory of all files arbitrary Linux programs create. Scope must expose unknown coverage rather than claiming that an empty list proves complete cleanup.

For this product direction, propose **Remove app and app data** as the default choice in the confirmation, with **Keep app data** available. Only verified, exclusively attributable data is eligible for the default. Identify that profiles can contain credentials and history before confirmation. Separately expose documents, saves, offline libraries, model files, synced content and other valuable content when a tested rule can attribute them; do not silently include such content through a broad parent-directory selection. Data whose contents cannot be safely classified needs explicit review. Weak name matches are clues, not preselected deletion targets.

This is a change from the earlier report's blanket unchecked-data recommendation. The revised design follows the requested full-uninstall product direction while preserving review, exclusive ownership and protection checks. It does not authorize arbitrary directory removal or whole-system cleanup.

## What the earlier fix did and did not solve

The working tree now accepts directories in Trash, binds AppImage candidates to previewed filesystem identity, repeats checks, and preserves cleanup warnings after the row disappears. These fixed removal and reporting defects. They did not create a reliable cross-source app-data inventory.

The remaining root problems are:

1. Installation identity and application-data identity are conflated. A package name, executable filename, app display name and persistent profile ID can differ.
2. `OperationPlan.app_data` and the selection boolean are AppImage-specific. Other native managers do not obtain a common reviewed-data plan.
3. Sharing checks use visible desktop launchers rather than all relevant installations. Hidden, CLI, other-scope and alternate-format installs can be missed.
4. Discovery errors can collapse into an empty result. Unknown ownership or unreadable directories must be represented as incomplete coverage.
5. Native APT, Snap and Flatpak uninstall paths lack a dedicated immediate postcondition; tools already re-inventory after successful removal.

Evidence: [plan and result DTOs](src/domain/operations/mod.rs), [uninstall dispatch](src/domain/operations/uninstall.rs), [AppImage discovery](src/domain/appimage/app_data.rs), [AppImage cleanup](src/domain/appimage/uninstall_cleanup.rs), [tool operations](src/domain/tool_managers/operations.rs), [confirmation UI](src/ui/dialog.rs), [apply boundary](src/backend.rs), and [result footer](src/ui/operation_footer.rs).

## Fresh external findings

**APT purge does not clear HOME profiles.** APT purge includes package configuration. Dpkg explicitly limits its purge discussion to system directories and excludes configuration written to individual users' homes. Scope therefore needs a separate current-user data plan even though its native APT command already uses purge. Maintainer scripts can have effects beyond a static file list; the preview cannot promise to enumerate their arbitrary behavior. [APT manual](https://manpages.debian.org/trixie/apt/apt-get.8.en.html), [dpkg manual, purge](https://manpages.debian.org/trixie/dpkg/dpkg.1.en.html#P).

**Flatpak data removal includes permissions.** `--delete-data` removes the app's `~/.var/app` data and permission-store records. Without a target reference it can remove unowned app data more broadly. A unified preview must account for permissions, current-user context, other scopes/refs, and recovery behavior; appending this flag is not a complete design. Never use its no-reference cleanup route for one-app uninstall. [Flatpak uninstall reference](https://docs.flatpak.org/en/latest/flatpak-command-reference.html#flatpak-uninstall).

**Snap can retain a recovery copy.** Normal removal creates an automatic snapshot unless `--purge` is used. The snapshot docs describe a default 31-day retention and irreversible `snap forget`. If full uninstall should avoid creating a new backup, preview that choice explicitly. Existing snapshots are separate reviewed effects; do not silently delete a whole set that may contain other snaps. Current Scope neither purges Snap nor deletes snapshots. [Snap snapshot documentation](https://snapcraft.io/docs/snapshots).

**XDG defines locations, not app ownership.** Config, data, cache and state roots are useful bounded search locations. The specification does not supply an inventory of each application's paths or guarantee exclusive use of a directory. Respect absolute XDG overrides and retain explicit unknowns for nonstandard locations. A name or a cache location does not establish that content is disposable. [XDG Base Directory specification](https://specifications.freedesktop.org/basedir-spec/latest/).

**IDE data outlives installation removal.** JetBrains documents Toolbox removal through Toolbox and separately identifies configuration, cache and data directories for standalone instances, preserving them for reuse. These are product/version-sensitive paths, not permission to remove a shared `JetBrains` parent. Projects are a separate category. This reinforces the need for application identity and source-specific removal capabilities. [JetBrains uninstall guide](https://www.jetbrains.com/help/idea/uninstall.html).

**Tool environment ownership is different from personal-data ownership.** uv documents isolated tool environments, which can include additional executables and depend on a Python installation. This supports keeping the rooted environment effect distinct from profile rules and shared runtimes. Scope's recorded receipts remain the authority for its tool removal commands. [uv tools concepts](https://docs.astral.sh/uv/concepts/tools/), [Scope receipts and commands](src/domain/tool_managers/operations.rs).

## What Mole contributes

Recloned `https://github.com/tw93/Mole.git` to `/tmp/scope-mole-unified-reference`; HEAD is `85162c24071d98560749e5c0a192876a252d7733`, the same macOS CLI revision pinned by the first report. Re-read its uninstall review and ownership/identity code. The Windows branch was not re-cloned this pass; Windows findings in the first report remain prior evidence, not new verification.

Transfer these behaviors, using an independent Linux implementation:

- Discover data using app identity and reviewed rules, then let review refine the discovered plan.
- Recheck sibling/shared ownership and filesystem identity near removal. Incomplete owner discovery cannot prove exclusive ownership.
- Keep valuable personal content separate. Mole's `--with-data` still holds back known browser profiles, game libraries and app-group containers; it also has exceptions to treating caches as disposable.
- Let the native manager remove its installation, verify the result, and prevent a broad fallback from silently extending reviewed effects.

Sources: [personal-data review](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/review.sh), [live ownership, identity and manager flow](https://github.com/tw93/Mole/blob/85162c24071d98560749e5c0a192876a252d7733/lib/uninstall/batch.sh). These observations concern the public CLI, not Mole's separate native apps. No Mole removal script was executed or copied. The macOS cleanup catalog cannot establish Linux coverage.

## Required source coverage

This table separates current behavior from the work needed for the proposed full uninstall. It covers Scope's modeled sources, not RPM, pacman, Nix or arbitrary installers.

| Source | What Scope removes now | Required app-data and removal work |
| --- | --- | --- |
| APT / installed .deb | Simulated, protected native purge; autoremove disabled | Resolve desktop/metainfo identities and reviewed legacy IDs; add exclusive HOME data; show indirect transaction effects; verify absent versus residual config versus unknown. |
| Snap | Native `snap remove`, with fresh informational details | Show manager-controlled data once, external/classic app data separately, and recovery snapshots explicitly. Research/version-test reviewed purge and snapshot policies before enabling them. Verify snapd absence. |
| Flatpak user and system | Scoped native uninstall; current-user data retained | Preview exact app-ID config/data/cache and permissions; check all relevant scopes/refs, including incomplete enumeration; verify exact scoped removal. Preserve shared runtimes and external unknowns. |
| Managed AppImage | Executable, verified launcher/icon set; optional discovered data | Generalize reviewed items; add portable sidecars and evidence-backed app IDs/legacy aliases; replace visible-launcher-only sharing checks. |
| Unmanaged AppImage | Discovery only | Distinguish downloaded artifact from installed app; offer integration/adoption only with an explicit verified contract. No generic deletion fallback. |
| uv | Receipt-bound tool/environment uninstall; postverification | Associate app IDs separately from Python distribution names; include supported dedicated personal data; retain shared Python, uv caches and project/uvx environments. |
| pipx | Recorded environment and exposed commands, including injected packages; postverification | Include injected-app consequences and their supported data identities; check suffixed/multiple environments; retain shared Python and unrelated profiles. |
| Cargo | Tracked crate binaries in recorded root; postverification | Associate application-specific config/state/cache rules; retain toolchains and shared registry/build caches. Receipt identity alone does not identify personal data. |
| npm global | Native uninstall in recorded prefix/runtime; postverification | Use package/command identity and reviewed app rules; keep project dependencies, Node and shared manager caches outside per-tool cleanup. |
| Bun global | Native removal in recorded roots; postverification | Same reviewed-data model with Bun-specific root and manifest validation; retain shared runtime/cache/dependencies not exclusively attributable. |
| Desktop / extracted apps | Read-only launcher/executable details | Add an extracted-install contract for dedicated owned roots before removal. Model wrappers, launchers, resources, other claims and app data. Handy is the acceptance case. |
| Steam | Read-only, Open in Steam | A real adapter needs supported manager removal, rooted library/account identity and verified outcomes. Saves, Proton data, workshop content and cloud behavior need separate review; never remove a library by AppID/name guessing. |
| JetBrains Toolbox | Read-only, Open Toolbox | A real adapter needs supported Toolbox removal and product/data-selector identity. Versioned/shared settings/plugins require ownership checks; never include projects or the whole vendor directory implicitly. |

Repository evidence for all rows: [native uninstall routes](src/domain/operations/uninstall.rs), [tool native context](src/domain/tool_managers/operations.rs), [JavaScript commands](src/domain/tool_managers/javascript_commands.rs), [footprint adapters](src/domain/package_details/inspection.rs), [Flatpak scope](src/domain/scanner/flatpak.rs), [desktop ownership](src/domain/scanner/apt_desktop.rs), [Steam scanner](src/domain/scanner/steam.rs), [Toolbox scanner](src/domain/scanner/jetbrains_toolbox.rs), and [Toolbox launch contract](src/domain/jetbrains_toolbox.rs).

Steam, Desktop, Toolbox and unmanaged AppImage removal are real capability gaps, not a final layer of folder matching. Until their management contracts exist, the unified preview must say removal is unavailable. The all-source product goal is not complete while these remain read-only.

## One preview, one reviewed plan

Every uninstall confirmation should contain the same groups:

| Group | Contents and control |
| --- | --- |
| App files | Native package/environment or verified local installation, launchers/icons, and indirect native effects. Mandatory effects are described, not selectable file checkboxes. |
| App data | Verified exclusive settings, profile/history, cache and state. Default included for full uninstall, with Keep app data and inspectable selections. |
| Personal content | Known documents, saves, libraries, offline downloads or model assets. Separate explicit choices where attribution is supported; unknown mixed content blocks automatic parent selection. |
| Kept / cannot verify | Shared dependencies/profiles, protected paths, other users' data, unsupported effects and incomplete searches, each with a reason. |
| Recovery and estimate | State which effects use Trash and which are native irreversible actions; show partial size estimates without promising that trashed bytes are freed. |

For Paperling, the proposed preview includes the managed AppImage/integration and the exact verified `com.saqla.marklite` config/data roots, which contain its persisted profile. It does not include the Markdown files merely listed in Recent. This is a design example, not a currently working alias rule.

Avoid parallel source-specific modal layouts. Source adapters contribute effects and evidence; the UI renders the shared plan. Existing cached/streamed details inform display but cannot authorize mutation. Long previews scroll while the confirmation action stays available. Coverage warnings are visible before confirmation, not buried after a successful uninstall.

## Identity and data discovery

Represent two distinct identities: the rooted installation and the application-data identity. A data identity can have multiple installation claimants across formats, roots, versions and scopes.

Use this evidence order:

1. Native receipts and explicit app IDs for installation identity; exact scoped Flatpak roots and verified portable sidecars for known data associations.
2. Verified desktop/metainfo and app/framework metadata as identity candidates, with evidence that the app actually uses the associated data paths. A `.desktop` ID or StartupWMClass alone is not a filesystem ownership receipt.
3. A small declarative, tested rule catalog for legacy IDs, nonstandard locations and shared-data exceptions. Each rule records supported identities, evidence provenance, path bounds, content class and exclusions. No shell commands, network-generated deletion paths or unreviewed AI guesses.
4. Exact name matching as lower-confidence discovery for review. Never treat user ownership, fuzzy similarity, a reverse-DNS-looking name, or absence from the visible app list as exclusive ownership.

Paperling's first-party local [Tauri manifest](/home/khurram/Projects/Paperling/src-tauri/tauri.conf.json) explicitly declares `com.saqla.marklite`; its current launcher uses Paperling/paperling. Existing config/data roots with the legacy ID were confirmed by filesystem metadata. Thus the association is evidence-backed, not a guessed spelling variant. Retain the profile if another surviving installation shares it. Generalize this rule across its packaging formats, rather than patching AppImage only.

Handy's launcher points at `~/.local/opt/Handy/AppRun`, while its scanned AppImage is a different downloaded artifact. This proves two distinct paths, not complete ownership of the extracted tree. Verify that dedicated tree and every integration before offering removal. Do not uninstall a download and claim the installed app is gone.

Bound discovery to known roots, record unreadable/skipped locations and ownership-enumeration completeness, and separate path discovery from size measurement. Inspect metadata without executing apps. Include hidden/no-launcher installations relevant to a claimant independently of the default list filters. Retain data whenever an unresolved claimant could share it.

## Apply and result contract

Introduce a shared domain module, for example `domain/app_data/`, used by every supported uninstall adapter. Migrate current AppImage callers and remove the AppImage-only plan field/selection API in the same implementation wave.

A reviewed item needs an opaque ID, category, effect, native-resource identity or filesystem path, provenance, claimants, eligibility/reason, optional filesystem identity, size status and discovery completeness. Effects should be typed: native package removal, native permission/data action, Trash, kept, or unknown. The same path cannot be both a selected parent and a separate selected child; manager-owned paths must not also be sent to Trash. Overlapping aliases/XDG roots are deduplicated without double-counting.

Apply consumes a single-use, expiring PlanStore ID plus validated selected item IDs. No client paths and no additional discoveries are authorized. Revalidate protection, exact native context, identity and sharing. Block or defer profile cleanup while an attributable app instance is running; report unknown writer state and never kill by loose process-name matching.

Run the native removal, then check the exact installation state. Failed, partial or unverifiable native outcomes retain optional user data unless a fresh reviewed plan safely establishes the next operation. An operation cannot be made atomic across native manager changes and user-data moves; do not promise rollback of a package transaction.

Immediately before each selected filesystem action, check ownership, identities, ancestor/mount constraints and current claims. The current pathname-check-then-move code is not a proven race-free implementation; the generalized executor needs a reviewed strategy for these boundaries and tests for substitutions. Preserve cross-filesystem failure behavior and never replace Trash failure with permanent recursive deletion.

Record per-item results: native removed, moved to Trash, kept by choice, kept shared, skipped changed, failed, and unknown. Verify selected paths/native resources after action; report paths recreated by a running writer as remaining or uncertain. A retry gets a fresh preview. Always rescan and retain the result after the app row disappears.

Distinguish **package removed / data cleanup incomplete** from **full reviewed cleanup completed**. Data moved to Trash remains recoverable and consumes storage until Trash is emptied. A user seeking immediate physical erasure needs a separately designed policy; this plan preserves Scope's Trash-only filesystem deletion rule. Local cleanup does not erase cloud data or files in other users' accounts.

## Implementation sequence and acceptance gates

1. **Shared contract and preview across every source.** Replace AppImage-only selections with opaque reviewed items, source capabilities, discovery completeness and item outcomes. Include truthful read-only states from the first version. Update repository policy alongside the broadened per-app behavior; do not introduce a system-cleaning feature.
2. **Execute and verify existing managed sources.** Add APT/Snap/Flatpak postconditions; preserve tool probes. Integrate generic selected-data execution and partial reporting, with no source-specific bypass.
3. **Prove data coverage.** Add exact Flatpak associations, managed AppImage sidecars, the evidence-backed Paperling identity and fixtures for every tool/APT data rule. Treat Snap permissions/backups and mixed-content profiles as explicit policy work. Do not call an adapter complete when it reports only installation files.
4. **Close installation capability gaps.** Design extracted Desktop/Handy and unmanaged artifact management, then supported Steam/Toolbox removal integrations. These remain separate contracts behind the common UI.
5. **Run disposable end-to-end acceptance.** Install, create settings/history/cache, preview, remove selected data, reinstall, and verify the expected clean profile. Preserve documents and sibling installations. Paperling is the first named case, not the entire scope. Only after equivalent correctness is established should preview/apply performance be optimized.

Required tests include all source/refusal adapters; Paperling's legacy ID across formats; Handy tree versus download; two scopes/roots/versions sharing a profile; hidden claimants; unreadable roots and worker failures; expired/replayed plans and forged selections; selected parent/child overlap; symlink/ancestor/mount substitution; foreign ownership; manager still present/residual-config/unknown/partial failure; permission-store effects; Snap retained snapshots; profile recreation; Trash failure and fallback; and results surviving rescan. Native destructive tests run only in private disposable environments, not against installed user apps.

Definition of a complete uninstall: the reviewed installation is confirmed removed; every selected effect has a verified outcome; remaining/shared/unknown coverage is visible; a clean-profile reinstall passes for supported rules. A clean app list alone never passes this gate.

## Verification and limits of this research

Performed: current-source audit across every modeled source; independent delegated audit; fresh Mole source clone and targeted reads; current official APT/dpkg/Flatpak/Snap/XDG/JetBrains/uv documents fetched through Firecrawl; read-only local app identity and launcher checks. Raw fetched pages are retained under `/tmp/scope-cleanup-research/.firecrawl/`; the independent detailed audit is `/tmp/scope-unified-source-audit.md`.

No app launch, real uninstall, profile modification, snapshot removal, privileged command, new build, benchmark, or clean-profile reinstall was performed in this research pass. The earlier 435 passing tests validate the earlier code change, not this unimplemented design. Official documentation describes native behavior, not proof that Scope handles every variant. Runtime behavior and filesystem race guarantees remain implementation acceptance work.
