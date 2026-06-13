# Design — Notion DocSource

> **Status:** Built · **Date:** 2026-06-14 · Group A sub-project 4. Autonomous.

## Goal

Add a `notion` provider behind the existing `DocSource` trait so
`palugada wiki page <id>` works on Notion. Reuses the `wiki_token` secret slot
(it's the doc-source token) — no new secret.

## Design

`DocSource::get_page(id) -> WikiPage { id, title, body_html }` and `verify()`
are unchanged. New module `clients/notion.rs` + a `notion` arm in `doc_source()`.

- **Config:** `wiki: { provider: notion }` (`base_url` defaults to
  `https://api.notion.com`). Auth: `wiki_token` as a Bearer token, with the
  required `Notion-Version: 2022-06-28` header.
- **get_page:** two calls (Notion splits metadata from content):
  - `GET {base}/v1/pages/{id}` → the page; the **title** is the value of the one
    property whose `type` is `title` (`title[].plain_text` joined).
  - `GET {base}/v1/blocks/{id}/children?page_size=100` → child blocks; **body**
    is the plain text of the common text blocks (`paragraph`, `heading_1..3`,
    `bulleted_list_item`, `numbered_list_item`, `to_do`, `quote`), one line each.
  - `body_html` carries plain text (Notion has no storage-HTML; the field name is
    historical). Pagination beyond the first 100 blocks is out of scope.
- **verify:** `GET {base}/v1/users/me` → `"Notion OK — authenticated as {name}"`.
- **Parsing is pure + testable:** `page_title(json)` and `render_blocks(json)`
  take the raw response JSON and return strings, so they unit-test without network.

## Non-goals

- Writing pages; nested/child-block recursion; databases; >100 blocks.

## Testing

- `page_title` extracts the title-type property's text from a sample page JSON;
  returns "" when absent.
- `render_blocks` joins paragraph/heading text from a sample blocks JSON and
  skips unknown block types.

## Files

`src/clients/notion.rs` (new); `src/clients/mod.rs` (`notion` arm + `mod`);
`README.md`.
