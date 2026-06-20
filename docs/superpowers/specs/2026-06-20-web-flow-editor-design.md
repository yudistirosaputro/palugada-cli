# Profile flow editor in `palugada web`

**Date:** 2026-06-20
**Status:** approved (design)
**Branch:** `feat/web-flow-editor`

## Problem

A profile's `flows:` (the step lists `palugada brief <flow>` assembles — e.g.
`bugfix: [code.recent, symbol.find, convention(errorhandling), convention(testing)]`)
can only be changed by hand-editing `profile.yaml`. The web console shows flows as
**read-only pills**. So after adding a convention to a profile (e.g.
`r8-analyzer`), there's no in-browser way to wire it into a flow so `brief` pulls
it; you must open the YAML file in a text editor. There is also no way to create a
new flow (e.g. `optimize`) from the web.

## Goal

Make the web profile-detail page's **Flows** section editable: per existing flow,
add/remove/reorder steps (engine tokens, `convention(<id>)`, `recipe(<id>)`,
`convention(by-file-kind)`); plus create and delete whole flows. Save writes only
the `flows:` block of `profile.yaml`, preserving the rest of the file (its
comments, `description`, `fact_families`, `review_map`). No skill regeneration is
needed — flows are read live by `brief`.

Non-goals (YAGNI, per the brainstorm): editing the profile's `review_map`;
renaming a flow (delete + re-create instead); deep validation of step contents
(the UI only offers valid steps via dropdowns).

## Approach

The web already receives `flows` (a `BTreeMap<String, Vec<String>>`) plus the
profile's `conventions`/`recipes` via `profile_json`, so the editor has
everything it needs to populate dropdowns. A new `profile::set_flows` does a
**surgical replacement of just the `flows:` block** in `profile.yaml` (find the
`flows:` line, replace its indented body, leave everything else byte-for-byte),
so authored comments and other sections survive. One route saves the whole flows
map atomically.

### Step vocabulary (what a step can be)

- **Engine tokens** (fixed set, from `skillmap::engine_label`): `code.recent`,
  `symbol.find`, `module.info`, `diff.scan`, `prd.context`.
- **`convention(<id>)`** — `<id>` from the profile's existing conventions.
- **`recipe(<id>)`** — `<id>` from the profile's existing recipes.
- **`convention(by-file-kind)`** — the review-map expansion step.

### Components

#### 1. `src/profile.rs` — `set_flows` (surgical YAML block write)

```rust
/// Replace the `flows:` block of a profile's `profile.yaml` with `flows`,
/// preserving every other line (comments, description, fact_families, review_map).
/// Flow names are validated as `[a-z0-9_-]`. Steps are written verbatim.
pub fn set_flows(kn: &Path, id: &str, flows: &BTreeMap<String, Vec<String>>) -> Result<(), String>
```

Algorithm (text-surgical, comment-safe):
- Read `profile.yaml`. Validate each flow name with the existing id rule
  (`[a-z0-9_-]`, non-empty).
- Render the new block:
  ```
  flows:
    <name>: [<step>, <step>, ...]
    ...
  ```
  (one line per flow, steps comma-joined; empty step list → `[]`). Flows emitted
  in the map's order (BTreeMap → alphabetical, stable).
- Locate the existing `flows:` line (a line equal to `flows:` after trimming
  trailing space, at column 0). Replace from that line through the contiguous
  following **indented** lines (`^[ \t]`), stopping at the first non-indented line
  (blank line or next top-level key/comment) — so the trailing blank line and the
  `# Maps each fact family…` comment + `review_map:` are untouched.
- If no `flows:` line exists, insert the block before the first top-level
  `review_map:` line, else append at end (with a leading blank line).
- Write the file back.

#### 2. `src/web.rs` — save route

- `POST /api/profile/{id}/flows` → `Route::SetFlows(id)`. Body:
  ```json
  { "flows": { "bugfix": ["code.recent", "convention(r8-analyzer)"], "optimize": ["convention(r8-analyzer)"] } }
  ```
  Handler: parse, `profile::set_flows(&kn, &id, &flows)`, return
  `{ "ok": true, "flows": <n> }`. (Reading is already covered by `profile_json`'s
  `flows`.)

