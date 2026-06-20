# Upload & split a markdown file into conventions (preview-then-commit) in `palugada web`

**Date:** 2026-06-20
**Status:** approved (design)
**Branch:** `feat/web-md-upload-split`

## Problem

Authoring knowledge today means one convention at a time — either the web "Add
convention" form (field-by-field) or the CLI `convention add <file.md>` (one
file → one convention). A user with a single document covering a topic (e.g. a
"Firebase integration" doc with Setup/Auth/Firestore sections), or a longer doc
covering several topics, has no way to drop the whole file into a profile in the
browser and have palugada slot the pieces in. The CLI importer exists but is
single-convention and terminal-only.

## Goal

Add a **web** panel on the profile-detail page to **upload (or paste) a markdown
file**, have palugada **split it into candidate conventions**, **preview** them
(edit ids/titles/tags, include/exclude), and on confirm **import the selected
ones into the profile** — reusing the existing markdown importer so sections,
token estimates, `{#slug}` anchors, and `_index.json` are produced automatically.

Non-goals (YAGNI, per the brainstorm): importing pieces as *recipes* (conventions
only); importing into a per-project *overlay* (profile only); a CLI `--split`
flag (the splitter is reusable, can follow later); drag-and-drop (file-picker +
paste is enough).

## Approach

Two backend endpoints over one pure splitter, plus a web panel. The split rule is
**by level-1 heading (`# H1`)**: each `# H1` block becomes one candidate
convention; its `## ` sub-headings become that convention's sections. Preview is
a no-write parse; commit reuses `add_convention_from_markdown` per selected piece
(which upserts and produces the canonical stored format).

### Split rule (`# H1` → one candidate)

- Each `# H1` line starts a new piece; the piece spans until the next `# H1` (or
  EOF). Fence-aware: a `# ` inside a ``` code fence is body text, not a heading.
- Per piece: `title` = the `# H1` text; `id` = `slug(title)`; `sections` = the
  `## ` heading titles within the piece (via the existing `sections()`); `body` =
  the piece markdown **after** its `# H1` line (blockquote/prose/`##` sections),
  excluding the H1 line itself.
- A document with a single `# H1` → one candidate (the Firebase case:
  `# Firebase Integration` + `## Setup`/`## Auth`/`## Firestore` → one
  `firebase-integration` convention with three sections).
- A document with **no** `# H1` → one candidate whose `body` is the whole
  document (after any front-matter); `title` comes from file front-matter, else
  empty (the preview requires the user to fill a title before that candidate can
  be imported).
- File-level front-matter (`title`/`tags` at the very top): used as the single
  candidate's defaults only when there is exactly one piece; with multiple pieces
  each candidate's metadata comes from its own `# H1` (tags default empty,
  editable in the preview).

### Components

#### 1. `src/knowledge.rs` — pure splitter

```rust
pub struct ConventionDraft {
    pub id: String,
    pub title: String,
    pub sections: Vec<String>,
    pub body: String,          // markdown after the `# H1` line (verbatim)
}

/// Split a markdown document into candidate conventions, one per `# H1`
/// (fence-aware). No `# H1` → a single draft over the whole body. Pure; no I/O.
pub fn split_markdown_conventions(raw: &str) -> Vec<ConventionDraft>
```

Reuses `front_matter_region`/`parse_doc_front_matter` (file-level meta),
`strip_frontmatter`, `slug`, and `sections()`.

#### 2. `src/web.rs` — two routes

- `POST /api/profile/{id}/import/preview` → `Route::ImportPreview(id)` (write_op
  semantics for body parsing, but read-only on disk). Body `{ "markdown": "..." }`.
  Handler: `split_markdown_conventions(markdown)`, then annotate each draft with
  `exists` by comparing `id` against `knowledge::conventions(kn, profile)` ids.
  Returns:
  ```json
  { "candidates": [
      { "id": "firebase-integration", "title": "Firebase Integration",
        "sections": ["Setup", "Auth", "Firestore"], "exists": false }
    ],
    "warnings": [] }
  ```
  (The `body` is not echoed back; the browser keeps the parsed pieces from its own
  split — see §3 — or, simpler, the preview returns `body` too so commit can send
  it back. **Decision: include `body` in each candidate** so the browser is the
  single source of truth and commit just returns what it received.)
- `POST /api/profile/{id}/import/commit` → `Route::ImportCommit(id)`. Body
  `{ "pieces": [ { "id", "title", "description", "tags": [..], "body" } ] }`.
  Handler: for each piece, build a normalized markdown doc
  `---\nid: {id}\ntitle: {title}\ndescription: {desc}\ntags: [..]\n---\n\n# {title}\n{body}`
  and call `knowledge::add_convention_from_markdown(conv_dir, &raw)` (profile
  conventions dir). Accumulate created/updated. Returns
  `{ "created": n, "updated": m, "ids": ["..."] }`. A piece with an empty
  `id`/`title` is rejected with a clear per-piece error (the whole commit fails
  before writing nothing partial? — **Decision: validate all pieces first**, then
  write; if any is invalid, return an error and write none).

