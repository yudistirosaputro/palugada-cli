# CLI markdown import for conventions & recipes (`convention add` / `recipe add`)

**Date:** 2026-06-20
**Status:** approved (design)
**Branch:** `feat/markdown-import-authoring`

## Problem

Authoring a convention today means hand-writing the *stored* format: YAML
front-matter with a `sections:` list (each carrying `id`, `tokens`, `code`), a
matching `## Heading {#slug}` body, **and** a parallel `_index.json` entry that
must stay in sync. That is fiddly and error-prone — nothing like writing an
ordinary Claude `SKILL.md` (simple `name`/`description` front-matter + free
markdown).

There is already an auto-mapping path — the `palugada web` "Add convention" form
calls `knowledge::add_convention`, which slugs section ids, estimates tokens, and
upserts `_index.json`. But there is **no CLI** to author a convention, and even
the web form requires structured per-section input rather than a plain markdown
document. Users who live in the terminal and think in markdown files have no
ergonomic path.

## Goal

Add CLI commands that import a **plain markdown file** (SKILL.md-style: a small
front-matter + `# Title` + `## Section` body) and auto-derive everything the
stored format needs — sections, token estimates, slug ids, and the `_index.json`
entry — writing the canonical convention/recipe on disk. The author writes a
normal document; palugada does the mapping.

Non-goals (YAGNI): interactive metadata wizard; recipe *overlays* (cycle C scoped
overlays to conventions only — recipes are profile-level); a web "paste markdown"
mode (the parser is reusable, so this can follow later); editing/rewriting an
existing doc's prose in place beyond whole-file replace.

## Approach

A single markdown→spec parser feeds the existing authoring core. The author's
**body is stored verbatim** (preserving their prose, blockquotes, ordering); only
the front-matter and `_index.json` are generated. This reuses the existing
`sections()` heading splitter (already skips code fences), `slug()`, the token
formula (`body.len()/4 + 8`), and `upsert_index`.

### Input contract (what the user writes)

```markdown
---
title: Error Handling
description: Result<T,String> with context, ? propagation, no panics.
tags: [rs, rust, error]
---

# Error Handling
> One-line summary.

## Result Type
body markdown...

## Adding Context
body... (may contain ```code``` fences)
```

Front-matter fields (all scalar/list, parsed as a small YAML struct):
- `title` (string) — required; **fallback**: the first `# H1` in the body if the
  field is absent. If neither exists → error.
- `description` (string) — optional, default empty.
- `tags` (list of strings) — optional, default empty.
- `id` (string) — optional; default `slug(title)`.

Recipes use the same front-matter; their body has no `## sections` semantics
(recipes are a single body blob).

### Auto-mapping (what palugada derives)

- **id**: front-matter `id` if present, else `slug(title)` (e.g. "Error Handling"
  → `error-handling`). Validated against the existing `[a-z0-9_-]` rule.
- **sections** (conventions only): derived by scanning the verbatim body with the
  existing `sections()` splitter. For each `## Heading`:
  - `id` = `slug(heading)`
  - `tokens` = `body.len()/4 + 8` (existing formula)
  - `code` = `true` if that section's body contains a ```` ``` ```` fence, else `false`
- **`_index.json`**: upserted (insert-or-replace by id) via the existing
  `upsert_index`, so it always stays in sync.
- **Stored body**: the author's markdown after the front-matter, written
  **verbatim** (no `{#slug}` anchor injection — `sections()`/drill-in key off the
  `## ` line, not anchors).

### Commands

- `palugada convention add <file.md> [--profile <id>] [--project <name>]`
  - default target: the resolved profile's `conventions/` dir
  - `--project <name>`: write to that registered project's **overlay**
    (`<repo>/.palugada/conventions/`, via the existing `add_convention_in` /
    `effective::overlay_dir`)
  - `--profile <id>`: profile override (as other commands)
- `palugada recipe add <file.md> [--profile <id>]`
  - target: the resolved profile's `recipes/` dir only (no overlay — recipes are
    profile-scoped; `--project` is not accepted here)

Both **upsert by id**: if the id already exists, replace and report
`updated <id>`; otherwise `created <id>`. The path written is printed.

## Components

### 1. `src/knowledge.rs` — markdown→spec parser + verbatim-body writers

