# Design — palugada web: professional reskin + "puzzle/pickup" Create composer

**Date:** 2026-07-01
**Status:** approved (brainstorming) → next: writing-plans
**Branch:** `feat/web-pro-reskin-composer`

## Context

A colleague reviewed the `palugada web` console and found the current **"Pop Workbench" comic/pop-art** theme (Bangers display font, Comic Neue body, halftone dot texture, hard offset shadows `3px 3px 0`, pow-yellow accents, tilted stickers) reads as **unprofessional for a developer tool**. Separately, authoring conventions & recipes today is scattered across several disconnected entry points (Add convention, Add recipe, Import markdown, Override) with **no reuse of existing content** — the only reuse is profile `extends` and flow references by id.

This design covers two threads, delivered together (they touch the same files: `style.css`, `app.js`, `web.rs`, `knowledge.rs`, `inherit.rs`):

- **Thread A — Reskin** the console from comic → a clean, professional dev-tool aesthetic.
- **Thread B — "puzzle/pickup" Create composer**: one **"Create new"** button opening a two-pane composer where the user *picks reusable pieces* (palette) and *assembles* (canvas) a new convention/recipe.

### Brainstorming decisions (locked)

1. **Visual direction: A — "Quiet Precision"** (Linear-lineage): cool grey-blue neutrals, hairline 1px borders, no hard shadows, mono-forward for ids/tokens/counts, a single iris accent, generous whitespace. Chosen from a 3-direction artifact mockup (A / B GitHub-native / C Stripe-enterprise). Mockups (Artifacts): 3-direction comparison `https://claude.ai/code/artifact/2dd72f16-3bea-4f39-a563-0c386077f90a` + composer (Direction A) `https://claude.ai/code/artifact/691d4a53-b23a-4468-845c-0ee62f1c8d1c`.
2. **Reuse primitives (all four):** (a) assemble a convention from **sections** picked across the profile + its inheritance chain; (b) **clone** an existing convention/recipe; (c) pull pieces from **other profiles / the `extends` base**; (d) recipe: pick **convention_refs + related_recipes**.
3. **Interaction: two-pane "puzzle"** — left PALETTE (searchable, grouped by origin, checkable), right CANVAS (the doc being assembled). Behind a single "Create new" button. Picked sections are **copied, not linked** (safe to edit without changing the source).
4. **Recipe refs: fix the latent data-loss bug + store refs in `_index.json`** (the source the engine actually reads). Nested `.md` front-matter (`references: {...}`) stays decorative for now (cleanup deferred).

## Goals

- Replace the comic identity with Direction A tokens/fonts, keeping structure & function unchanged and **preserving semantic color meaning** (convention=green, recipe=gold, review=blue, engine=violet) — just desaturated.
- One "Create new" entry that absorbs the scattered authoring buttons and lets users **assemble** conventions/recipes from reusable pieces.
- Fix the pre-existing recipe-refs data-loss bug as part of the recipe-authoring work.
- No regression: all existing views/routes/tests keep working; dark mode + offline-bundled fonts preserved.

## Non-goals (deferred / YAGNI)

- Drag-and-drop reorder (start with ↑/↓ buttons).
- Cross-profile access control / redaction in the palette (loopback-only trust model; acceptable).
- Palette result caching (fine at current profile scale; lazy-load other profiles instead).
- Mirroring recipe refs into `.md` front-matter + parsing them on import (index-only for now).
- A professional favicon / apple-touch-icon (net-new surface; optional later).
- Per-section "point from a specific place in the recipe body" (recipes have no `{#anchor}` structure; refs point at whole convention + optional section id).

---

## Thread A — Reskin to Direction A

The comic identity is almost entirely **token-driven**, so this is largely a value swap plus removing a few structural devices and swapping the bundled fonts. **`app.js` needs no edits for the reskin itself** (zero hardcoded colors/fonts — all via CSS classes); Thread B separately adds the composer view to `app.js`.

### A1. Token values (`src/web/style.css` `:root` + `[data-theme="dark"]`)

Change **values only**, keeping the existing token **names** (`--ink`, `--ink-soft`, `--faint`, `--conv`/`--conv-bg`, `--rec`, `--rev`, `--eng`, `--warn`, `--err`, `--accent`, etc.). Target values:

