# Design — `palugada web` local authoring console (v1)

> **Status:** Approved for planning · **Date:** 2026-06-14 · Autonomous build.
> Inspired by 9router's local dashboard, but kept palugada-native: an optional,
> embedded, file-backed console — **not** a server-centric rewrite.

## 1. Problem / goal

Authoring profiles and knowledge (conventions/recipes) and generating per-project
agent skill files is done by hand-editing YAML/markdown today, which is tedious
and Android-biased. Give humans an optional local web console — `palugada web` —
to browse config/profiles/knowledge and **author profiles + knowledge + generate
agent skill files**, so the tool is stack-agnostic and user-controlled. The
console writes the **same files the CLI reads**; it adds no new source of truth.

The agent-consumption path stays the cold CLI. The server runs only while a human
is editing.

## 2. Goals (v1 scope)

- `palugada web [--port N] [--open]` — embedded synchronous server on
  `127.0.0.1`, serving a vanilla-JS single-page console (sidebar layout).
- **Read:** overview (knowledge dir, active project, default profile), projects,
  profiles, a profile's conventions/recipes/fact-families/flows, and a
  convention/recipe body.
- **Author:** create a profile; add a convention (title + token-split sections)
  or a recipe to a profile — writing the `.md` + updating `_index.json`.
- **Generate:** agent skill files into a project (reusing `init` logic).

## 3. Non-goals (later slices)

- Editing global config / **secrets** (the API never reads or writes
  `secrets.yaml`); editing per-project integration config; running connectors,
  `prd`, or the indexer from the UI; live search; auth beyond localhost; any
  build pipeline or JS framework.

## 4. Architecture

### 4.1 Server
New module `src/web.rs`. `web::run(port, open) -> Result<(), String>` starts a
**`tiny_http`** server bound to `127.0.0.1:{port}` (default 7777). Synchronous,
no tokio (matches palugada's "no async runtime" rule). A single-threaded request
loop is fine for a one-user console. `--open` best-effort launches the browser
(`open` on macOS, `xdg-open` on Linux, `cmd /c start` on Windows); failure is a
warning, not an error.

**Security:** bound to loopback only; every request's `Host` header must be
`localhost`/`127.0.0.1` (rejects DNS-rebinding). The API exposes no secrets.

### 4.2 Static UI
Vanilla `index.html` + `app.js` + `style.css` under `src/web/`, embedded with
`include_str!` → still a single binary, no build step. Sidebar layout: Overview /
Projects / Profiles / Knowledge. `app.js` fetches the JSON API and renders;
forms POST JSON. No framework, no bundler.

### 4.3 JSON API (method + path routing in `web.rs`)
| Method · Path | Action |
|---|---|
| `GET /` , `/app.js`, `/style.css` | serve embedded assets |
| `GET /api/overview` | knowledge dir, active project, default profile, counts |
| `GET /api/projects` | registry (name, repo_path, active) |
| `GET /api/profiles` | `[{id,title}]` (via `profile::list`) |
| `GET /api/profile/{id}` | conventions `[{id,title,sections}]`, recipes `[{id,title}]`, fact_families, flows |
| `GET /api/profile/{id}/convention/{cid}` | raw convention markdown |
| `GET /api/profile/{id}/recipe/{rid}` | raw recipe markdown |
| `POST /api/profile` `{id,title,languages}` | create (→ `profile::scaffold_new`) |
| `POST /api/profile/{id}/convention` `{id,title,description,tags,sections:[{title,body,code}]}` | write `<cid>.md` + update index |
| `POST /api/profile/{id}/recipe` `{id,title,description,tags,body}` | write `<rid>.md` + update index |
| `POST /api/init` `{repo,agents[],profile?,name?}` | generate agent skill files |

Errors return a JSON `{error}` with a 4xx/5xx status. All bodies are JSON
(`serde_json`).

### 4.4 New library helpers (logic out of the HTTP layer, testable + reused)
- **`knowledge` data accessors** (data, not stdout): `conventions(kn,profile) ->
  Vec<TopicMeta>`, `recipes(kn,profile) -> Vec<RecipeMeta>`,
  `convention_md(kn,profile,id) -> String`, `recipe_md(kn,profile,id) -> String`.
  (`read_conv_index`/`read_recipe_index` already exist privately — expose typed
  results.)
- **`knowledge::add_convention(kn, profile, ConventionSpec)`** — render front-matter
  + `## Title {#slug}` sections from the spec, write `conventions/<id>.md`, and
  insert/replace the topic in `conventions/_index.json`. `add_recipe(...)` likewise.
  Slugs are derived from section titles (lowercase, non-alnum → `-`).
- **`brief::flows(kn,profile) -> Vec<(String,Vec<String>)>`** — expose the flow
  list already parsed there (or a small profile.yaml reader in `web.rs`).
- **`scaffold::generate(opts) -> Result<GenerateOutcome,String>`** — split file
  generation out of `scaffold::run`; `cmd_init` prints around it, `/api/init`
  returns it as JSON. Behavior identical to today.

### 4.5 Token-split authoring
"Add convention" submits an array of sections; each becomes a `## Title {#slug}`
block — exactly the unit `q <topic>.N`, `brief`, and the budget packer pull on
demand. The UI nudges authors to break content into sections rather than one wall
of text. No new format — it's the existing convention schema.

## 5. Dependencies

Add `tiny_http = "0.12"` (synchronous HTTP, no async runtime). `serde_json` is
already present. Build-time + binary-size cost is small; no Node, no DB.

## 6. Testing

- **Writer round-trip:** `add_convention` then `profile::validate` passes and
  `knowledge::convention_md` reads the section back; `add_recipe` likewise.
- **Index update:** adding a convention inserts exactly one topic into
  `_index.json` (and replacing an existing id doesn't duplicate).
- **Slug derivation:** `slug("Errors in Coroutines") == "errors-in-coroutines"`.
- **Routing:** a pure `route(method, path)` parser maps to the right handler enum
  (incl. path params), tested without a live server.
- **`scaffold::generate`** writes the expected file set for a temp repo (the
  former `init` behavior), asserted on the returned outcome.
- **Host guard:** a pure `host_ok(header)` accepts localhost/127.0.0.1, rejects
  others.
- HTTP serving itself is smoke-tested (start, `GET /api/overview`, stop) but the
  logic lives in unit-tested helpers.

## 7. Affected files

| File | Change |
|---|---|
| `Cargo.toml` | add `tiny_http` |
| `src/web.rs` + `src/web/{index.html,app.js,style.css}` | new server + embedded UI |
| `src/knowledge.rs` | data accessors + `add_convention`/`add_recipe` + `slug` |
| `src/scaffold.rs` | extract `generate(opts)` from `run` |
| `src/main.rs` | `Web` command + `cmd_web`; `mod web`; `cmd_init` uses `scaffold::generate` |
| `README.md` | document `palugada web` |

## 8. Risks

- **Scope creep** — v1 deliberately excludes config/secrets editing and connectors
  in the UI; keep the console an authoring tool, not a control panel for live ops.
- **Single-threaded server** — fine for one local user; documented, not a bug.
- **Vanilla JS growth** — keep `app.js` small and view-segmented; if it balloons,
  that's the signal to split the file, not adopt a framework.
