# Per-project convention overlay + effective-rules merge (web cycle C)

**Date:** 2026-06-19
**Status:** approved (design)
**Branch:** `feat/convention-overlay`

## Problem

Conventions live in the bound profile (`knowledge/profiles/<id>/conventions/`)
and are shared by **every** project on that profile. A single project that needs
a tweak — an extra rule, a corrected `architecture` note, a different
file-kind→convention mapping — has no way to do it without forking the whole
profile (losing shared updates) or editing the shared profile (polluting it for
every other project).

The skill-flow map (cycle B) and credentials editor (cycle A) already resolve and
edit per-project state on the project detail page, but conventions are still
profile-global. Cycle C closes that gap.

## Goal

Let a project **add, override, and remap** conventions for itself only, stored in
its own repo (`.palugada/`, committed → shared with the team via git). The CLI
`brief` flows must resolve against the **merged "effective rules"** (profile +
overlay), not the raw profile — otherwise the overlay is cosmetic. Surface and
edit the overlay on the existing web project-detail page, and add a read-only CLI
inspector.

Non-goals (YAGNI / later): overriding *recipes*; overlay affecting `fact`/symbol
index; fully *hiding* a profile convention (overlay only adds / overrides body /
remaps — a disable flag can come later); editing overlay convention *metadata*
beyond what `add_convention` already writes.

## Approach

Mirror the profile's own on-disk layout inside the repo so the existing
convention reader/writer code is reused, and keep the merge logic in one pure,
unit-tested Rust module (`effective.rs`) consumed by three surfaces: `brief`
(behavior), the web detail page (edit + view), and a CLI inspector (verify).

### Storage (in-repo, committed, shared via git)

- `<repo>/.palugada/conventions/<id>.md` + `<repo>/.palugada/conventions/_index.json`
  — overlay conventions in the **exact same format** as profile conventions
  (frontmatter `id`/`title`/`description`/`tags`/`sections` + markdown body).
- `<repo>/.palugada/config.yaml` gains an optional `review_map:` block:

  ```yaml
  review_map:
    viewmodel: [architecture, testing, our-extra-rule]
  ```

  Tokens are still never stored here (secrets stay in `~/.palugada/secrets.yaml`).

### Merge semantics ("effective rules")

- **Conventions, by id:**
  - overlay id matches a profile id → `Overridden` (overlay body wins).
  - overlay-only id → `Project`.
  - profile-only id → `Profile`.
- **review_map, per family (replace-by-key):** a family present in the overlay
  override **replaces** that family's profile entry entirely; families absent from
  the override keep the profile's entry. Predictable and easy to reason about
  (no append/union ambiguity).

## Components

### 1. `src/knowledge.rs` — dir-parameterized convention accessors

The existing convention read/write functions derive the conventions dir from
`kn + profile`. Extract the dir-taking core so both profile and overlay reuse it:

```
pub fn conventions_in(dir: &Path) -> Result<Vec<TopicMeta>, String>
pub fn convention_md_in(dir: &Path, id: &str) -> Result<String, String>
pub fn add_convention_in(dir: &Path, spec: &ConventionSpec) -> Result<(), String>
pub fn set_convention_body_in(dir: &Path, id: &str, markdown: &str) -> Result<(), String>
```

The current profile-based wrappers (`conventions`, `convention_md`,
`add_convention`, `set_convention_body`) become thin shims computing
`profiles/<id>/conventions` and delegating to the `_in` variants. Behavior for
profile callers is unchanged (existing tests stay green). An overlay dir that
does not exist yet → `conventions_in` returns an empty vec (not an error), so a
project with no overlay resolves cleanly.

### 2. `src/effective.rs` (new) — pure merge + resolver

```
pub enum Origin { Profile, Project, Overridden }

pub struct EffectiveConvention {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub origin: Origin,
}

pub struct EffectiveReviewEntry {
    pub family: String,
    pub conventions: Vec<String>,
    pub origin: Origin,           // Project if overridden, else Profile
}

pub struct EffectiveRules {
    pub project: String,
    pub profile: String,
    pub conventions: Vec<EffectiveConvention>,
    pub review_map: Vec<EffectiveReviewEntry>,
    pub warnings: Vec<String>,
}
```

