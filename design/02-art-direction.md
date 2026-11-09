# 02 Art direction: Preview First

Approved 2026-11-06 by the owner at Gate 1 ("c i like"). The authority on every visual question. Tier: **Crafted** (skill default; the owner did not ask for Showpiece).

> **Signature move.** The page shows Scope's promise instead of stating it: a real uninstall plan builds itself as you scroll. The exact command appears, the APT simulation runs, the deny-list check is stamped, the plan waits for confirmation, Scope revalidates the system, and only then does the Polkit dialog run it. Four numbered steps beside it light up in order. Nothing on the page runs before you have read it, and neither does anything in the app.

| Field | Decision | Source |
|---|---|---|
| Name | Preview First | original |
| Feeling | Calm, exact, trustworthy. A tool for the machine you depend on, explained by someone careful | original |
| Palette | Site chrome is neutral: `--ground #0B0C0F` (the app window's bottom background stop), ink `#EEECEA` as an opacity ladder (100 / 70 / 56%), hairline ink at 10%. One red: `--signal #C9443E` (the app's `danger()`), for the primary button, warnings and the stamp. `--accent-text #F08C82` (the app's `accent_text()`) for links and warning text. `--ok #7FBF8E` only for passed checks in the plan | steal #10, app `src/theme.rs`, contrast measured (see 03) |
| Product surfaces | The replica window, screenshots and plan sheet keep the app's burgundy (`#211719` → `#0B0C0F`, `#EFE6E4` text). They are the product, not site chrome, so they look exactly like the app | original, steal #12 |
| Type | **Geist** 400/500/600 for everything, **Geist Mono** 500 for labels, commands and numbers | craft §1 strategy 2 |
| Type scale | h1 `clamp(3.25rem, 9vw, 8.5rem)` against a 17px body: **8x** at desktop. Phones get their own step so the h1 holds two lines | anti-slop §4 |
| Grid | 12 columns, `clamp(16px, 2vw, 24px)` gutter, 1200px max, 16px side gutter on phones | original |
| Density | Spacious, section padding `clamp(5rem, 10vw, 9rem)`. Two places break density on purpose: the sources ledger (12 dense rows) and the footer | anti-slop §3 |
| Corners | Two radii: **4px** for site chrome (buttons are pills, the one exception, matching the app's pill buttons), **12px** for product surfaces, matching the app window | anti-slop §1 |
| Borders | No cards. Hairlines separate rows (ledger, steps, FAQ, guarantees, fact strip) | steal #5, anti-slop §1 |
| Shadows | One: the replica window's deep ambient shadow, argued: it is a lit object on a dark set. Nothing else | anti-slop §1 |
| Texture | None on the ground. The product is the only texture | original |
| Hero | Centred and compact: mono eyebrow `Scope for Linux · v0.3.8`, h1 "See it before it runs.", one sentence, Download for Ubuntu + Get the AppImage, the copyable install line, then the working replica directly below | techniques §1 centred, steals #1, #3, #8 |
| Nav | Solid ground bar with a hairline once scrolled, 64px. Download always visible, phones included. Mobile menu closes on Escape | techniques §4 |
| Footer | Compact: closing line "Read it. Then run it." echoing the hero, Download button, maker line, links | techniques §3 compact, steal #11 |
| Cursor | Native | tiers.md Crafted |

## Motion inventory (Crafted, 8)

1. Scroll reveals, `expo.out`, 0.06s stagger, `[data-reveal]`
2. Hover transforms on every interactive element
3. Press feedback `translateY(1px) scale(0.98)`
4. Wipe underlines on text links (`scaleX` from the left)
5. Lenis wired to the GSAP ticker; off on touch and under reduced motion
6. SplitText masked line reveal on the h1 and section h2s, after `document.fonts.ready`, `aria-label` on the parent
7. Choreographed hero load, about 1.1 s perceived (timeline in 05)
8. **Signature:** the plan sheet builds as you scroll through the pinned `#plan` section (desktop ≥ 961px). The one pin on the page

The replica window keeps its own interactions (search, open a row, preview, confirm, footer progress). Those are product behaviour, not decorative motion.

## Anti-list

- No "AI" anywhere in headlines
- No app-store grid of icons. Scope shows what you already have, in its own list
- No fake numbers. The version is a fact; stars are not printed
- No "Get Started". Buttons name the action: "Download for Ubuntu", "Get the AppImage"
- No promise the app cannot keep: no "safe", "never breaks". Say what the checks do and what they do not cover

## Forbidden (anti-slop §1, adapted)

No purple or blue anywhere. No gradient text. No Inter. No three-card grids or card borders. No soft drop shadows beyond the replica's. No bounce or elastic easing. No emoji as icons (SVG at 1.5px stroke). No stock imagery. No lorem ipsum. No `border-radius: 8px`. No light mode (one mode done well; the app is dark).

## Risk

Leading with safety could hide the "one list for everything" benefit. Mitigation: the lede names apps, tools and games; the replica (a list of real apps) is on the first screen; the fact strip says "12 sources" before the plan section starts.
