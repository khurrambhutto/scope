# 06 Page specs

All pages share the nav, footer, tokens and `styles.css`. Each keeps its URL, title, description, canonical, OG/Twitter tags and JSON-LD (updated to v0.3.8). Every page works with JavaScript off or the CDN blocked.

## `/` home

0. **Nav** (fixed, 64px): Scope mark + "Scope", links How it works (`#plan`), Sources (`#sources`), Safety (`safety.html`), Install (`install.html`), GitHub icon link; Download button always visible. ≤ 860px: links move into a menu button (Escape closes, focus returns).
1. **Hero** (`#top`), centred:
   - Eyebrow (mono): `Scope for Linux · v0.3.8`
   - h1: "See it before it runs."
   - Lede: "Every app, tool and game on your Linux machine in one list. Update or uninstall any of them after reading exactly what will happen."
   - Buttons: **Download for Ubuntu** (direct `.deb` link) · **Get the AppImage** (direct link)
   - Install line with Copy: `$ sudo apt install ./scope_0.3.8_amd64.deb`, then a small note "Ubuntu on x86-64 · Free and open source, MIT"
   - The working replica (`#demo`), full width up to 1080px, with a caption: "A working replica. Search, open a row, then try Uninstall or Update. Nothing runs on your computer." The search field shows a `/` keycap; pressing `/` outside a text field focuses it.
2. **Fact strip**, three cells on hairlines: `12` "sources in one list" · `1` "plan before every change" · `0` "passwords seen by Scope".
3. **How a change works** (`#plan`), the signature (05):
   - h2: "Nothing runs the moment you click"
   - Steps 01 Plan · 02 Confirm · 03 Revalidate · 04 Run, each a one-line h3 and one sentence (copy below)
   - The plan sheet (product surface)
4. **Sources** (`#sources`):
   - h2: "Apps, dev tools and games in one place" (the long search phrase lives in the title and description instead; see ADR-8)
   - Intro: "Scope calls each package manager with fixed arguments and never edits its database. This is the uninstall command it runs for each source."
   - Ledger of 12 rows: dot, source, what Scope can do, the uninstall command (mono, wraps). Rows from AGENTS.md command table. Footnote rows for read-only sources.
5. **Screenshots**: h2 "Straight from the app", tab switch App list / App details, the two real screenshots in a 12px frame, caption. "Native Rust on Zed's GPUI, no webview."
6. **Safety** (`#safety`): h2 "Built for the machine you depend on", intro, six guarantees as a two-column list on hairlines, link "Read how safety works".
7. **Install** (`#install`): h2 "Install Scope for Linux", tabs Ubuntu .deb / AppImage / From source with the commands, Copy, and a note; button Install guide.
8. **FAQ** (`#faq`): h2 "Questions", native `<details>`, eight questions matching the FAQPage JSON-LD.
9. **Footer**: "Read it. Then run it." + Download for Ubuntu; maker line; links Install, Safety, GitHub, Releases, Report an issue, Full text for AI tools.

### Plan copy

- 01 **Plan.** "Scope builds the exact commands for the app you chose. APT changes are simulated first."
- 02 **Confirm.** "You read the plan and confirm or cancel. A plan runs once and expires after five minutes."
- 03 **Revalidate.** "Right before running, Scope checks the app is still installed, its version has not moved, and the deny-list still allows it."
- 04 **Run.** "Admin commands go through the system's Polkit dialog. Progress shows at the bottom, and you can keep browsing."

### Fact sources

Commands and capabilities: AGENTS.md command table (checked against `src/domain/operations/`). Plan TTL: `PLAN_TTL` 5 minutes in `src/domain/operations/mod.rs`.

## `/install.html`

PageHead (breadcrumb `Scope / Install`, h1 "Install Scope for Linux on Ubuntu", lede), then the article with an on-page index (sticky at ≥ 961px). Sections: Install the .deb (v0.3.8) · Run the AppImage (needs `libfuse2t64`) · Install apps with Scope (Open With → Scope for `.deb` and AppImage files; AppImages go to `~/AppImages`, need `squashfs-tools`) · Updates to Scope · Build from source · Remove Scope · Other distributions.

## `/safety.html`

Same layout. Sections: Every change starts as a preview · Scope checks the system again before it runs · APT changes are simulated first (now purge, autoremove disabled, home data kept) · Admin rights go through the system dialog · Your own tools stay yours (uv/pipx/Cargo/npm/Bun run as you, in recorded roots, never as root) · Critical packages are protected · Deleted files go to the Trash (AppImage uninstall) · Flatpak scope is never guessed · Scope updates are verified too · What these checks do not cover.

## `/404.html`

Nav-less. Mark, h1 "Page not found", one line, button "Go to Scope for Linux". Same tokens, Geist.

## Failure states

- No JS: the replica is rendered by script, so its list holds the real screenshot `image.png` as a fallback, replaced when the replica starts.
- CDN failure: no smooth scroll, no split text, no pin; plan sheet fully visible.
- Image failure: alt text; the frame keeps its aspect ratio.