Pure functions (unit-tested without I/O), mirroring `credentials.rs`'s testable
transforms:

- `merge_conventions(profile: &[TopicMeta], overlay: &[TopicMeta]) -> Vec<EffectiveConvention>`
- `merge_review_map(profile: &BTreeMap<String,Vec<String>>, overlay: &BTreeMap<String,Vec<String>>) -> Vec<EffectiveReviewEntry>`

I/O resolver, mirroring `skillmap::skillmap`:

- `effective_rules(global: &GlobalConfig, name: &str) -> Result<EffectiveRules, String>`
  - resolve project → `ProjectConfig` (profile + repo_path).
  - profile conventions via `knowledge::conventions(kn, profile)`; profile
    `review_map` via the profile loader (reuse `brief`/`skillmap`'s `profile.yaml`
    read).
  - overlay conventions via `knowledge::conventions_in(<repo>/.palugada/conventions)`;
    overlay `review_map` from `ProjectConfig.review_map`.
  - `warnings`: an overlay `review_map` family that references a convention id
    present in neither profile nor overlay; an overlay convention whose id
    duplicates a profile id (informational: "overrides profile convention").

### 3. `src/config.rs` — overlay field on `ProjectConfig`

Add `review_map` to `ProjectConfig`, defaulted + skipped when empty (same pattern
as the existing `exec` field, so old configs parse unchanged):

```
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub review_map: BTreeMap<String, Vec<String>>,
```

A helper `set_review_map(repo_path, map)` (load → set → save) for the web write
path, alongside the existing `set_profile`.

### 4. `src/brief.rs` — resolve against effective rules

`brief` already loads `ProjectConfig` (it has `repo` + `profile`). Thread the
overlay in at the two convention resolution points:

- the effective `review_map` = `merge_review_map(profile_map, pc.review_map)`
  flattened back to a `BTreeMap` for `mapped_topics`.
- `convention(by-file-kind)` and direct `convention(<id>)` steps render the
  **overlay** body when that id is overridden/added by the project, else the
  profile body. Concretely: build an overlay conventions dir path from the repo;
  when rendering a convention outline/body, prefer `convention_*_in(overlay_dir)`
  if the id exists there, else fall through to the profile.

A small helper `effective::convention_source(global, project) -> ConventionSource`
returning the two dirs (profile dir, optional overlay dir) keeps `brief` from
re-implementing path logic.

### 5. `src/web.rs` — routes (mirror cycle A/B)

- `GET /api/project/{name}/rules` → `Route::ProjectRules(name)` → read handler →
  `effective::effective_rules(global, name)` as JSON.
- `POST /api/project/{name}/convention` → `Route::AddOverlayConvention(name)` →
  write handler → `knowledge::add_convention_in(overlay_dir, spec)` (body is a
  `ConventionSpec`, same shape the profile editor already posts).
- `POST /api/project/{name}/convention/{id}/body` →
  `Route::SetOverlayConventionBody(name, id)` →
  `knowledge::set_convention_body_in(overlay_dir, id, markdown)`. Body `{ markdown }`.
- `POST /api/project/{name}/review-map` → `Route::SetOverlayReviewMap(name)` →
  `config::set_review_map(repo, map)`. Body `{ review_map: { family: [ids] } }`.

All write handlers resolve `name` → `repo_path` (URL-decoded via
`http::decode_segment`, as in cycle A) and compute
`<repo>/.palugada/conventions`. Read is masked-free here (no secrets touched).

### 6. JSON shape (`GET /api/project/{name}/rules`)

```json
{
  "project": "status-saver",
  "profile": "android-mvvm",
  "conventions": [
    { "id": "architecture", "title": "Architecture", "tags": ["mvvm"], "origin": "overridden" },
    { "id": "errorhandling", "title": "Error Handling", "tags": ["kt"], "origin": "profile" },
    { "id": "our-extra-rule", "title": "Our Extra Rule", "tags": ["kt"], "origin": "project" }
  ],
  "review_map": [
    { "family": "viewmodel", "conventions": ["architecture", "testing", "our-extra-rule"], "origin": "project" },
    { "family": "repository", "conventions": ["architecture", "errorhandling", "testing"], "origin": "profile" }
  ],
  "warnings": [
    "review_map family 'service' references convention 'nonexistent' not found in profile or overlay"
  ]
}
```

