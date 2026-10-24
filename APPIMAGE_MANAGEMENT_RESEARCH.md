# AppImage management in Scope for Linux

Investigated 24 October 2026. Gear Lever reference commit: [`a2917f2adafc78e0478e47d5843de9ede6c1aa3f`](https://github.com/mijorus/gearlever/tree/a2917f2adafc78e0478e47d5843de9ede6c1aa3f). The reference repository was cloned locally for inspection. Scope's implementation was written independently.

## Reproduced failures

ZCode was installed at `~/AppImages/zcode.appimage`, with its extracted icon still present in `~/AppImages/.icons/zcode.png`. Its installed application launcher directly named that file in `Exec`, but had neither `TryExec` nor `X-AppImage-Version`. Scope required `TryExec` to discover a managed installation, so it showed the discovery-only action. The versionless filename also produced `unknown`.

The launcher's current shape is consistent with an application rewriting its desktop integration. The investigation did not execute ZCode or observe the rewrite occurring. Reading the installed bundle's root desktop file with `unsquashfs -cat` found `X-AppImage-Version=3.16.3.8992`.

The direct scanner test reproduced both symptoms before the fix and subsequently reported `name=ZCode version=3.16.3.8992`, with the integration passing apply-time verification. The same inspection found T3 Code's bundle reporting `0.1.2-nightly.20261022.3526` while its installed launcher retained `0.0.34-nightly.20260819.1133`. Bundle metadata must therefore take precedence even when the launcher supplies a version.

## What Gear Lever does

- [Installed discovery](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/src/providers/AppImageProvider.py#L96-L151) finds installed apps through desktop launchers and their `TryExec` fields, and reads launcher version metadata. It has the same limitation when an app drops those fields.
- [Installation and naming](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/src/providers/AppImageProvider.py#L280-L328) use a name derived from the app, with version or numeric suffixes to resolve collisions. A versionless installed filename is normal.
- [Launcher rewriting](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/src/providers/AppImageProvider.py#L367-L436) sets the installed executable, `TryExec`, icon, and available AppImage metadata, while preserving installed launch settings on replacement. Scope retains this interoperability and now also preserves the shipped environment on a fresh installation.
- [Metadata extraction](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/src/providers/AppImageProvider.py#L743-L904) reads bundled desktop metadata. Its optional self-extraction fallback can execute the AppImage runtime. Scope continues to read the ELF and SquashFS data without executing the AppImage.
- [Settings](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/data/it.mijorus.gearlever.gschema.xml#L4-L33) include configurable storage and external-folder management. Scope keeps its existing, narrower `~/AppImages` management boundary.
- Gear Lever's [license](https://github.com/mijorus/gearlever/blob/a2917f2adafc78e0478e47d5843de9ede6c1aa3f/COPYING) is GPLv3; Scope's license is MIT. These changes use the observed behavior as a reference and copy no Gear Lever implementation code.

## Implemented behavior

Managed discovery accepts a user-owned application launcher whose parsed `Exec` directly names a user-owned type 2 AppImage in `~/AppImages`. A supplied `TryExec` must match. Shell wrappers, relative/PATH commands, conflicting targets, symlinked launchers, and symlinked AppImage roots do not establish a managed installation. Before apply, revalidate the file identity and the complete launcher/icon set so new or retargeted entries require a rescan.

Read the current bundle's root `X-AppImage-Version` before relying on the installed launcher or filename. The version reader limits output to 256 KiB, uses one decompression processor and an 8 MiB decompression cache, allows two concurrent reads, and bounds each read to five seconds and the scan phase to 30 seconds. Successful results are cached in memory by path, file size, and modification time. Failed reads keep the existing fallback. Multiple root launchers are ambiguous and do not supply a version through this reader.

Integration stages the icon, desktop launcher, and checksum-verified executable before committing. The executable is committed last. Private hard-linked backups allow rollback on a commit error; a failed rollback retains the backup and reports its location. A fresh installation cannot overwrite an unrelated icon. This handles ordinary write and rename failures, not power-loss recovery across several files.

## Verification and limits

Regression tests cover the actual ZCode launcher shape, missing and stale launcher versions using small real SquashFS fixtures, cache invalidation after file replacement, launcher retargeting, new launchers after preview, symlinked targets, environment preservation, and integration rollback. The optional real-image test performs install, launcher rewriting, rediscovery, and Trash removal under a disposable home. A separate ignored read-only scanner test checks an existing installation through `SCOPE_TEST_APPIMAGE`.

AppImages have no universal installed-version contract. Paperling's inspected root desktop file has no version key; Scope must keep the version unknown when no supported source provides one. Update support still depends on supported embedded update information and a published checksum. This investigation did not establish support for every publisher's updater, custom Gear Lever destination, or AppImage type.