**Light** — `--ground/bg #FBFBFC`, `--surface #FFFFFF`, `--surface-2 #F4F5F7`, `--border #E7E8EC`, `--ink #16181D`, `--ink-soft #5C616B`, `--faint #8B8F98`, `--accent #5B5BD6` (iris), `--accent-press #4A4AC0`, `--accent-tint #F0F0FB`, `--on-accent #FFFFFF`. Semantic (fg/bg): conv `#157F3B`/`#E9F6EE`, rec `#8A6100`/`#FBF1D6`, rev `#2952CC`/`#E8ECFB`, eng `#6244C9`/`#EFEAFA`, warn `#8A5300`/`#FBF1D6`, err keep a professional red (`#C4321F`/`#FBE9E7`), ok=conv.

**Dark** — `--ground #0E0F12`, `--surface #16181D`, `--surface-2 #1D2026`, `--border #2A2D35`, `--ink #E9EAEE`, `--ink-soft #A3A7B0`, `--faint #787D87`, `--accent #8A8AF0`, `--accent-tint #1E1E33`, `--on-accent #14101F`. Semantic: conv `#54CF8A`/`#152C1F`, rec `#E2B455`/`#2C2410`, rev `#8AA6FF`/`#1A2545`, eng `#B49BF5`/`#231A3A`, warn `#E2B455`/`#2C2410`.

Radii: tighten — `--r-sm 6px`, `--r 8px`, `--r-lg 10px`, `--r-pill 999px`.

### A2. Shadows / borders

- `--shadow`/`--shadow-sm` (`style.css:44-46`) `Npx Npx 0 var(--ink)` → **soft or none**. Direction A = essentially no elevation; use `--shadow: none` (or a very soft `0 1px 2px rgba(...)` reserved for overlays/toast only).
- **Fix the hardcoded dark shadow hex** (`style.css:75-76`, currently `#0C0915`) in lockstep — derive from ink or set matching neutral. (Risk: easy to miss; part of verification grep.)
- `--bw` (border weight) → keep at `1px` (hairline) — do **not** keep 2px ink outlines.

### A3. Remove comic structural devices

- **Halftone**: delete the body `background-image: radial-gradient(...)` + `background-size` (`style.css:91-93`) or zero `--halftone` alpha in both themes (`:26`, `:58`).
- **Four independent `rotate()` rules** — set to `none`/delete: `.brand .mark` (`:109`), `.sticker` (`:284`), `.empty .pow` (`:383`), `.status` (`:464`). No shared class ties them — grep `rotate(` is part of verification.
- **Pow-yellow**: retire/recolor `--pow`/`--pow-deep` (`:32-33`, `:64-65`) and its consumers (`.sticker.new`, `.step .num`, `.empty .pow`, `.btn.secondary:hover`/`.ghost-btn:hover`/`.reveal:hover`) → neutral or the iris accent. Flow step numbers become quiet mono badges (not yellow circles).
- Re-audit `.step-tag.*` / `.origin-*` semantic color blocks (`:318-321`, `:348-350`) — they're token-driven but read pop-art-saturated; ensure they use the desaturated palette.

### A4. Fonts (CSS **and** Rust)

Swap Bangers + Comic Neue → one professional OFL sans (**recommended: IBM Plex Sans**, dev-appropriate & distinctive; weights 400/600) with `system-ui, sans-serif` fallback; keep `--font-mono` as the system mono stack. Steps:
- Drop woff2 into `src/web/` (e.g. `plexsans-400.woff2`, `plexsans-600.woff2`); delete `bangers.woff2`/`comic400.woff2`/`comic700.woff2` once unreferenced.
- `style.css`: rewrite `@font-face` (`:1-3`); point `--font-display` + `--font-ui` (`:12-13`) at the new family (drop Comic Sans/Impact/fantasy fallbacks). Direction A needs **no separate display face** — headings use the UI face heavier/tighter.
- `web.rs`: replace `include_bytes!` consts (`:15-17`), the `Route::Font` filename match (`:207-211`), and the two `route()` unit-test assertions (`:781-782`). `Route::Font` dispatch (`:79`, generic `*.woff2`) and `font_asset()` (`:733`, `font/woff2`, immutable cache) need **no change**.
- **Confirm license** permits static `include_bytes!` embedding + redistribution in the binary (IBM Plex = OFL, OK). Add attribution file if required.

