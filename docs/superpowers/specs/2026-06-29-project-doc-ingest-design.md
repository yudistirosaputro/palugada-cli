# F6 — Per-project doc ingest (wiki + issue → project knowledge)

> Status: approved · Date: 2026-06-29 · Branch: `feat/project-doc-ingest`
> PRD: `docs/PRD-onboarding-connectors.md` (extends the deferred "wiki-corpus ingest").

## Problem

A user fetched their PRD (in Notion) with `palugada prd fetch <id>` and got
"jira base_url is empty" — because `prd fetch` is hardwired to the **issue tracker**,
not the **wiki/DocSource**. The correct command for Notion is `wiki page <id>`, but that
only prints (no corpus), and the corpus (`prd list/cat/search`) is the GLOBAL
`~/.palugada/personal/`, not per-project. The user wants: ask → search first; fetch /
get-info / page-id → auto-download into the **per-project knowledge**, visible in the
**web console**.

## Decisions (locked)

- **Per-project store, local & gitignored:** `<repo>/.palugada/docs/` — a re-fetchable cache.
  The dir self-ignores (a `.gitignore` containing `*` is written on first save) so fetched
  (possibly sensitive) ticket/PRD content never gets committed.
- **Ingest on both** `wiki page <id>` (DocSource → `source: wiki`) **and** `prd fetch <KEY>`
  (IssueTracker → `source: issue_tracker`). Plus a clearer `prd fetch` error hint.
- `prd list/cat/search` operate on the **per-project** docs dir (so "search first" finds them).
- Web console gets a per-project **Docs** view.
- The `palugada-docs` skill tells the agent: search first (`prd search`/`list`), fetch via
  `wiki page`/`prd fetch` (auto-saves to the project's docs).

## Design

### Storage (`src/personal.rs` — already dir-parameterized)
- New `format_wiki_doc(page: &WikiPage, fetched_at)` (front-matter `id`/`title`/`source: wiki`/
  `fetched_at` + body) and `save_wiki(dir, page, fetched_at)` (stem = sanitized title, else id).
- New `ensure_dir_ignored(dir)`: `create_dir_all` + write `.gitignore` = `*` if absent.
- Reuse `list`/`cat`/`search`/`save_issue` unchanged (they take `dir`). `list` already filters `*.md`,
  so the `.gitignore` is invisible to it.

### CLI (`src/main.rs`)
- Helper `project_docs_dir(global, project) -> PathBuf` = `<resolved repo_path>/.palugada/docs`
  (via `resolve_project_name` + `expand_home`).
- `cmd_wiki::Page`: after `get_page`, `ensure_dir_ignored` + `save_wiki`; print the page AND
  the saved path.
- `cmd_prd`: resolve the per-project docs dir (not `personal::dir()`); `Fetch` saves there
  (`ensure_dir_ignored` first); `List/Cat/Search` read there. `Fetch`'s issue-tracker build
  error gets a hint: "`prd fetch` reads the issue tracker; for a Notion/wiki page use
  `palugada wiki page <id>`".

### Web (`src/web.rs` + `src/web/app.js`)
- `GET /api/project/{name}/docs` → `[{name, title, source, fetched_at}]`; `GET .../docs/{doc}` → body.
- Project-detail **Docs** card listing ingested docs (source badge + fetched_at) with view.

### Skill (`src/scaffold.rs`)
- `palugada-docs` SKILL body: "to answer about a ticket/PRD/page, FIRST `palugada prd search`/
  `prd list`; fetch with `palugada wiki page <id>` (Notion/Confluence) or `prd fetch <KEY>`
  (issue) — both save into this project's `.palugada/docs/` cache."

## Acceptance criteria

1. `palugada wiki page <notion_id>` saves a `source: wiki` doc into `<repo>/.palugada/docs/` and
   `prd list`/`prd search <kw>` find it (per-project).
2. `palugada prd fetch <KEY>` with no/blank issue-tracker base_url errors with the wiki hint
   (no silent jira failure surprise).
3. The docs dir is self-ignored (`.gitignore` = `*`); `list` never shows non-`.md`.
4. Web project-detail Docs card lists the ingested docs; existing tests stay green.

## Constraints
No async, `Result<T,String>`, inline tests, gitignored cache (no committed external content),
loopback+host-guarded web. **No `cargo fmt`** (repo hand-formats wide-style).
