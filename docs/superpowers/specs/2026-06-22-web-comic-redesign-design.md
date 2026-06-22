# palugada web — "Pop Workbench" comic redesign

**Date:** 2026-06-22
**Status:** Approved (design mockup signed off; v3 "elegant comic")
**Scope:** Restyle the `palugada web` console — bright, light-first, comic/pop-art identity with a dark toggle. CSS rewrite + targeted structural changes to the existing vanilla-JS console.

## Goal

The console works but reads as a bare utility (all-monospace, flat cards, weak hierarchy). Give it a distinctive, polished identity the user explicitly chose: **comic / pop-art, dialed to "elegant"** — not the generic AI-default look.

Visual source of truth: the approved mockup (artifact `5712209a…`, version `v3-elegant-comic`). The mockup's CSS is the reference; this work ports it onto the real DOM, which `app.js` generates.

## Design language — "Pop Workbench"

- **Light-first, airy**, with a **dark toggle** (persisted). Subtle halftone-dot page texture.
- **Type:** `Bangers` (display: wordmark, h1, section titles, stat numbers, button labels, flow names) + `Comic Neue` (body/labels). `ui-monospace` reserved for code/ids/paths. All three bundled as local woff2 — no external font dependency, stays offline.
- **Comic structure, restrained:** 2px ink outlines, small hard offset shadows (`3px`/`2px`, no blur), tactile button "press" on hover/active, numbered flow steps (01→02→03) with a connector — earned because flows *are* sequences.
- **Color:** comic-blue `#2B6CFF` = primary action; pow-yellow `#FFD23F` = sparing signature accent (active-nav left bar, step numbers, "new" sticker). Ink `#1A1722`. Paper `#FCFAF3`.
- **Semantic colors preserved (carry meaning):** conventions=green, recipes=gold, review_map=blue, engine=violet, warnings=amber, errors=red, success=green. Foregrounds tuned to clear WCAG AA on their tinted chips.

## Components (all themed via CSS custom properties; `[data-theme="dark"]` overrides)

topbar (brand + path chip + theme toggle + mobile menu button) · sidebar nav (icons, group label, active = ink border + inset yellow bar) · view head (eyebrow + h1 + subtitle) · cards (`card-head` with title/count/action) · stat grid · key-value rows · list rows (`id-chip` + title + meta + hover/focus/touch-revealed actions) · pills · status stickers · numbered flows · rules table (in a horizontal-scroll wrapper) · origin badges · buttons (primary/secondary/ghost) · form fields · empty states · toast.

## Files changed

1. **`src/web/style.css`** — full rewrite to the Pop Workbench system. `@font-face` points at local `/bangers.woff2`, `/comic400.woff2`, `/comic700.woff2`.
2. **`src/web/index.html`** — topbar gains a mobile menu button + theme toggle; sidebar gets a group label and per-item SVG icons; small boot script sets `data-theme` from `localStorage`.
3. **`src/web/app.js`** — generated markup updated to the new classes/structure across **every** view (overview, projects, project-detail incl. credentials & rules, profiles, profile-detail incl. conventions/recipes/fact-families/flows/import/generate, knowledge). Adds: numbered flow rendering, `card-head`, `id-chip`, status stickers, `table-scroll` wrapper, theme-toggle + persistence, mobile drawer toggle. **No API or business-logic changes** — same endpoints, same payloads.
4. **`src/web.rs`** — embed the three woff2 via `include_bytes!`; add a `Font(String)` route for `*.woff2`; add a binary response helper serving `font/woff2` with a long `Cache-Control`. Update the route unit test.
5. **`src/web/*.woff2`** — three bundled OFL fonts (Bangers, Comic Neue 400/700).

## Constraints / non-goals

- No framework, no build step (vanilla JS, embedded assets) — unchanged.
- No new HTTP endpoints beyond static font routes; no change to JSON API shapes.
- Loopback-only server and host check — unchanged.
- Keyboard reachability for nav/rows and reduced-motion respected (carried from the design-critique pass).

## Verification

- `cargo build` clean; `cargo test` (route test updated) passes.
- Run `palugada web`, load the console: every view renders; light↔dark toggle works and persists; fonts load from local routes (no network); flows show numbered steps; mobile width (<880px) exposes the drawer and the rules table scrolls without the page scrolling.
