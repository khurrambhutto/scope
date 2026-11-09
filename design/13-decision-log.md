# 13 Decision log (and stack, performance, plan)

- ADR-0 (2026-11-06) The dossier lives in `design/`, not `docs/`, because GitHub Pages serves `main:/docs` and the dossier is not part of the public site. Work happens on branch `site/redesign-dossier`.
- ADR-1 (2026-11-06, owner, Gate 1) Direction **C, Preview First**, chosen over A Night Survey (recommended) and B Field Guide. Tier **Crafted**: the owner named no tier, so the skill default applies. Remaining gates not requested; building proceeds.
- ADR-2 Product surfaces (replica, screenshots, plan sheet) keep the app's burgundy palette; site chrome is neutral near-black. Reason: visitors should see exactly the app they download. Rejected: restyling the replica neutral, which would misrepresent the app.
- ADR-3 Primary red is `#C9443E` (the app's `danger()`), not the mock's `#D4504A`: white text on `#D4504A` measures 4.16:1 (fails AA); on `#C9443E` 4.79:1.
- ADR-4 Stack: static HTML/CSS/JS in `docs/`, no build step. GSAP 3.13.0 (ScrollTrigger, SplitText) and Lenis 1.3.11 from jsDelivr, pinned. Fonts: Geist and Geist Mono from Google Fonts with `display=swap` and preconnect, as the current site already loads Google Fonts. Rejected: Astro (adds a build to a 4-page site served from `main:/docs`); self-hosted fonts (no binary downloads needed for parity).
- ADR-5 Keep and rework the existing replica in `site.js`: update its source data to v0.3.8 (12 sources, purge command, managed AppImages, npm/Bun/Steam/JetBrains), keep its behaviour. Motion code moves to a separate `motion.js` so the replica works without the CDN.
- ADR-6 Content is corrected to v0.3.8 everywhere (live site says v0.3.7, four sources, "apt remove", AppImage "listed only"). Sources of truth: README.md, AGENTS.md command table, `src/domain/operations/`. `llms.txt`, `llms-full.txt`, `sitemap.xml` updated to match.
- ADR-7 The site rulebook is `design/CLAUDE.md`, with a pointer from AGENTS.md. Reason: `docs/` is published, so a rulebook there would be served publicly.

- ADR-8 (build) The sources h2 is "Apps, dev tools and games in one place". The keyword-heavy phrase from 06 wrapped to four lines at the h2 size; the search terms are carried by the title, meta description and the ledger rows instead.
- ADR-9 (build) The plan sheet shows group 01 from the start of the pin; only the stamp and groups 02–04 animate. An empty sheet at pin start read as broken. Updated 05.
- ADR-10 (build) A head script adds `html.motion` before first paint (no reduced motion) and removes it after 2.5 s if GSAP never loads, so hidden-before-reveal content can never stay hidden.

## Validation (2026-11-06, local `python3 -m http.server`, headless Chrome)

- Viewports 1920×1080, 1440×900, 1024×600, 768×1024, 390×844, 320×568, 844×390 on `/`, `/install.html`, `/safety.html`: no horizontal overflow (also after scrolling the full page), nav Download visible, no hidden content after scrolling, no console or page errors. One overflow at 390 (install command `white-space: pre` widening a grid track) was found and fixed with `min-width: 0` on grid children.
- Signature at 1440: the stage pins at 64px through the section; active step goes 1 → 2 → 3 → 4 with scroll; sheet lines shown 4 → 12 of 12.
- Reduced motion (emulated) at 390 and 1440: no pin, no Lenis, no `motion` class, all 12 sheet lines and all content visible.
- CDN blocked at 1440: `motion` class removed by the safety net, nothing hidden, replica renders 19 rows.
- Replica: `/` focuses search; searching "cursor" gives 1 result; uninstall preview shows the purge command from AGENTS.md; focus lands on Confirm; Escape closes; confirm ends with "Cursor was uninstalled".
- Keyboard: skip link first, visible focus outline on nav items; mobile menu opens, Escape closes and returns focus.
- Static: one h1 per page, unique titles under 60 chars, descriptions ≤ 160, canonicals, JSON-LD parses on home and install, every image has width/height, all in-page anchors and local links resolve. The 404 page's `/scope/` links only resolve on GitHub Pages (absolute base path), so they were not checked locally.
- Not run: Lighthouse, real-device phones, Firefox/Safari, screen-reader pass.

## Performance budget

Home under 400 KB transfer excluding the two screenshots (lazy). LCP element is the h1 text. Scripts `defer`. Replica icons are 36px PNGs already in `docs/demo/`. Screenshots carry width and height.

## Implementation plan

| Phase | Includes | Done when |
|---|---|---|
| 1 Foundation | tokens, reset, type, nav, footer, buttons in `styles.css` | home renders with nav/footer at 1440 and 390 |
| 2 Static skeleton | all four pages with real v0.3.8 copy, zero animation; replica data updated | screenshots at 1440 and 390 reviewed |
| 3 Scroll layer | `motion.js`: Lenis + GSAP ticker, reveals, reduced-motion gate | reveals run; no console errors |
| 4 Type motion | SplitText on h1/h2 after fonts | line breaks correct at 3 widths |
| 5 Signature | pinned plan builds (05) | observed at 1440; static at 390 and in reduced motion |
| 6 Micro | hover, press, wipe underlines, `/` shortcut, mobile menu | keyboard pass |
| 7 Polish | llms, sitemap, 404, metadata | static checks pass |
| 8 Audit | verification.md | report appended here |