### A5. Brand mark

`index.html:13` — replace the hardcoded `🛠` emoji mark with a clean wordmark (or a neutral inline SVG). Theme bootstrap script (`:10`) is unaffected.

---

## Thread B — "puzzle/pickup" Create composer

### B0. Entry point & scope

A single **"+ Create new"** button on the **Profile detail** view opens the composer as a full **view** (two-pane needs the width). It **absorbs** the scattered `Add convention` / `Add recipe` / `Override` affordances (the composer covers create + clone + override via a locked-id). `Import markdown` may fold into the composer as a mode or stay as-is (decide in plan — low coupling). Context = the profile detail's profile; composer includes a profile switcher.

### B1. Palette resolver (new)

**New function** `palette_sections(kn, active_profile) -> Vec<PaletteEntry>` (place in `inherit.rs`, which already owns chain/provenance, or a new `src/palette.rs` if it grows). `PaletteEntry { source_profile, topic_id, section_id (anchor), title, tokens, origin ("own"|"overridden"|"inherited"|"other-profile"), from, body }`.

Assembly (all reuse existing fns):
- **Active profile + chain**: `merged_conventions_provenance(kn, active)` → topics with per-section `origin`/`from` (metadata only, no bodies). For each topic, `resolve_convention_raw(kn, active, topic)` → strip front-matter → `parse_sections()` → `{anchor,title,body}`; **join bodies onto provenance sections by id==anchor** (both are `slug(title)` or explicit `{#anchor}` → they align).
- **Other profiles**: `profile::list(kn)` minus `resolve_chain(active)`; for each, read its own conventions (`conventions_in`/`convention_md_in` + `parse_sections`), tag `origin="other-profile"`, `from=<id>`.
- **Perf**: profile+chain eager (cheap, one pass); **other profiles lazy** — the palette lists them collapsed; bodies (and possibly section lists) fetched on group-expand. Exact endpoint shape (single `Route::Palette` with a `scope`/profile param, vs eager) decided in plan.

**Route** `GET /api/profile/{id}/palette` via `read()` (500 on error), following the `profile_json` precedent. Add `route()` unit-test asserts.

**Join risk**: `SectionMeta.id` (from `_index.json`, hand/generator-authored) and `MergedSection.anchor` (parsed live from `.md`) are two sources of truth; if they drift, the join silently drops a section's body. `profile validate` already checks md⇔index section-id parity (`profile.rs`) — verify coverage; the resolver should skip-with-warning (not crash) on a missing body.

### B2. Save a composed **convention** — reuse existing writer

The canvas is a list of `{title, body}` sections + id/title/tags — **exactly `ConventionSpec { id, title, description, tags, sections: Vec<SectionSpec{title, body, code}> }`**. So **reuse the existing `POST /api/profile/{id}/convention` (`AddConvention` → `knowledge::add_convention`/`add_convention_in`)** — no new convention writer or route. Blank sections = `SectionSpec` with empty body.

