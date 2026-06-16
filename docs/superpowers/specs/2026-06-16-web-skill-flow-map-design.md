# Per-project Skill Flow map in `palugada web` (phase 1)

**Date:** 2026-06-16
**Status:** approved (design)
**Branch:** `feat/web-skill-flow-map`

## Problem

The `palugada web` console can author profiles/knowledge and switch a project's
bound profile, but it can't show **what a generated agent skill actually does**
for a given project. A user looking at `palugada-bugfix` sees only the generic
SKILL.md ("run `palugada brief bugfix`, then `palugada q <topic>`") — not the
*exact* steps and the *concrete* conventions/recipes that `brief bugfix` will
pull, nor which tool-skills are active for that project.

That information already exists: `profile.yaml` carries a `flows:` map
(`bugfix: [code.recent, symbol.find, convention(errorhandling), convention(testing)]`)
and a `review_map`, and the per-project tool-skill set is gated by which
integrations are configured (`scaffold::integration_kinds`). It just isn't
resolved or surfaced.

## Goal

Add a **per-project detail page** to the web console that, for a selected
registered project, resolves and visualizes every generated skill and the exact
things it routes to, and lets the user **edit** the underlying conventions/
recipes in place (which live centrally in `~/.palugada`, so no project repo is
touched).

Non-goals (later cycles): editing credentials/integrations in the web (cycle A);
true per-project convention *overlays* (cycle C); editing convention/recipe
*metadata* (title/desc/tags) — phase 1 edits body content only.

## Approach

**Resolve on the backend.** A new endpoint `GET /api/project/{name}/skillmap`
loads the project config + the bound profile's `profile.yaml`, parses
`flows`/`review_map`, classifies each step, checks which conventions/recipes
exist, and computes tool-skill gating — returning a structured JSON the frontend
renders directly. Keeps token-parsing and gating logic in Rust (unit-testable,
single source of truth) instead of duplicating it in untested JS.

## Components

### 1. `src/skillmap.rs` (new) — resolver + step parser

Pure-as-possible functions, unit-tested:

- `classify_step(step: &str, conv_ids: &BTreeSet<String>, recipe_ids: &BTreeSet<String>, review_map) -> Step`
  - `convention(X)` → `Step::Convention { id: X, exists: conv_ids.contains(X) }`
  - `recipe(X)` → `Step::Recipe { id: X, exists: recipe_ids.contains(X) }`
  - `convention(by-file-kind)` / `by-file-kind` → `Step::ReviewMap { expand: Vec<{family, conventions}> }` from `review_map`
  - engine tokens (`code.recent`, `symbol.find`, `prd.context`, `module.info`,
    `diff.scan`) → `Step::Engine { token, label }` with a human label from a
    static table; unknown token → `Step::Engine { token, label: token }`.
- `tool_skills(kinds: &[&str]) -> Vec<ToolSkill>` — static table mirroring
  `scaffold`'s gating; each entry `{ name, requires: &[kind], enabled }` where
  `enabled = requires.any(|k| kinds.contains(k))`:
  - `palugada-git` ← `[git_host]`
  - `palugada-docs` ← `[issue_tracker, wiki]`
  - `palugada-ci` ← `[ci, chat]`
  - `palugada-design` ← `[design]`
- `skillmap(global, name) -> Result<SkillMap, String>` — orchestrates: resolve
  project → `ProjectConfig` (profile + integrations) → load profile `flows`/
  `review_map` + `knowledge::{conventions,recipes}` ids + `custom_skill ids`
  (from `profiles/<id>/skills/`), build the full skill list.

A test asserts the four tool-skill gating rules stay in sync with
`scaffold::skill_files` (same kinds → same enabled tool skills).

### 2. JSON shape (`GET /api/project/{name}/skillmap`)

```json
{
  "project": "my-app",
  "profile": "android-mvvm",
  "skills": [
    { "name": "palugada-search", "kind": "search",
      "command": "palugada symbol <q> / palugada fact <family>" },
    { "name": "palugada-bugfix", "kind": "flow",
      "command": "palugada brief bugfix <target>", "flow": "bugfix",
      "steps": [
        { "kind": "engine", "token": "code.recent", "label": "recent changes" },
        { "kind": "engine", "token": "symbol.find", "label": "relevant symbols" },
        { "kind": "convention", "id": "errorhandling", "exists": true },
        { "kind": "convention", "id": "testing", "exists": true }
      ] },
    { "name": "palugada-review", "kind": "flow",
      "command": "palugada brief review <ref>", "flow": "review",
      "steps": [
        { "kind": "engine", "token": "diff.scan", "label": "scan the diff" },
        { "kind": "review_map", "expand": [
          { "family": "viewmodel", "conventions": ["architecture", "testing"] },
          { "family": "repository", "conventions": ["architecture", "errorhandling", "testing"] }
        ] }
      ] },
    { "name": "palugada-git", "kind": "tool", "enabled": false, "needs": ["git_host"] },
    { "name": "palugada-docs", "kind": "tool", "enabled": true, "needs": ["issue_tracker", "wiki"] },
    { "name": "my-custom-skill", "kind": "custom" }
  ],
  "warnings": [
    "flow 'feature' references recipe(feature) which does not exist in profile 'android-mvvm'"
  ]
}
```

Flow skills are rendered **generically** — one per key in `profile.yaml`
`flows:` (not hardcoded to the four). If a `flows` key has no matching generated
skill, or a generated flow-skill has no `flows` entry, it still renders with a
`warnings[]` note. `exists: false` on any `convention`/`recipe` step also adds a
warning.

