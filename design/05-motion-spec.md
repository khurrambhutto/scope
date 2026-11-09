# 05 Motion spec

Library floor: GSAP 3.13.0 + ScrollTrigger + SplitText and Lenis 1.3.11 from jsDelivr, pinned. One rAF loop: Lenis runs on `gsap.ticker`. Every timeline lives inside `gsap.matchMedia().add("(prefers-reduced-motion: no-preference)", …)`. If the CDN fails, `site.js` still runs the replica, tabs, copy buttons and FAQ, and all content is visible (the reveal-hiding CSS only applies under `html.motion`, which is set by `motion.js` after GSAP loads).

## Scroll

Lenis `{ lerp: 0.1, smoothWheel: true, syncTouch: false }`, created only for `(pointer: fine)`. `lenis.on("scroll", ScrollTrigger.update)`, `gsap.ticker.lagSmoothing(0)`. Stopped while the replica's modal or the mobile menu is open. Anchor links scroll through Lenis with the nav height as offset.

## Hero load (at `document.fonts.ready`)

| t (ms) | What |
|---|---|
| 0 | Nav `y -8, opacity 0 → 1`, 600, expo.out |
| 60 | Eyebrow `opacity 0 → 1`, 500 |
| 120 | h1 lines mask-reveal `yPercent 110 → 0`, 1000, expo.out, stagger 0.08 |
| 420 | Lede `y 12, opacity 0 → 1`, 700 |
| 520 | Buttons and install line `y 12, opacity 0 → 1`, 700, stagger 0.06 |
| 600 | Replica `y 40, opacity 0 → 1`, 1100, expo.out; its first scan starts when 25% is in view (existing behaviour) |

Perceived total about 1.1 s, overlapped.

## Inventory detail

| # | Item | Trigger | Tween | Reduced motion |
|---|---|---|---|---|
| 1 | Reveals | `[data-reveal]` enters at 85% | `y 24, opacity 0 → 1`, 700, expo.out; children with `[data-reveal-stagger]` stagger 0.06 | visible, no tween |
| 2 | Hover | pointer | buttons `translateY(-1px)`, ledger rows ink-2 → ink, 160 | colour change only |
| 3 | Press | `:active` | `translateY(1px) scale(0.98)`, 160 | none |
| 4 | Wipe underline | hover/focus on `.text-link` | `::after scaleX 0 → 1`, origin left, 320 | underline always shown |
| 5 | Lenis | load | see Scroll | not created |
| 6 | SplitText | fonts ready; h2 at 85% | lines `yPercent 110 → 0`, 900, stagger 0.08, `mask: "lines"` | not split |
| 7 | Hero load | fonts ready | table above | all visible |
| 8 | Plan builds | `#plan` pinned, ≥ 961px | see Signature | full sheet shown, all steps active |

## Signature: the plan builds

Desktop (≥ 961px, no reduced motion): `#plan .plan-stage` pins for `+=220%` of the viewport. One timeline, `scrub: 0.6`. The plan sheet has 4 groups, one per step; each group's lines rise `y 10, opacity 0 → 1` in order; the matching step in the left list gets `.is-active` (ink, a 2px signal bar) while earlier steps get `.is-done` (ink-2 with an ok tick). Timeline:

| Progress | Step | What appears |
|---|---|---|
| 0–0.25 | 01 Plan | group 01 (heading, APT purge command, simulation, size) is visible from the start so the sheet is never empty; the `Deny-list clear` stamp lands (`scale 1.25 → 1, rotate -8 → -3`) at 0.08 |
| 0.25–0.45 | 02 Confirm | `Plan expires in 5:00 · runs once`, Cancel / Confirm buttons; Confirm gets a pressed state |
| 0.45–0.75 | 03 Revalidate | four ok checks, one at a time |
| 0.75–1 | 04 Run | Polkit line, progress line, `Cursor was uninstalled` |

Below 961px and under reduced motion: no pin; the step list stacks above the full sheet, every line visible, and the four sheet groups reveal one after another as they enter (item 1).