Guards (composer/handler side):
- Pre-validate the new id with `valid_doc_id` (fail fast, clear message).
- **Dedup section titles**: two picked sections with the same title slug to the same anchor → collision. The composer must disambiguate titles before save (there's no library-level dedup).
- **Id-collision confirm**: `add_convention*` overwrites by id silently. Check `conventions_in(dir)` first; if the id exists (or shadows an inherited one), the composer prompts "overwrite?" before POSTing.
- **Target dir**: profile → `kn/profiles/<id>/conventions`; per-project overlay → `effective::overlay_dir(repo)` (reuse, don't reimplement). v1 targets the **profile**; overlay target optional (mirror existing `AddOverlayConvention`).

### B3. Save a composed **recipe** + refs — extend writer, fix bug

Recipes are whole-body markdown (no sections). Extend the write path to carry references:
- **`RecipeSpec`** (`knowledge.rs:314`): add `#[serde(default)] convention_refs: Vec<ConvRef>` + `#[serde(default)] related_recipes: Vec<String>`.
- **`write_recipe_files`** (`knowledge.rs:536`): accept the two, **include them in the upserted `_index.json` entry** (alongside id/title/description/file/tags). Thread through `add_recipe` (`:521`).
- **Bug fix (data loss):** because `upsert_index` replaces the whole entry and the old writer omitted refs, re-saving a recipe that had refs (e.g. `android-mvvm` feature/refactor) **silently wiped** them. Writing refs into the entry closes this. Add a **regression test**.
- **Canonical shape**: flat `convention_refs: [{topic, section}]` (matches `ConvRef` serde) in `_index.json`. Nested `.md` front-matter stays decorative (not parsed) — cleanup deferred.
- **Ref validation**: before persisting, resolve picked convention ids (+ optional section) and related recipe ids against the merged set (reuse `doctor`/`profile validate` resolution logic) → warn/block on dangling (avoid persisting broken refs). At minimum, the picker only offers existing ids.
- `web.rs` `AddRecipe` handler (`:270-276`) needs no change beyond `RecipeSpec` deserializing the new optional fields (serde default handles older clients).

### B4. Clone

Prefill the canvas from an existing doc using the **existing raw routes**: `GET .../convention/{cid}/raw` (`ConventionRaw` → `convention_md`) and `GET .../recipe/{rid}/raw` (`RecipeRaw` → `recipe_md`). Strip front-matter client-side (mirror `strip_frontmatter`) before filling. For recipe clone, also prefill `convention_refs`/`related_recipes` from the profile-detail `RecipeMeta` already in hand.

### B5. Frontend (`src/web/app.js` + `index.html`)

- Add `create: renderCreate` to the `VIEWS` map (`app.js:272`); optionally a sidebar `data-view="create"` nav-item (`index.html:22-29`) — but the primary entry is the profile-detail "Create new" button (`renderProfileDetail`, `app.js:1008`).
- `renderCreate(profileId, {kind, presetId})` — two-pane composer view (pattern after `renderProfiles`/`renderKnowledge`):
  - **Left palette**: `api('/api/profile/{id}/palette')`; grouped by origin (this profile / base extends / other profiles), search filter, checkable rows (`conv#section`, title, token est), "Clone whole convention" select.
  - **Right canvas**: kind toggle (convention/recipe) + id/title/tags; convention mode = assembled section cards (origin badge, remove, edit, ↑/↓ reorder) + "add blank section"; recipe mode = body textarea + convention_refs & related_recipes chip pickers.
  - **Save** via `api()` → existing `/convention` (ConventionSpec) or extended `/recipe` (RecipeSpec) → `toast()` → `renderProfileDetail(id)` (full re-render, matching every other save handler).
- Reuse `h()`/`esc()`/`toast()`/`splitCsv()`/`placePanel()`. Escape all interpolated strings via `esc()`.

---

## Component boundaries (isolation)

| Unit | File(s) | Responsibility | Depends on |
|------|---------|----------------|------------|
| Theme tokens | `style.css` `:root`/dark | Direction A palette, radii, shadow, fonts | — |
| Font serving | `web.rs` consts + `Route::Font` + tests, `src/web/*.woff2` | Embed & serve professional woff2 | — |
| Palette resolver | `inherit.rs` (or new `palette.rs`) | Flatten pickable sections + bodies + origin across profile+chain+others | `merged_conventions_provenance`, `resolve_convention_raw`, `parse_sections`, `profile::list`, `conventions_in` |
| Recipe writer ext | `knowledge.rs` (`RecipeSpec`, `write_recipe_files`, `add_recipe`) | Persist convention_refs/related_recipes; fix data-loss | `upsert_index`, `ConvRef` |
| Web routes | `web.rs` (`Route::Palette` + arm; reuse AddConvention/AddRecipe) | Palette GET; compose commit via existing writers | resolver, writers |
| Composer view | `app.js` `renderCreate` + `index.html` nav | Two-pane pick/assemble UI; save via `api()` | palette route, convention/recipe routes, raw routes |

Each unit is understandable/testable independently: the palette resolver is pure-ish (fs read → Vec), the recipe writer extension is unit-testable (round-trip + regression), the reskin is CSS/token + a localized Rust font swap, the composer view is a self-contained render function.

## Data flow

1. Open composer → `GET /api/profile/{id}/palette` → grouped pickable sections (profile+chain eager; other profiles lazy).
2. User checks sections / clones / adds blank → canvas state (client) accumulates copied `{title, body, origin, from}`.
3. Save convention → `POST /api/profile/{id}/convention` with `ConventionSpec` → `add_convention*` writes `<id>.md` + upserts `_index.json`.
4. Save recipe → `POST /api/profile/{id}/recipe` with extended `RecipeSpec` → `write_recipe_files` writes `.md` + `_index.json` **incl. refs**.
5. Re-render profile detail (reads via chain-aware `merged_conventions_provenance`/`recipes`).

## Error handling

- Reads via `read()` → 500 + `{error}`; writes via `write_op()` → 400 + `{error}`; both surfaced by `api()`'s throw → `toast(msg, isErr)`.
- Empty canvas / empty id / duplicate section titles / invalid id → client-side block with a clear message before POST.
- Id collision (existing/inherited) → confirm-overwrite prompt.
- Dangling refs → validated against merged set; block or warn.
- Palette body-join miss (md⇔index drift) → skip that section with a logged warning, never crash.

## Testing

- **Rust unit**: `palette_sections` joins metadata+body+origin correctly across own/inherited/overridden/other-profile; anchor-collision/dedup behavior; `valid_doc_id` guard; recipe refs **round-trip** through `recipes()`; **regression**: re-saving a recipe preserves refs (the bug); target-dir resolution (profile vs overlay).
- **Route**: add `route_parses_paths()` asserts for `Route::Palette` (+ any new); update the two font-path asserts to new filenames.
- **Build/format discipline**: `cargo build` (0 warnings) + `cargo test`; **do NOT run `cargo fmt`** (repo hand-formats wide-style — documented incident). Match the compact style.
- **Frontend**: `node --check src/web/app.js`; curl e2e against an isolated `~/.palugada` (palette JSON shape; compose convention from picked sections → `convention add`-shaped file; compose recipe with refs → refs in `_index.json`; clone prefill; override locked-id). Clean up scratch artifacts after.
- **Visual smoke**: light + dark across Overview/Projects/Profiles/Knowledge/Connectors + the new composer; grep `rotate(`, `--pow`, `Bangers`, `halftone` to confirm devices removed; confirm semantic hues still distinguishable.

## Risks

- **md⇔index id drift** breaks the palette body-join → skip-with-warning + lean on `profile validate` parity check.
- **Perf**: resolving every other profile's bodies is O(profiles×chain) fs reads, no cache → lazy-load other profiles.
- **Font license**: confirm the replacement is OFL/embeddable via `include_bytes!` (IBM Plex = OK); add attribution if required.
- **Dark shadow hardcoded hex** must change in lockstep with the ink/shadow language.
- **Semantic hue distinction** must survive desaturation (accessibility) — verify the four families remain visually distinct.
- **Recipe refs schema fragmentation**: `.md` nested front-matter vs flat `_index.json` — canonical = `_index.json`; `.md` block stays decorative (documented, cleanup deferred) to avoid perpetuating a *silent* mismatch, not a new one.

## File change map (grounded)

- `src/web/style.css`: tokens (`:11-76`), shadows (`:44-46`,`:75-76`), halftone (`:91-93`), rotates (`:109/284/383/464`), pow consumers, `@font-face` (`:1-3`), semantic blocks (`:318-321`,`:348-350`).
- `src/web/index.html`: brand mark (`:13`), optional `create` nav-item (`:22-29`).
- `src/web.rs`: font consts (`:15-17`), `Route::Font` match (`:207-211`), tests (`:781-782`); new `Route::Palette` variant (`:21-70`) + `route()` arm (`:73-162`) + `api()` arm (`:233-493`) + test; palette handler reusing `knowledge_dir()`/resolver.
- `src/inherit.rs` (or new `src/palette.rs`): `palette_sections`.
- `src/knowledge.rs`: `RecipeSpec` (`:314`), `write_recipe_files` (`:536`), `add_recipe` (`:521`) + tests.
- `src/web/app.js`: `VIEWS` (`:272`), `renderCreate` (new), profile-detail entry (`:1008`), absorb scattered add buttons.
- `src/web/*.woff2`: add professional woff2, remove comic woff2.

## Open items for the plan (writing-plans)

- Split into **Plan A (reskin)** + **Plan B (composer)** or one plan? (Prior features split like this — likely two plans, reskin first as it's lower-risk.)
- Exact palette endpoint shape (eager vs lazy `scope` param).
- Whether `Import markdown` folds into the composer or stays separate.
- Whether v1 composer targets profile only or also the per-project overlay.
