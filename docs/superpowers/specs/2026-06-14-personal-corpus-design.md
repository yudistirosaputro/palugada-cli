# Design — Personal corpus (`prd` commands)

> **Status:** Built · **Date:** 2026-06-14 · Group B. Autonomous.

## Goal

A local, per-user markdown store at `~/.palugada/personal/` that persists fetched
tickets for offline reading + keyword search — PRD layer D. Commands:
`prd fetch <KEY>`, `prd list`, `prd cat <name>`, `prd search <kw>`.

## Design

New module `src/personal.rs` owning the corpus directory and its file ops; a
`Prd` clap command + `cmd_prd` in `main.rs`.

- **Location:** `~/.palugada/personal/` (`config::home_dir().join(".palugada").join("personal")`).
  Per-user, not committed (it's under `~/.palugada`, like secrets).
- **`prd fetch <KEY>`:** resolve the active project, fetch the issue via the
  configured `IssueTracker` (`get_issue`), and save it as
  `personal/<sanitized-key>.md` with YAML front-matter (key, summary, status,
  type, assignee, source, fetched_at) + the description body. `fetched_at` is an
  RFC3339 timestamp (`chrono`, already a dep). This is the only networked verb.
- **`prd list`:** list the `.md` doc names in the corpus.
- **`prd cat <name>`:** print one saved doc (`<name>.md`, name sanitized).
- **`prd search <kw>`:** case-insensitive substring search across saved docs;
  prints each matching doc name + a context line.
- **Filename safety:** `sanitize_name` keeps `[A-Za-z0-9._-]` and maps everything
  else to `_`, so keys like `owner/name#42` become `owner_name_42` (no path
  traversal, no separators).
- **Pure, testable helpers:** `sanitize_name` and `format_issue_doc(&Issue, ts)`
  are pure; file ops wrap them.

## Non-goals (deferred)

- The reverse-index **PR-title walk** (relating commits/PRs to docs) — separate,
  more speculative; noted for later.
- Fetching wiki pages into the corpus (`prd fetch` covers issues; wiki can be
  added with a `--wiki` flag later); uploads; cross-machine sync.

## Testing

- `sanitize_name("owner/name#42")` → `"owner_name_42"`; rejects separators.
- `format_issue_doc` includes the key, summary, and body, with valid front-matter.
- File ops (save/list/cat/search) exercised via a `tempfile`-backed corpus dir by
  pointing the helpers at an explicit dir (the IO helpers take a `dir: &Path` so
  tests don't touch the real `~/.palugada`).

## Files

`src/personal.rs` (new); `src/main.rs` (`Prd` command + `cmd_prd` + `mod personal`);
`README.md`.
