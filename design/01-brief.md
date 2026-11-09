# 01 Brief

## What it is

The public website for Scope for Linux, a free, open-source (MIT) desktop app that puts every installed app, tool and game on a Linux machine into one searchable list, and previews every update or uninstall before it runs. Native Rust on Zed's GPUI, no webview. The site replaces the README as the first thing a stranger sees, and has to turn someone who searched "uninstall apps ubuntu", "snap flatpak apt manager" or "linux app manager gui" into a download within one screen.

**Archetype:** SaaS and developer tools (`site-archetypes.md` §2), as a native desktop utility.
**Tier:** **Crafted** (skill default; the owner has not asked for Showpiece).

Site: `https://khurrambhutto.github.io/scope/`, served by GitHub Pages from `main:/docs`. Static HTML, no build today.

## Conversion

**Primary:** Download the `.deb` for Ubuntu (v0.3.8, x86-64), then `sudo apt install ./scope_0.3.8_amd64.deb`.

Secondary, ranked:
1. Download the AppImage (other x86-64 distros)
2. Star the repo on GitHub
3. Read how safety works (`safety.html`) — the trust step before letting an app uninstall packages
4. Report an issue / contribute

Visitors who cannot convert (phones, macOS, Windows, ARM, Fedora/Arch): their conversion is **Star on GitHub** or **copy the link**. The site says so plainly instead of hiding the download.

## Audience

| Who | Knows | Device | Context |
|---|---|---|---|
| Ubuntu desktop users who installed from four places and lost track | `apt list`, Snap Store / App Center, Flathub | Desktop Ubuntu x86-64 | Disk is full, or an app they removed is still there; searching how to uninstall |
| Developers with tools spread across uv, pipx, Cargo, npm and Bun | Each tool's own list command | Desktop Linux | Want one place to see and update everything, including CLI tools |
| Cautious Linux users and distro-hoppers | That a wrong `apt remove` can take the desktop with it | Desktop | Will read the safety page before trusting it |
| Link-followers from Reddit, Hacker News, GitHub | Nothing | ~40% phones | Cannot install from a phone; star or come back |

## Facts the site may state (checked 2026-11-06 against README, `src/` and GitHub)

- Version **0.3.8**, released 2026-10-29. Assets: `scope_0.3.8_amd64.deb`, `Scope-0.3.8-x86_64.AppImage`, `SHA256SUMS`, `SHA256SUMS.minisig`.
- Sources: APT, Snap, Flatpak (update + uninstall); uv, pipx, Cargo, npm global, Bun global (update + uninstall); AppImage in `~/AppImages` (managed, Gear Lever-compatible; update when the AppImage publishes update info); other AppImages and unmanaged launchers (discovery only); Steam games (read-only, open in Steam); JetBrains Toolbox IDEs (read-only, open Toolbox).
- Every update/uninstall is a previewed plan; admin commands go through Polkit (`pkexec`); deny-list protects critical packages; APT uninstall purges package config but leaves home-directory data; Scope never runs autoremove.
- Opens downloaded `.deb` and AppImage files with an install preview ("Open With → Scope").
- Official `.deb` builds self-update after verifying a minisign-signed SHA-256 manifest.
- Ubuntu x86-64 is the primary target. No `.rpm`, no Fedora/Arch package-manager support yet.
- 26 GitHub stars (do not print; fetch live or omit).

## Success criteria

- Lighthouse mobile: Performance ≥ 90, Accessibility 100, Best Practices 100, SEO 100 on every page
- LCP < 2.5 s, CLS < 0.05, INP < 200 ms on a mid-range phone profile
- The download is reachable from the first screen on desktop; the source list is one scroll away
- Every page keeps a unique title, description, canonical, OG image and valid JSON-LD (the current site already has these; keep parity)
- Copy and JSON-LD match v0.3.8 facts (the live site still says 0.3.7 and four sources)

## Non-goals

- No accounts, newsletter, analytics or cookies (none today; keep it that way)
- No fake numbers, testimonials or "trusted by" logos
- No blog; GitHub Releases is the changelog
- No Fedora/Arch/macOS messaging beyond one honest FAQ answer
- No build step unless the dossier argues for one (Pages serves `docs/` as-is)

## Locked constraints

- Keep intact in `docs/`: `BingSiteAuth.xml`, `a047ee63da15ed50edadb8db092ddc9f.txt`, the `google-site-verification` meta tag, `robots.txt`, `sitemap.xml`, `llms.txt`, `llms-full.txt`, and the URLs `/`, `/install.html`, `/safety.html`, `/404.html`.
- Brand: the red telescope mark (`docs/scope-mark.svg`, `#E10600` on a warm white tile) and the app's burgundy palette (`src/theme.rs`).
- Real product claims only: every feature named exists in v0.3.8. Real screenshots and real app icons only (`docs/demo/`, `docs/image.png`, `docs/app-details.png`).
- AGENTS.md design principle applies to copy: cut taglines, helper text and secondary labels.
