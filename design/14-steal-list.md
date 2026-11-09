# 14 Steal list: Scope for Linux website

Phase 1 artifact. Every decision in `02-art-direction.md` traces to a numbered move below or is labelled `original`.

Researched 2026-11-06. Job: turn an Ubuntu user who lost track of apps across APT, Snap, Flatpak, AppImage and dev-tool managers into a `.deb` download. Archetype: SaaS and developer tools (`site-archetypes.md` §2).

Method: headless Chrome at 1440×900, computed styles read from the DOM (h1, first long paragraph, body background, colour frequency), plus a viewport screenshot. Motion was not watched frame by frame; motion notes come from DOM evidence and are labelled. Two fetches failed and were replaced: `flattool.github.io/Warehouse/` returned a GitHub Pages 404, and `collection.cooperhewitt.org` (planned adjacent vertical, a museum collection catalogue) returned a Cloudflare challenge. `mijorus.it/projects/gearlever/` rendered only a title and prev/next links, so it is recorded for tone, not layout.

Baseline (the current Scope site, not a steal): Bricolage Grotesque 700 h1 "Everything you installed. One list." at about 88px, Work Sans 19.5px body, `#1B1214` burgundy ground, `#D4504A` accent, centred hero, and an interactive HTML replica of the app window below the fold line. It is good work; the redesign keeps what earns its place.

## Sites examined

