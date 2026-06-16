# Per-project Credentials & Integrations in `palugada web` (cycle A)

**Date:** 2026-06-16
**Status:** approved (design)
**Branch:** `feat/web-credentials`

## Problem

Credentials and integrations differ per project, but `palugada web` can't edit
them — it exposes no secrets and has no integration editor. Setting up a project
means hand-editing `<repo>/.palugada/config.yaml` (integrations) and
`~/.palugada/secrets.yaml` (tokens). Cycle A brings that into the per-project
detail page so a project can be wired up from the browser.

## Goal

On the per-project detail page (from the cycle-B skill-flow map), add a
**Credentials & Integrations** editor that can:
- set each integration's `provider` / `base_url` / `repo` (or clear it),
- set the bound `auth_profile`'s tokens (write-only, masked on read),
- **verify** a configured integration against its provider.

Saving updates the project's `config.yaml` + `secrets.yaml`; the page re-renders
so the skill-flow map's tool-skill gating reflects the new integrations.

## Security posture

- The server is loopback-bound and Host-guarded (existing).
- **Read is always masked:** `GET .../config` returns `mask_secret(token)` for
  every secret field; plaintext tokens are never sent to the browser.
- **Write-only secrets:** an empty token field means "leave unchanged"; a
  non-empty value overwrites. Non-secret identifiers (`jira_email`,
  `wiki_email`, `jenkins_user`) are shown in full and overwrite directly.
- `secrets.yaml` keeps its atomic `0600` write (`Secrets::save`).
- **Verify** is the only network path in the web server — per click, errors
  caught and returned as data (never panics/hangs the loop), `insecure=false`.

## Components

### `src/credentials.rs` (new)

- `supported_providers() -> serde_json::Value` — capability → allowed provider
  names (mirrors the `clients::*` factories):
  `issue_tracker:[jira,github_issues]`, `wiki:[confluence,notion]`,
  `git_host:[github,gitlab]`, `design:[figma]`,
  `ci:[jenkins,github_actions,gitlab_ci]`, `chat:[slack]`.
- `project_config_json(global, name) -> Value` — `{project, profile,
  auth_profile, integrations{cap→{provider,base_url,repo}|null}, providers,
  secrets{...masked...}}`. Tokens masked via `config::mask_secret`; emails /
  `jenkins_user` shown in full.
- `save_project_config(global, name, body) -> Value` — parse payload; load
  `ProjectConfig`, set `auth_profile` (default `default` if blank) and each
  integration (provider `"(none)"`/blank → clear; absent in payload → leave),
  `save_to`. Then load `Secrets`, update `auth_profiles[auth_profile]`:
  non-secret fields overwrite directly; secret fields overwrite only when
  non-empty; `save`.
- `verify_capability(global, name, cap) -> Value` — load project config +
  secrets + bound auth profile, build the capability's client via
  `clients::{issue_tracker|doc_source|git_host|design_source|ci_provider|chat_notify}`,
  call `verify()`. Returns `{ok:true, message}` on success, `{ok:false, error}`
  on a verify/connection failure; `Err` only for an unknown capability.

### `src/web.rs` — routes

- `GET  /api/project/{name}/config`        → `project_config_json` (read; 500 on error)
- `POST /api/project/{name}/config`        → `save_project_config` (write_op; 400 on bad input)
- `POST /api/project/{name}/verify/{cap}`  → `verify_capability` (read; verify failures are 200 `{ok:false}`)

`name` URL-decoded (like the other project routes).

### `src/main.rs`

`mod credentials;`.

### `src/web/app.js` — credentials editor

In `renderProjectDetail`, fetch `/api/project/<name>/config` and render a
**Credentials & Integrations** card above SKILL FLOW:
- `auth_profile` text input (with a note: shared by all projects using the same
  name).
- One row per capability: provider `<select>` (options from `providers[cap]` +
  `(none)`), `base_url` input, `repo` input, **[Verify]** button + a result
  badge (`✓ message` / `✗ error`).
- Secret inputs (`type=password`) with the masked current value as placeholder;
  `jira_email`/`wiki_email`/`jenkins_user` as plain pre-filled text inputs.
- **Save** → `POST .../config` → on success re-render the detail page (skill-flow
  tool-skills update).
- `[Verify]` → `POST .../verify/<cap>` → show the badge.

### `src/web/style.css`

Minor: verify-result badge + form-row spacing.

## Error handling

- Unknown/unregistered project → 500 with a clear message.
- Bad JSON / unknown capability → 400 / `Err`.
- Verify failure (bad token, unreachable host, missing integration) → `200`
  `{ok:false, error}` rendered as a `✗` badge; never crashes the server loop.
- Saving with a not-yet-existing `auth_profile` creates it.

## Testing

- **Unit (`src/credentials.rs`, temp dirs):**
  - `project_config_json` masks every token (asserts the raw token never appears
    in the JSON; `mask`/`(unset)` present).
  - `save_project_config`: empty token leaves the existing value; non-empty
    overwrites; provider `(none)` clears an integration; a set provider
    round-trips through `ProjectConfig::load_from`; email overwrites directly.
  - `supported_providers` lists every capability.
- **Route test (`src/web.rs`):** the three new paths parse.
- **Manual e2e (`palugada web`):** open a project → set `git_host=github`,
  `repo=owner/name`, paste a token → Save → re-render shows `palugada-git`
  active in the skill map; `secrets.yaml` updated (token masked on reload);
  Verify returns `✓`/`✗`. (Verify's network path is manual-only.)

## Files

| File | Action |
|---|---|
| `src/credentials.rs` | Create (providers, read JSON, save, verify) + unit tests |
| `src/main.rs` | `mod credentials;` |
| `src/web.rs` | 3 routes + handlers + route test |
| `src/web/app.js` | credentials editor in the detail page |
| `src/web/style.css` | verify badge / form rows |

## Risk / notes

- Editing tokens edits the bound `auth_profile` (shared by all projects using
  that name) — surfaced with a UI note. Per-project-only auth isn't in scope.
- Verify introduces network calls to the web server; kept per-click and
  error-boxed so a hang/failure never affects editing or the rest of the UI.