#### 3. `src/web/app.js` — editable Flows section (replaces read-only pills)

In `renderProfileDetail(id)`, replace the Flows pills with a `flowsCard(id, d)`
(`d` = the profile JSON, carrying `flows`, `conventions`, `recipes`). It holds the
flows map in memory and renders:

- For each flow: the flow name + a **× delete flow** link, and an ordered row of
  step chips. Each chip shows the step text and has **×** (remove) and **↑ / ↓**
  (reorder) controls.
- A per-flow **+ add step** control: a `<select>` of step *kind*
  (engine / convention / recipe / by-file-kind); choosing engine/convention/recipe
  reveals a second `<select>` of the concrete value (engine token list, or the
  profile's convention/recipe ids); an **add** button appends the step.
- A global **+ add flow** input (name) → adds an empty flow to the map.
- A **Save flows** button → `POST /api/profile/<id>/flows` `{ flows }` → toast +
  `renderProfileDetail(id)`.
- A muted note: "Saved to the profile's `profile.yaml`; `brief` uses it live — no
  skill regeneration needed."

All edits mutate the in-memory map; nothing persists until **Save flows**.

#### 4. `src/web/style.css`

A `.step-chip` rule (inline-block chip with the remove/reorder controls) and minor
layout for the add-step row.

### Error handling

- Invalid flow name (not `[a-z0-9_-]`) → rejected on Save with a clear message;
  the UI also blocks adding a flow with a bad name.
- Empty flows map (all flows deleted) → allowed, but the UI confirms first
  ("delete all flows?"); `brief <flow>` then reports "flow not defined".
- `profile.yaml` unreadable/missing `flows:` → `set_flows` inserts the block (no
  error); other malformed YAML surfaces the read error.
- A flow with zero steps is allowed (written as `[]`); `brief` renders nothing for
  it.

## Testing

- **`profile::set_flows` unit (`tempfile`):**
  - round-trip: write a profile.yaml fixture (with a `description`,
    `fact_families`, `flows`, and a commented `review_map`), call `set_flows` with
    a new map, then re-read `flows` → matches; **assert the `review_map` block and
    the `# ...` comments are still present byte-for-byte**.
  - inserting flows into a profile.yaml that has none → block added, other content
    intact.
  - invalid flow name → `Err`.
- **web route test (`route_parses_paths`):** `POST /api/profile/p/flows` →
  `Route::SetFlows("p")`.
- **CI parity:** `cargo build --release` + `cargo test --release` green, no new
  warnings.
- **Manual e2e:** `palugada web` → Profiles → android-mvvm → Flows editor: add
  `convention(r8-analyzer)` to `bugfix`, reorder a step, create an `optimize` flow
  with `convention(r8-analyzer)`, Save. Then `palugada brief bugfix X --profile
  android-mvvm` shows the r8 convention, and `palugada brief optimize X` renders
  it; confirm `profile.yaml` kept its comments + `review_map`. Revert the demo
  edits afterward.

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `src/profile.rs` | Modify | `set_flows` surgical block writer (+ tests) |
| `src/web.rs` | Modify | `SetFlows` route + handler (+ route test) |
| `src/web/app.js` | Modify | `flowsCard` editor (replaces read-only Flows pills) |
| `src/web/style.css` | Modify | step-chip styling |

## Risk / notes

- The surgical writer is the delicate part; the round-trip test that asserts
  comments + `review_map` survive is the gate. If the regenerated block drops the
  existing column alignment (cosmetic), that's acceptable.
- BTreeMap ordering re-sorts flows alphabetically on save (bugfix/feature/optimize/
  refactor/review). Acceptable; flow order is not semantically meaningful.
- Reading flows already works (`profile_json`); only writing is new.
- No version bump / npm release as part of this work.