### 7. `src/main.rs` — CLI `project rules <name>`

Extend the existing `project` subcommand with `rules <name>`:

- prints the effective conventions, each tagged `[profile]` / `[project]` /
  `[overridden]`, then the effective `review_map` with the same origin tags, then
  warnings. Read-only. This is the e2e verification surface.

### 8. `src/web/app.js` + `style.css` — Effective Rules card

On the existing project detail page, add an **Effective Rules** card:

- one row per effective convention with an origin badge
  (`profile` / `project` / `overridden`); project + overridden rows get
  `[view]`/`[edit]` (against the overlay body endpoints); profile-only rows are
  read-only with `[view]` against the existing profile body endpoint.
- a **review_map** sub-section: each family → its effective convention ids, origin
  badge, edited via an inline editor that POSTs the override block.
- an **"Add project rule"** form (reuses the profile `add convention` form shape)
  posting to `/api/project/<name>/convention`.
- warnings rendered at the top of the card.
- a muted note: "Edits here touch THIS project's overlay in its repo
  (`.palugada/`), committed with the project — not the shared profile."

### 9. Error handling

- Unknown/unregistered project → error with a clear message (as cycle A/B).
- Overlay dir absent → treated as empty overlay (no error); first
  `add convention` / `set body` creates the dir + `_index.json`.
- `set_convention_body_in` on an id not present in the overlay → error (edit-only;
  use add to create), matching the profile editor's semantics.
- Empty `id`/`markdown` rejected.
- review_map override referencing an unknown convention → accepted but surfaced as
  a `warnings[]` entry (non-fatal; the user may add the convention next).

### 10. Testing

- **`effective.rs` unit (pure, no I/O):** `merge_conventions` classifies
  profile-only / project-only / overridden correctly; `merge_review_map` replaces
  per-family and leaves untouched families as `Profile`; warning generated for a
  dangling convention reference.
- **`knowledge.rs` (`tempfile`):** `conventions_in` on a missing dir → empty;
  `add_convention_in` writes `<id>.md` + `_index.json`; `set_convention_body_in`
  overwrites verbatim and errors on unknown id; profile wrappers still pass
  (delegation unchanged).
- **`brief` integration (`tempfile` repo + knowledge):** an overlay convention
  overrides the rendered body for `convention(<id>)`; an overlay `review_map`
  override changes which conventions `convention(by-file-kind)` pulls.
- **`config.rs`:** `ProjectConfig` round-trips with and without `review_map`;
  `set_review_map` load→set→save.
- **`web.rs` route test:** the four new routes parse (with URL-encoded names).
- **CLI smoke:** `project rules <name>` prints origins.
- **Manual e2e:** against `status-saver` (profile android-mvvm) — add a project
  rule, override `architecture` body, remap a family; confirm `palugada project
  rules status-saver` and `palugada brief review` reflect the overlay, and the web
  card shows the right origin badges.

## File structure

| File | Action | Responsibility |
|---|---|---|
| `src/knowledge.rs` | Modify | dir-parameterized `*_in` accessors + shims (+ tests) |
| `src/effective.rs` | Create | merge transforms + `effective_rules` resolver (+ tests) |
| `src/config.rs` | Modify | `review_map` on `ProjectConfig` + `set_review_map` (+ test) |
| `src/brief.rs` | Modify | resolve conventions/review_map against the overlay (+ test) |
| `src/web.rs` | Modify | 4 new routes + handlers |
| `src/main.rs` | Modify | `mod effective;` + `project rules <name>` |
| `src/web/app.js` | Modify | Effective Rules card (view/edit/add/remap) |
| `src/web/style.css` | Modify | origin badge + row styling |

## Risk / notes

- Reusing the profile convention format in the repo means a future `palugada index`
  could, in principle, pick up overlay conventions; in this cycle only `brief` and
  the web/CLI inspectors read them — index is untouched.
- `review_map` override is replace-per-family by design; if union turns out to be
  wanted, it's an additive change to `merge_review_map` later.
- The overlay travels with the repo, so a clone on another machine gets the team's
  project-specific rules automatically — no `~/.palugada` sync needed.