#### 3. `src/web/app.js` — upload panel + preview (on profile detail)

Add an **"Import markdown file"** card to `renderProfileDetail(id)` (alongside the
Add convention form):

- A `<input type="file" accept=".md">` and/or a `<textarea>` to paste markdown,
  plus a **Detect** button. The file is read client-side with `FileReader`
  (`readAsText`) — no multipart upload; the text is POSTed as JSON.
- On **Detect** → `POST /api/profile/<id>/import/preview` `{markdown}`. Render each
  returned candidate as a row/card:
  - `☑ include` checkbox (checked by default),
  - editable **id** (`<input>`, default the candidate id),
  - editable **title**,
  - editable **tags** (comma-separated, default empty),
  - the detected **sections** list (read-only, muted),
  - a badge: `new` or `will update` (from `exists`).
- A **"Import selected"** button → `POST /api/profile/<id>/import/commit`
  `{ pieces: [...] }` built from the checked rows (id/title/description(empty for
  now)/tags/body, where `body` is the candidate body the preview returned). On
  success → toast `created N, updated M` and `renderProfileDetail(id)` (the new
  conventions appear in the Conventions list).
- Warnings from preview rendered at the top of the card.

#### 4. `src/web/style.css`

Minor styling for the candidate rows (the existing `.muted`/`.pill`/section
styles are reused; add a `.candidate` row rule if needed).

### Error handling

- Empty markdown / no candidates detected → preview returns
  `{ candidates: [], warnings: ["no headings found — add a `# Heading` per topic"] }`;
  the UI shows the warning and disables Import.
- File not `.md` (by name) → the UI warns before reading; pasted text is always
  accepted.
- Candidate with empty title (no `# H1`, no front-matter title) → preview returns
  it with `title: ""`; the UI shows an empty editable title and blocks Import for
  that row until filled.
- Invalid id (`[a-z0-9_-]`) on commit → the existing `validate_doc_id` error,
  surfaced per piece; commit writes nothing if any piece is invalid.
- Id collision with an existing profile convention → `will update` badge; on
  commit it upserts (replaces), consistent with the importer.

## Testing

- **`split_markdown_conventions` unit (pure):**
  - one `# H1` with two `## ` → one draft, two sections, body excludes the H1 and
    keeps a `>` blockquote.
  - two `# H1` blocks → two drafts with the right titles/ids/sections.
  - no `# H1` → one draft over the whole body; title from front-matter when
    present, else empty.
  - a `# ` inside a ``` fence is not treated as a new piece.
- **web route test (`route_parses_paths`):** `POST /api/profile/p/import/preview`
  and `.../import/commit` parse to the new variants.
- **commit path:** the commit handler is a thin loop over the already-tested
  `add_convention_from_markdown` (sections/tokens/anchors/`_index.json` upsert are
  covered by that function's existing tests), so the new coverage focuses on the
  splitter; a route-level smoke confirms a normalized piece imports.
- **CI parity:** `cargo build --release` + `cargo test --release` green, no new
  warnings.
- **Manual e2e:** `palugada web` → Profiles → android-mvvm → Import markdown:
  upload a Firebase doc → preview shows one `firebase-integration` candidate with
  its sections → Import → it appears under Conventions and `palugada q
  firebase-integration --profile android-mvvm` shows the body. Then a multi-`# H1`
  doc → multiple candidates → import a subset. Clean up the demo conventions
  afterward.

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `src/knowledge.rs` | Modify | `ConventionDraft` + `split_markdown_conventions` (+ tests) |
| `src/web.rs` | Modify | `ImportPreview`/`ImportCommit` routes + handlers (+ route test) |
| `src/web/app.js` | Modify | Import-markdown card: file/paste → Detect → preview → Import selected |
| `src/web/style.css` | Modify | candidate row styling |

## Risk / notes

- Commit normalizes each piece's metadata into fresh front-matter; if the user
  renamed the title in the preview, the stored `# H1` is rebuilt from the edited
  title (commit emits `# {title}` + body), so front-matter and body heading agree.
- The splitter is profile-agnostic and reused as-is if a CLI `convention add
  --split` or a per-project-overlay import is added later.
- Preview echoes each candidate's `body` to the browser so commit is a faithful
  round-trip; bodies are markdown text, not secrets, and the server is loopback +
  host-guarded (same as the rest of the console).
- No version bump / npm release as part of this work.