### 3. `src/web.rs` — routes

- `GET /api/project/{name}/skillmap` → `Route::SkillMap(name)` → read handler
  calling `skillmap::skillmap(...)` (name is URL-decoded like
  `set_project_profile`).
- `POST /api/profile/{id}/convention/{cid}/body` → `Route::SetConventionBody(id, cid)`
  → `knowledge::set_convention_body(kn, id, cid, markdown)`. Body: `{ "markdown": "..." }`.
- `POST /api/profile/{id}/recipe/{rid}/body` → `Route::SetRecipeBody(id, rid)`
  → `knowledge::set_recipe_body(kn, id, rid, markdown)`. Body: `{ "markdown": "..." }`.

Both edit the raw `.md` **verbatim** (the editor pre-fills from the existing
`convention_md`/`recipe_md` GET). Metadata in `_index.json` is intentionally left
untouched, so `q --list` titles/descriptions stay stable across body edits.

### 4. `src/knowledge.rs` — `set_convention_body` / `set_recipe_body`

```
pub fn set_convention_body(kn, profile, id, markdown) -> Result<(), String>
pub fn set_recipe_body(kn, profile, id, markdown) -> Result<(), String>
```
Each overwrites `conventions/<id>.md` (resp. `recipes/<id>.md`) with `markdown`
verbatim. Edit-only: errors if the file doesn't already exist (creation stays
with the section/body-based `add_convention`/`add_recipe`). Round-trip
unit-tested.

### 5. `src/web/app.js` — per-project detail view

- In `renderProjects()`, make each project name a link → `renderProjectDetail(name)`.
- `renderProjectDetail(name)`:
  - Header: project name, repo path, profile dropdown (reuse the existing
    set-profile call), `← projects` back link.
  - **SKILL FLOW** section: fetch `/api/project/<name>/skillmap`; render each
    skill as a card:
    - flow skills: command + an ordered step list. Each step shows a badge by
      `kind`: `engine` (muted label), `convention`/`recipe` (`id` + `[view]`
      `[edit]` actions; `⚠ missing` if `exists:false`), `review_map` (expand the
      `family → conventions` rows, each convention also `[view]`/`[edit]`).
    - search skill: just the command.
    - tool skills: `enabled` → normal; else `⚠ needs <kinds>` (muted).
    - custom skills: name only in phase 1 (no body view endpoint yet).
  - `warnings[]` rendered at top of the section.
- `[view]` reuses `showBody` against the existing convention/recipe body endpoint.
- `[edit]`: editor with one `<textarea>` pre-filled from the body GET endpoint;
  Save → `POST /api/profile/<id>/convention/<cid>/body` (or `.../recipe/<rid>/body`)
  `{ markdown }` → re-render. (Verbatim `.md`, including its front-matter.)
  - A muted note: "edits the profile's knowledge in ~/.palugada (shared by all
    projects on this profile); the project repo is not touched."

### 6. Error handling

- Unknown/unregistered project → 500 with a clear message ("project '<n>' is not
  registered").
- Bound profile missing / `profile.yaml` unreadable or malformed `flows` → error
  surfaced via the toast; the page shows what it could load.
- Edit: empty `markdown`/`id` rejected (400). Editing a convention id that
  doesn't exist → error.
- This feature reads integrations only (for gating) and never reads/writes
  secrets; integration editing is cycle A.

### 7. Testing

- **Rust unit tests** (`src/skillmap.rs`):
  - `classify_step` for each kind: `convention(x)` exists/missing,
    `recipe(x)` exists/missing, `by-file-kind` expands via review_map, engine
    tokens get labels, unknown token falls through.
  - `skillmap` over a fixture profile (flows + review_map + a couple of
    conventions/recipes) + a fixture project config with one integration set →
    asserts the resolved skills, the `enabled`/`needs` tool gating, and a
    `warnings` entry for a missing `recipe(...)`.
  - tool-skill gating matches `scaffold` for the same `kinds`.
- **Route test** (`src/web.rs`): `route("GET", "/api/project/x/skillmap")` and
  `route("POST", "/api/profile/p/convention/c/body")` parse correctly.
- **knowledge**: `set_convention_body` / `set_recipe_body` round-trip (write →
  body GET reflects it; `_index.json` metadata preserved; unknown id errors).
- **Manual e2e**: `palugada web` → Projects → open a project → verify the map
  renders bugfix/feature/refactor/review with concrete conventions, review
  expands via review_map, a disabled tool skill shows `needs git_host`; edit a
  convention body and confirm it persists.

## File structure

| File | Action | Responsibility |
|---|---|---|
| `src/skillmap.rs` | Create | step classification + skillmap resolver (+ tests) |
| `src/main.rs` | Modify | `mod skillmap;` |
| `src/web.rs` | Modify | 2 new routes + handlers |
| `src/knowledge.rs` | Modify | `set_convention_body` (+ test) |
| `src/web/app.js` | Modify | per-project detail view, skill-flow render, view/edit |
| `src/web/style.css` | Modify | minor step/badge styling |

## Risk / notes

- The flow→steps truth lives in `profile.yaml`; the generated SKILL.md is a
  generic wrapper. The map deliberately reflects the engine's actual behavior
  (flows/review_map), not the skill prose — that's the "exact" answer.
- Editing a convention affects every project bound to that profile. Phase 1
  surfaces this with a UI note; per-project isolation is the deferred overlay
  (cycle C).
