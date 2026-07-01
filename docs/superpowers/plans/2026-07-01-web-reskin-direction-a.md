# Web Reskin (Direction A "Quiet Precision") Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reskin the `palugada web` console from the comic "Pop Workbench" theme to Direction A "Quiet Precision" — a clean, professional dev-tool aesthetic — without changing structure or function.

**Architecture:** The comic identity is token-driven in `src/web/style.css`. Swap token *values* (keeping token *names*), switch typography to a **system font stack** and remove the three bundled comic woff2 files + their Rust serving code, and delete a handful of structural comic devices (halftone texture, `rotate()` tilts, pow-yellow accent, hard offset shadows). `src/web/app.js` needs no edits (zero hardcoded colors/fonts). Semantic color meaning (convention=green, recipe=gold, review=blue, engine=violet) is preserved but desaturated.

**Tech Stack:** Rust (`tiny_http`, `include_str!`/`include_bytes!`), vanilla CSS/HTML/JS. No build step for the frontend.

## Global Constraints

- Rust edition per `Cargo.toml`; **do NOT run `cargo fmt`** (repo hand-formats compact wide-style — documented incident). Match surrounding style.
- `cargo build` must end with **0 warnings**; `cargo test` must pass.
- Frontend verified via `node --check src/web/app.js` and a manual light+dark visual smoke — there is no JS test runner.
- Commit per task with trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- Fonts must remain fully offline: **no external font URLs** (CSP/offline requirement). Direction A uses the OS system font stack; no webfont is bundled.
- Preserve dark mode and the `localStorage palugada-theme` toggle. Keep the four semantic hues visually distinguishable after desaturation.

---

### Task 1: Switch to system font stack; remove bundled comic fonts

**Files:**
- Modify: `src/web/style.css:1-3` (`@font-face`), `:12-13` (`--font-display`/`--font-ui`)
- Modify: `src/web.rs:15-17` (font consts), `:26` (`Route::Font` variant), `:79` (route arm), `:207-219` (font handler), `:733-738` (`font_asset`), `:781-782` (font route tests)
- Delete: `src/web/bangers.woff2`, `src/web/comic400.woff2`, `src/web/comic700.woff2`

**Interfaces:**
- Produces: no bundled-font route; `--font-display`/`--font-ui` resolve to the system stack.

- [ ] **Step 1: Update the two font-route unit tests to expect the fonts are gone**

In `src/web.rs` `route_parses_paths()` (starts `:777`), replace the two woff2 assertions (`:781-782`):
```rust
        // fonts are no longer bundled — .woff2 requests fall through to NotFound
        assert_eq!(route("GET", "/bangers.woff2"), Route::NotFound);
```
(Remove the `comic700.woff2` assertion line entirely.)

- [ ] **Step 2: Run the test to verify it FAILS**

Run: `cargo test web::tests::route_parses_paths` (bin crate — no `--lib`)
Expected: FAIL — the `("GET", [name]) if name.ends_with(".woff2")` arm still returns `Route::Font(...)`.

- [ ] **Step 3: Remove the font-serving code in `src/web.rs`**

- Delete the three consts (`:15-17`):
```rust
const FONT_BANGERS: &[u8] = include_bytes!("web/bangers.woff2");
const FONT_COMIC_400: &[u8] = include_bytes!("web/comic400.woff2");
const FONT_COMIC_700: &[u8] = include_bytes!("web/comic700.woff2");
```
- Delete the `Font(String)` variant from `enum Route` (`:26`).
- Delete the route() arm (`:79`): `("GET", [name]) if name.ends_with(".woff2") => Route::Font(...)`.
- Delete the whole `Route::Font(name) => { ... }` handler block (`:207-219`).
- Delete the now-unused `font_asset` fn (`:733-738`).

- [ ] **Step 4: Delete the woff2 files**

Run:
```bash
git rm src/web/bangers.woff2 src/web/comic400.woff2 src/web/comic700.woff2
```

- [ ] **Step 5: Replace `@font-face` + font tokens in `src/web/style.css`**

Replace the three `@font-face` blocks (`:1-3`) with nothing (delete them). Set the font tokens (`:12-13`) to:
```css
  --font-display: system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --font-ui: system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
```
Leave `--font-mono` as-is (it already uses the system mono stack).

- [ ] **Step 6: Verify build + tests pass, JS still valid**

Run: `cargo build 2>&1 | tail -5 && cargo test web:: && node --check src/web/app.js`
Expected: build 0 warnings (no unused `font_asset`/const), tests pass, `node --check` silent.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(web): system font stack; drop bundled comic fonts

