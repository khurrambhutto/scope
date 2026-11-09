# Gate 1: three directions

Status: **decided 2026-11-06, the owner chose C.** See `02-art-direction.md` and ADR-1. Kept as the record of the alternatives.

All three keep: the red telescope mark, real icons and screenshots, the v0.3.8 facts in `01`, the existing URLs, verification files and SEO.

---

## A. Night Survey (recommended)

**Feeling:** an observatory at night pointed at your own machine. Quiet, precise, a little wonder.

| | |
|---|---|
| Palette | The app's own: `#2D1414` → `#0B0C0F` ground (the app window's background gradient), warm ink `#EFE6E4` at 100 / 72 / 56% (steal #10), hairline `#4B3034`, signal red `#E10600` from the telescope for the scan and the primary button only |
| Type | Display **Bricolage Grotesque** 600 (continuity with the current site), text **Work Sans** (the app's own bundled face), labels **JetBrains Mono** 500 (craft §1 strategy 3, adapted) |
| Hero | Centred one-line h1, one sentence, `.deb` button + AppImage, the release eyebrow (steal #3). The live replica window sits directly under it as the hero object (steal #1, techniques §1 centred, compact) |
| Signature | **The scan** (steal #6). On load, a thin reticle sweeps a dark field where real app icons are scattered in loose clusters by source (APT, Snap, Flatpak, uv, Steam…). Each icon the reticle crosses lights, and is counted into the replica's list; the search field's count ticks to `46 results`. Then the replica is fully interactive. Reduced motion: the list is simply there |
| Sections | fact strip (#5) → sources ledger (12 rows, real commands) → 01–04 Plan / Confirm / Revalidate / Run (#7) with the uninstall preview rebuilt in HTML (#12) → screenshots → safety → install with copyable command (#8) → FAQ → maker footer (#11) |
| Steals | 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12 |
| Risk | The scan could delay reaching the download. Mitigation: it plays inside the replica area only, after the h1 and buttons are already visible, and finishes under 2 s |

## B. Field Guide

**Feeling:** a naturalist's field guide to the software living on your computer. Warm, literate, careful.

| | |
|---|---|
| Palette | Warm paper `#F1EFE8` (the icon tile's bottom stop), ink `#211719` (the app's elevated surface as text), signal red `#E10600`; per-source colours from `theme.rs` appear only on that source's entry |
| Type | Display **Instrument Serif**, text **Work Sans**, labels **JetBrains Mono** (craft §1 strategy 1) |
| Hero | Left-aligned: "Every app on your machine, catalogued." with the real app screenshot mounted like a plate, bleeding off the right edge (techniques §1 left-aligned) |
| Signature | **Specimen entries.** Each of the 12 sources is a numbered catalogue entry (`APT-01`, `SNP-02`, `FLT-03`…) with what Scope can do (update / uninstall / read-only) and the exact command it runs, printed like field notes. Hovering an entry highlights its apps in a small pinned screenshot |
| Steals | 2, 3, 5, 7, 8, 9, 11; teenage-style codes from the skill's techniques §7 |
| Risk | A light paper page for a dark app: the screenshots look like a different product. Mitigation: mount them on dark plates |

## C. Preview First

**Feeling:** calm, trustworthy, Swiss. A site about one promise: nothing runs until you have read it.

| | |
|---|---|
| Palette | Neutral near-black `#0B0C0F`, ink at three alphas, one red `#D4504A` (the app's accent) for warnings and the button |
| Type | **Geist** + **Geist Mono** only (craft §1 strategy 2) |
| Hero | Centred, very short: "See it before it runs." + download. The app screenshot below |
| Signature | **The plan builds itself.** A pinned uninstall plan sheet assembles line by line as you scroll: command, sizes, deny-list check stamped, APT simulation compared, Polkit dialog, done. One pin |
| Steals | 5, 7, 8, 9, 10, 12 |
| Risk | Leads with safety, so the "one list for everything" benefit arrives late. Mitigation: the fact strip names the 12 sources first |

---

## Recommendation

**A, Night Survey, at Crafted tier.** It keeps visual continuity with the app (people download something that looks like the page), reuses the replica already built in `docs/site.js`, and its signature is Scope's real behaviour rather than a borrowed effect (craft §4). B is the most distinctive but splits the brand from a dark app; C is the safest and the least memorable.