| # | Site | Stack | Type | Palette (observed) | Memorable thing |
|---|---|---|---|---|---|
| 1 | zed.dev (ecosystem: Scope is built on Zed's GPUI) | Next.js | h1 IBM Plex Serif italic 48px / 340, -0.96px; body custom "writer" 16px | light paper, ink `rgb(36,37,41)`, signal blue `lab(36 31 -81)` | Blueprint grid with small crosshair marks at intersections; keycap chips inside buttons ("Download now `D`", "Clone source `C`"); a three-up fact strip (Fast / Agentic / Collaborative) on hairlines; product UI rebuilt in HTML |
| 2 | ghostty.org (native Linux app, peer) | Next.js | no h1; Pretendard 16px | `#0F0F11` ground, `#3551F3` blue, `#959597` body | The hero is a single native app window and nothing else: one sentence and two buttons below it. The window content is the brand (animated ASCII) |
| 3 | usebottles.com (Linux desktop app, peer) | Vite + Tailwind CDN | Inter 128px / 800, -6.4px; sub 30px (ratio 4.3x) | white, zinc `#18181B`, faint grid paper | `RELEASE 67.6` eyebrow and the version in the h1 ("Bottles 66"): the release is the news. Two real app windows overlap under the fold |
| 4 | flathub.org (ecosystem: where many Scope apps come from) | Next.js | Inter 24px / 700 | white, ink `oklch(0.26 0.04 297)`, blue `oklch(0.50 0.13 252)` | Real app icons are the only imagery, and they are instantly recognisable. Search field with a `/` keycap |
| 5 | oguzhaninan.github.io/Stacer-Web (direct competitor: Linux system utility) | static, jQuery-era | Product Sans 32px / 700 | white, ink `#3E396B`, violet `#7642FF`, purple blobs | Install commands printed in the hero as copyable code. Otherwise the counter-example: purple blobs, faded body text, press-logo wall |
| 6 | missioncenter.io (Linux system utility) | Bootstrap 5.3 from CDN | no h1 | white, `#212529`, Bootstrap blue `#0D6EFD` | Counter-example: an unchanged Bootstrap theme. Says nothing about the app on first screen |
| 7 | linear.app (ecosystem quality bar) | Next.js | Inter Variable 64px / 510, -1.41px; body 15px | `#08090A` ground, `#F7F8F8` ink, `#8A8F98`, `#62666D` | h1 text appears 3x in the DOM: split-line masked reveal with an accessible copy (DOM evidence). Greys as steps of one ink |
| 8 | mijorus.it/projects/gearlever (direct competitor for AppImages, and Scope is compatible with it) | Hugo | Inter 32px / 800, -1.28px | white, `#111` | A maker's plain project page: name, author, prev/next. Honest and small; nothing pretends to be a company |
| 9 | rubinobservatory.org (adjacent vertical: an observatory running a sky survey) | Next.js | Source Sans 3, 22px body | sky photography, `#1F2121`, teal `#058B8C` / `#30E0E3` | Survey-coverage overlay: hexagonal field tiles drawn over the night sky, and four numbered circles as the reading order. A survey is an inventory of everything in the field of view |

## Moves to steal

| # | Move | From | Category | How it adapts here |
|---|---|---|---|---|
| 1 | The app window is the hero object | 2 | layout | Keep the working HTML replica of the Scope window (already built in `docs/site.js`) and promote it to the first screen. One sentence and two buttons sit with it, nothing else |
| 2 | Real app icons as the only imagery | 4 | content | The replica and any scene use the real icons in `docs/demo/` (Firefox, Brave, Bitwarden, Cursor, App Center…). Visitors recognise their own machine |
| 3 | The release is the eyebrow | 3 | content | A mono eyebrow: `v0.3.8 · Ubuntu x86-64 · MIT`. One source of truth in the HTML, checked against GitHub Releases by a script before deploy |
| 4 | Keycaps inside controls | 1, 4 | interaction | `/` focuses the replica's search (shown as a keycap in its field). Only shortcuts the site really implements |
| 5 | Three-up fact strip on hairlines | 1 | layout | Directly under the hero: `12 sources`, `A preview before every change`, `Polkit, never your password`. Facts, not features |
| 6 | Survey field with coverage tiles | 9 | motion / signature | The scan: a reticle sweeps a field of real app icons grouped by source; each found icon is counted into the one list. It is what Scope literally does on launch (cached rows, then inventory, then update results) |
| 7 | Numbered reading order | 9 | layout | Plan → Confirm → Revalidate → Run as `01`–`04` in mono, one row each, on hairlines |
| 8 | Install commands as copyable code | 5 | content | The `.deb` install line `sudo apt install ./scope_0.3.8_amd64.deb` sits under the download with a copy button. The one good idea on Stacer's page |
| 9 | Split-line masked reveal with an accessible copy | 7 (DOM evidence) | motion | SplitText `mask: "lines"` on the h1 and section h2s, `aria-label` on the parent |
| 10 | Greys as steps of one ink | 7 | colour | The app already has `text` / `text_dim` / `text_faint` (`#EFE6E4`, `#BCA5A1`, `#9E8581`). Express them as one warm ink at three alphas plus hairline |
| 11 | Plain maker voice | 8 | content | "Built by Khurram Bhutto" in the footer with GitHub and LinkedIn; issues link phrased as an invitation. Gear Lever named where Scope is compatible with it |
| 12 | Product UI rebuilt in HTML, not a flat image | 1 | technique | Already true of the replica; extend it to the uninstall preview modal so the safety story is shown in the app's own UI, crisp at any zoom |

## Table stakes

What every peer does. Must have, earns no credit.

- Download in the nav, visible at all times, including phones
- Product visual on the first screen
- Supported-sources table, install steps, FAQ, licence, GitHub link
- `SoftwareApplication` + `FAQPage` JSON-LD (already present; update to v0.3.8 and 12 sources)

## Deliberate deviations

- **No purple, no blue.** Stacer, Bottles' buttons and Flathub lean violet or blue; Zed and Ghostty are blue. Scope is burgundy and the telescope's signal red, from the app and its icon.
- **No Inter.** Five of nine sites use it. Scope's app ships Work Sans (`assets/fonts/`); the site uses the app's own face.
- **No app-store browsing grid.** Flathub sells new apps; Scope shows the ones you already have. The icons appear in one list, the app's own layout, never a card grid.
- **No press-logo wall or star counter.** Real numbers only, fetched live, or nothing.

## Rejected

| Move | From | Why rejected |
|---|---|---|
| Version number as the h1 ("Bottles 66") | 3 | Scope is pre-1.0 with no release brand; the eyebrow (move 3) carries it |
| News carousel | 3 | One maintainer, no news cadence. GitHub Releases is the changelog |
| ASCII animation in the window | 2 | Ghostty's signature, and terminal-native. Scope is a GUI; its window shows its own list |
| Light blueprint grid background | 1, 3 | Becoming a trend across dev-tool sites, and it fights the app's dark burgundy |
| "Trusted by" logo strip | 1, 5 | Nothing honest to show |
| Language toggle | 9 | English only; `lang` set so it can be added |