Remove Bangers/Comic Neue woff2 + include_bytes/Route::Font serving; the
professional theme uses the OS system font stack (fully offline, no webfont).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Direction A color palette (light + dark)

**Files:**
- Modify: `src/web/style.css` `:root` block (~`:11-49`) and `[data-theme="dark"]` block (~`:51-76`)

**Interfaces:**
- Produces: Direction A token values under the existing token names.

- [ ] **Step 1: Set light-theme token values (`:root`)**

Set these tokens to the Direction A values (keep every existing token *name*; only change values):
```css
  --ground: #FBFBFC;  --surface: #FFFFFF;  --surface-2: #F4F5F7;
  --ink: #16181D;     --ink-soft: #5C616B; --faint: #8B8F98;
  --border: #E7E8EC;
  --accent: #5B5BD6;  --accent-press: #4A4AC0; --accent-tint: #F0F0FB; --on-accent: #FFFFFF;
  --conv: #157F3B; --conv-bg: #E9F6EE;
  --rec:  #8A6100; --rec-bg:  #FBF1D6;
  --rev:  #2952CC; --rev-bg:  #E8ECFB;
  --eng:  #6244C9; --eng-bg:  #EFEAFA;
  --warn: #8A5300; --warn-bg: #FBF1D6;
  --err:  #C4321F; --err-bg:  #FBE9E7;
  --ok:   #157F3B;
```
(If a token name here differs from the current file, keep the file's actual name and just update its value — match by role. Confirm the exact border/surface token names in the file first.)

- [ ] **Step 2: Set dark-theme token values (`[data-theme="dark"]`)**

```css
  --ground: #0E0F12;  --surface: #16181D;  --surface-2: #1D2026;
  --ink: #E9EAEE;     --ink-soft: #A3A7B0; --faint: #787D87;
  --border: #2A2D35;
  --accent: #8A8AF0;  --accent-press: #9D9DF4; --accent-tint: #1E1E33; --on-accent: #14101F;
  --conv: #54CF8A; --conv-bg: #152C1F;
  --rec:  #E2B455; --rec-bg:  #2C2410;
  --rev:  #8AA6FF; --rev-bg:  #1A2545;
  --eng:  #B49BF5; --eng-bg:  #231A3A;
  --warn: #E2B455; --warn-bg: #2C2410;
  --err:  #FF8A7A; --err-bg:  #34191A;
  --ok:   #54CF8A;
```

- [ ] **Step 3: Verify build + visual**

Run: `cargo build 2>&1 | tail -3`
Then run the console (`cargo run -- web` — note the printed URL) and eyeball Overview + Profiles in **both** themes: cool neutrals, iris accent, semantic hues still distinct. Stop the server.

- [ ] **Step 4: Commit**

```bash
git add src/web/style.css
git commit -m "feat(web): Direction A color palette (light + dark)

Cool grey-blue neutrals + iris accent; semantic hues desaturated but distinct.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Remove comic structural devices (shadows, halftone, rotations, pow-yellow, borders, radii)

**Files:**
- Modify: `src/web/style.css` — shadow tokens (`:44-46`, dark `:75-76`), border weight (`:46`), radii (`--r-sm/--r/--r-lg`), halftone (`:26`,`:58`,`:91-93`), rotates (`:109`,`:284`,`:383`,`:464`), pow tokens+consumers (`:32-33`,`:64-65`), semantic-saturation rules (`:318-321`,`:348-350`)

**Interfaces:**
- Produces: a flat, hairline, elevation-light visual language.

- [ ] **Step 1: Soften shadows + fix hardcoded dark shadow**

Light (`:44-46`): `--shadow: none;` and `--shadow-sm: none;` (Direction A is elevation-free; reserve a soft shadow only if a later overlay needs it). Keep `--bw: 1px;`.
Dark (`:75-76`): set `--shadow`/`--shadow-sm` to the same `none` (removing the hardcoded `#0C0915` hex so light/dark stay in lockstep).

- [ ] **Step 2: Tighten radii**

Set `--r-sm: 6px; --r: 8px; --r-lg: 10px;` (keep `--r-pill: 999px`).

- [ ] **Step 3: Remove halftone texture**

Delete the body background rule (`:91-93`):
```css
  background-image: radial-gradient(var(--halftone) 1.5px, transparent 1.6px);
  background-size: 20px 20px;
```
(Leave the body's `background: var(--ground)`.)

- [ ] **Step 4: Remove the four `rotate()` tilts**

In each of these rules delete the `transform: rotate(...)` declaration: `.brand .mark` (`:109`), `.sticker` (`:284`), `.empty .pow` (`:383`), `.status` (`:464`).

- [ ] **Step 5: Retire pow-yellow**

Recolor `--pow`/`--pow-deep` (`:32-33`, dark `:64-65`) to the neutral accent so its consumers stop reading as pop-art:
```css
  --pow: var(--accent-tint);  --pow-deep: var(--accent);
```
Then check the consumers (`.sticker.new` background, `.step .num` background/border, `.empty .pow` background, `.btn.secondary:hover`/`.ghost-btn:hover`/`.reveal:hover`): the flow-step number badge (`.step .num`) should become a quiet chip — set its `background: var(--surface-2); color: var(--ink); border: 1px solid var(--border);` and drop the `--bw solid var(--ink)` outline.

- [ ] **Step 6: Desaturate semantic tag/origin blocks**

Confirm `.step-tag.convention/.recipe/.review_map/.engine` (`:318-321`) and `.origin-profile/.origin-project/.origin-overridden` (`:348-350`) reference the `--conv/--rec/--rev/--eng` tokens (now desaturated). If any hardcode a saturated hex directly, replace with the corresponding token.

- [ ] **Step 7: Verify devices are gone**

Run:
```bash
grep -n "rotate(\|halftone\|3px 3px 0\|2px 2px 0\|Bangers\|Comic Neue" src/web/style.css || echo "clean"
cargo build 2>&1 | tail -3
```
Expected: no `rotate(`/halftone/hard-shadow/comic-font hits remain (favicon/legit uses aside); build clean. Visual smoke both themes: flat, hairline, no tilts, no yellow, no dots.

- [ ] **Step 8: Commit**

```bash
git add src/web/style.css
git commit -m "feat(web): remove comic devices (halftone, tilts, pow-yellow, hard shadows)

Flat hairline elevation-light language; step numbers become quiet chips.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Professional brand wordmark

**Files:**
- Modify: `src/web/index.html:13`

**Interfaces:**
- Produces: a clean wordmark (no comic emoji mark).

- [ ] **Step 1: Replace the emoji mark**

Change (`:13`):
```html
<div class="brand"><span class="mark">🛠</span><span class="name">palugada</span></div>
```
to a wordmark with a small neutral square glyph (the `.mark` keeps layout but reads as a logo tile, not an emoji):
```html
<div class="brand"><span class="mark" aria-hidden="true">◆</span><span class="name">palugada</span></div>
```
(If `.mark` styling still forces an emoji look, set in `style.css` `.brand .mark { color: var(--accent); font-size: 15px; }` and ensure no `rotate` remains from Task 3 Step 4.)

- [ ] **Step 2: Verify**

Run: `node --check src/web/app.js` (unaffected) and visual: topbar shows a clean `◆ palugada` wordmark in the accent color.

- [ ] **Step 3: Commit**

```bash
git add src/web/index.html src/web/style.css
git commit -m "feat(web): professional brand wordmark (drop emoji mark)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Full-console verification

**Files:** none (verification only)

- [ ] **Step 1: Build + test + JS check**

Run: `cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -5 && node --check src/web/app.js`
Expected: 0 warnings, all tests pass, JS valid.

- [ ] **Step 2: Device grep**

Run: `grep -rn "rotate(\|halftone\|Bangers\|Comic Neue\|--pow\b" src/web/style.css | grep -v "var(--pow" || echo "clean"`
Expected: no comic devices left (only the retired `--pow` token definition mapped to accent, if kept).

- [ ] **Step 3: Visual smoke — every view, both themes**

Run `cargo run -- web`, open the URL, and click through Overview / Projects / Profiles / Profile detail / Knowledge / Connectors in **light and dark**. Confirm: cool neutrals, iris accent, hairline borders, no tilts/dots/yellow, semantic chips distinguishable, wordmark clean. Stop the server.

- [ ] **Step 4: (No commit needed — verification only.)** If any issue found, fix in the owning task's file and amend/commit.

---

## Self-Review notes

- **Spec coverage:** A1 tokens → Task 2; A2 shadows/borders + dark-hex fix → Task 3 Step 1; A3 devices (halftone/rotates/pow/semantic) → Task 3; A4 fonts → Task 1 (system stack instead of bundled IBM Plex — deviation noted: fully offline, no download/license, matches the approved mockup which used system fonts); A5 brand → Task 4.
- **Deviation from spec:** spec recommended bundling IBM Plex Sans; this plan uses the system font stack (removing bundled fonts) for a zero-dependency, fully-offline result identical to the approved mockup. If cross-machine typographic consistency is later wanted, bundling a woff2 is an additive follow-up (re-add `@font-face` + `include_bytes!` + a `Route::Font`).
- **No unit tests for CSS** — deliverables are `cargo build`/`cargo test` (font-route change), `node --check`, grep, and visual smoke. This is inherent to a CSS reskin.
