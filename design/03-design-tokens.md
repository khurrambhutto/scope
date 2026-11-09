# 03 Design tokens

Source of truth: the `:root` block at the top of `docs/styles.css`. This table is for humans. Components use no raw values.

## Colour (site chrome)

| Token | Value | Use | Contrast on `--ground` |
|---|---|---|---|
| `--ground` | `#0B0C0F` | page | |
| `--raised` | `#131418` | code blocks, plan sheet frame | |
| `--ink` | `#EEECEA` | headings, body emphasis | 16.6 |
| `--ink-2` | `#AAA9A8` (ink 70%) | body text | 8.3 |
| `--ink-3` | `#8A898A` (ink 56%) | labels, captions | 5.6 |
| `--rule` | `rgba(238,236,234,.10)` | hairlines | |
| `--rule-strong` | `rgba(238,236,234,.18)` | ghost button outline, focus-adjacent | |
| `--signal` | `#C9443E` | primary button bg (white text 4.8), stamp, warnings | 4.1 (non-text only) |
| `--signal-hover` | `#B83E39` | primary hover (white text 5.6) | |
| `--accent-text` | `#F08C82` | links, warning text | 8.2 |
| `--ok` | `#7FBF8E` | passed checks in the plan | 9.1 |
| `--focus` | `#F08C82` | 2px focus ring, 3px offset | |

## Colour (product surfaces, from `src/theme.rs`)

`--app-top #2D1414`, `--app-bottom #0B0C0F`, `--app-elev #211719`, `--app-elev2 #291C1F`, `--app-border #4B3034`, `--app-text #EFE6E4`, `--app-dim #BCA5A1`, `--app-faint #9E8581`, `--app-accent #D4504A`, `--app-accent-text #F08C82`, `--app-primary #A83A36`, `--app-primary-hover #BC4540`, `--app-hover #352327`, `--app-selected #3D2529`, `--app-detail #332426`.

Source colours: `--apt #A1352C`, `--snap #2196F3`, `--flatpak #4A154B`, `--appimage #0B8A4F`, `--desktop #7569D2`, `--uv #738D32`, `--pipx #527BA1`, `--cargo #B36C49`, `--npm #CB3837`, `--bun #B6A18B`, `--steam #1B5C8C`, `--jetbrains #7F52FF`. Used only as 8px dots beside a source name, inside product surfaces and the sources ledger; the app uses the same colours.

## Type

| Token | Value |
|---|---|
| `--sans` | `"Geist", system-ui, sans-serif` |
| `--mono` | `"Geist Mono", ui-monospace, monospace` |
| `--step-h1` | `clamp(3.25rem, 9vw, 8.5rem)`, weight 600, tracking -0.045em, line-height 0.98 |
| `--step-h2` | `clamp(2.25rem, 4.6vw, 4rem)`, weight 600, tracking -0.035em, line-height 1.02 |
| `--step-h3` | `1.25rem`, weight 600, tracking -0.015em |
| `--step-lede` | `clamp(1.125rem, 1.5vw, 1.3125rem)`, line-height 1.5 |
| `--step-body` | `1.0625rem` (17px), line-height 1.65 |
| `--step-small` | `0.9375rem` |
| `--step-label` | `0.75rem` mono, weight 500, tracking 0.08em, uppercase |
| `--step-code` | `0.875rem` mono |

## Space and layout

`--gutter clamp(16px, 2vw, 24px)`, `--max 1200px`, `--section clamp(5rem, 10vw, 9rem)`, `--nav 64px`, spacing steps `--s1 .25rem` … `--s8 4rem` (×2 each step from `--s3 .75rem`: `.25 .5 .75 1 1.5 2 3 4` rem).

## Radius and shadow

`--r-chrome 4px`, `--r-pill 999px`, `--r-product 12px`, `--shadow-stage 0 40px 120px -20px rgba(0,0,0,.7)` (the replica only).

## Motion

| Token | Value |
|---|---|
| `--ease-out` | `cubic-bezier(0.16, 1, 0.3, 1)` (expo.out) |
| `--ease-io` | `cubic-bezier(0.77, 0, 0.175, 1)` |
| `--ease-in` | `cubic-bezier(0.5, 0, 0.75, 0)` |
| `--dur-1` | `160ms` (hover, press) |
| `--dur-2` | `320ms` (underline wipe, state changes) |
| `--dur-3` | `700ms` (reveals) |
