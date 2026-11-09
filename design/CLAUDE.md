# Scope website rulebook

The public site is static HTML/CSS/JS in `docs/`, served by GitHub Pages from `main:/docs`. `design/` is law for it: 01 brief, 02 art direction, 03 tokens, 05 motion, 06 pages, 13 decisions. Keep intact in `docs/`: `.nojekyll`, `BingSiteAuth.xml`, `a047ee63da15ed50edadb8db092ddc9f.txt`, the google-site-verification meta, `robots.txt`, `sitemap.xml`, `llms.txt`, `llms-full.txt`, and the URLs `/`, `/install.html`, `/safety.html`, `/404.html`.

## Stack
No build. `docs/styles.css` (tokens at the top), `docs/site.js` (replica, tabs, copy, menu: works without the CDN), `docs/motion.js` (GSAP 3.13.0 + ScrollTrigger + SplitText, Lenis 1.3.11, jsDelivr, pinned). Fonts: Geist, Geist Mono.

## Hard rules
1. No raw values in components. Everything resolves to a token in `styles.css :root`.
2. Transform and opacity only.
3. Every interactive element has a visible `:focus-visible` state.
4. Explicit width and height on every image.
5. Every animation has a reduced-motion path inside `gsap.matchMedia` (CSS: media queries). Content is visible without JavaScript.
6. SplitText only after `document.fonts.ready`, `aria-label` on the parent.
7. Facts come from README.md, AGENTS.md (command table) and GitHub Releases. When the version changes, update every `0.3.8` in `docs/` and the JSON-LD.
8. Code diverging from a doc: update the doc, log an ADR in `design/13`, then code.
9. Never claim a test or audit passed without running it.
10. Copy: short, no taglines (AGENTS.md design principle), name real limits plainly, no "AI" headlines.

## Forbidden
No purple or blue. No gradient text. No Inter. No three-card grids or card borders. No soft drop shadows beyond `--shadow-stage`. No bounce or elastic easing. No emoji as icons. No stock imagery. No lorem ipsum. No `border-radius: 8px`. No light mode. No fake numbers or testimonials.

## Signature
The uninstall plan that builds as you scroll through `#plan` (design/05).

## Current phase
See design/13 implementation plan.