- `parse_doc_front_matter(raw: &str) -> Result<DocMeta, String>` — parse the
  leading `---`…`---` block as a small YAML struct
  `DocMeta { id: Option<String>, title: Option<String>, description: String, tags: Vec<String> }`.
  No front-matter → all-default `DocMeta` (title falls back to H1 downstream).
- `add_convention_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String>`
  — returns `(id, replaced)`. Steps: parse front-matter; resolve title (field →
  else first `# H1` → else Err); resolve id (field → else `slug(title)`),
  validate; `body = strip_frontmatter(raw)`; derive `sections` from
  `sections(body)` with tokens + `code` (fence scan); detect `replaced` =
  the `<id>.md` already exists; write canonical front-matter + **verbatim body**;
  `upsert_index(conventions/_index.json)`.
- `add_recipe_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String>`
  — same front-matter handling; no sections; verbatim body; upsert
  `recipes/_index.json`.
- Refactor the front-matter *writing* shared by `add_convention` and the importer
  into a small helper (`write_convention_doc(dir, &ConventionSpec, body)` taking a
  prebuilt body), so the verbatim-body importer and the section-generating
  `add_convention` share one writer and the existing `add_convention` behavior is
  unchanged.

### 2. `src/main.rs` — CLI surface

- New subcommands. Either a `convention` / `recipe` subcommand group (preferred,
  with `Add { file, profile, project }`) or top-level `convention-add`. Each
  handler:
  - reads the file (error if missing or not `.md`),
  - resolves the target dir: profile conventions/recipes dir, or overlay dir when
    `--project` is given (conventions only),
  - calls the matching `*_from_markdown`,
  - prints `created <id> → <path>` or `updated <id> → <path>`.

### 3. Reused as-is

`sections()`, `slug()`, `strip_frontmatter()`, `validate_doc_id()`,
`upsert_index()`, `add_convention_in`/`effective::overlay_dir` (overlay target),
profile/project resolution helpers.

## Error handling

- File missing / not readable / extension not `.md` → clear error.
- No `title` field and no `# H1` → error ("convention needs a title: add a
  `title:` field or a `# Heading`").
- Invalid id (`[a-z0-9_-]`) → existing `validate_doc_id` error.
- `--project` on an unregistered project → existing "project not registered"
  error. `--project` passed to `recipe add` → error ("recipes are profile-scoped;
  use --profile").
- A convention with zero `##` sections → allowed, but warn ("no `##` sections
  found — added with an empty outline").

## Testing

- **Pure parse (no I/O)**: `parse_doc_front_matter` reads title/description/tags/
  id; missing front-matter → defaults; section derivation finds `##` headings,
  estimates tokens, sets `code=true` only when a fence is present and `false`
  otherwise; H1 fallback for title when the field is absent.
- **Roundtrip (`tempfile`)**: `add_convention_from_markdown` into a temp dir →
  `conventions_in` lists it with the derived sections, `convention_md_in` shows
  the verbatim body, `_index.json` carries the entry. Re-import same id →
  `replaced == true` and content updated.
- **Recipe**: `add_recipe_from_markdown` writes `<id>.md` + upserts recipe index;
  re-import updates.
- **CLI smoke**: `convention add fixture.md --profile rust-cli` then
  `palugada q <id> --profile rust-cli` shows the body; `q --list` shows it.
- **Overlay**: `convention add fixture.md --project <name>` writes to the repo's
  `.palugada/conventions/` (verify via `effective_rules` origin = project).
- **CI parity**: `cargo build --release` + `cargo test --release` green, no new
  warnings.

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `src/knowledge.rs` | Modify | `parse_doc_front_matter`, `add_convention_from_markdown`, `add_recipe_from_markdown`, shared writer refactor (+ tests) |
| `src/main.rs` | Modify | `convention add` / `recipe add` subcommands + handlers |

## Risk / notes

- Verbatim-body storage means the stored `.md` no longer always matches
  `add_convention`'s generated layout (it now reflects the author's prose). That's
  intended and strictly more flexible; readers key off `## ` lines, not anchors.
- The parser is deliberately small (front-matter + `##` scan) — it does not aim to
  fully model markdown. Token counts remain estimates (same as today).
- The same parser can later back a web "paste markdown" mode and a `recipe`
  overlay if cycle C ever extends overlays to recipes — out of scope now.
- No version bump / npm release as part of this work.
