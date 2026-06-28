# Notion DocSource fidelity — design

> Status: approved (full-fidelity) · Date: 2026-06-28 · Branch: `fix/notion-docsource-fidelity`

## Problem

Dogfooding the onboarding PRD (uploaded to Notion) via `palugada wiki page <id>` revealed the
Notion DocSource is lossy:

- **Truncates at 100 blocks** (no pagination) — the PRD was cut off at §10 of 14.
- **Drops `table`/`table_row` blocks** — every table lost.
- **Drops `code` blocks** — diagrams / CLI examples lost.
- **No child recursion** — nested list items (e.g. numbered acceptance criteria) lost.
- **Flattens structure** — headings/lists lose their markdown markers.

Root cause (`src/clients/notion.rs`): `get_page` does a single `/blocks/{id}/children?page_size=100`
call; `render_blocks` only renders a fixed set of top-level text blocks via `holder()` and skips
everything else.

## Decision

**Full fidelity.** Faithfully render the page as Markdown so a rich doc round-trips from Notion.

## Design (one file: `src/clients/notion.rs`)

- `BlocksResp` gains `has_more` + `next_cursor`.
- `Block` gains `id`, `has_children`, `code` (rich_text + language), `table` (has_column_header),
  `table_row` (cells). `to_do` carries `checked`.
- New internal `BlockNode { block, children }`.
- `Notion::fetch_children(parent_id)` (impure I/O): paginate `/blocks/{id}/children` following
  `has_more`/`next_cursor` until exhausted; for each block with `has_children`, recurse. Builds the
  node tree.
- Pure `render_nodes(nodes, &mut lines, depth)` → Markdown:
  - heading_1/2/3 → `#`/`##`/`###`; paragraph → text; quote → `> `;
  - bulleted → `- ` / numbered → `N. ` / to_do → `- [ ]`/`- [x]` (indented by depth);
  - code → fenced ```` ```lang ````; table → markdown pipe table built from `table_row` children
    (separator after row 1; cells escape `|` and newlines);
  - unknown kinds → no own line, but their children are still recursed (preserves column/toggle text).
- `get_page` → page title (unchanged) + `render_nodes(fetch_children(id))`.

## Acceptance

- **Unit (inline, TDD):** pagination fields parse; code fenced; table pipe render; nested-list
  recursion + indent; heading markers (update the existing flat-render test to the structured output).
- **Live:** re-fetch the PRD page → output reaches §14, with tables + code + nested ACs present.

## Constraints

No async (sync `ureq` via `Http`), `Result<T, String>` errors, inline `#[cfg(test)]` tests,
**no `cargo fmt`** (this repo hand-formats wide-style).
