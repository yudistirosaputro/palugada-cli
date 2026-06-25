# Connectors & API Keys — a global setup menu in `palugada web`

**Date:** 2026-06-25
**Status:** approved (design)
**Branch:** `feat/web-connectors-menu`
**Mockup:** artifact `c14b13c7-f037-4fab-b679-ab081a43ed21` · reference `docs/superpowers/specs/2026-06-25-web-connectors-menu-mockup.html`

## Problem

palugada's connector setup is split across two stores but exposed through **one
per-project surface only**:

- **API keys (secrets) are GLOBAL** — a single `~/.palugada/secrets.yaml` (chmod
  `0600`, atomic write), keyed by *auth-profile* name, holding 9 token/email/user
  fields (`AuthProfile`, `config.rs:119-142`). The same `default` profile is shared
  by every project.
- **Provider wiring is PER-PROJECT** — which provider, `base_url`, `repo` per
  capability, stored in each repo's `.palugada/config.yaml` (`Integrations` → 6
  capability slots of `Option<Provider{provider, base_url, repo}>`, `config.rs:203`).
- **The only editor is `credentialsCard()`** inside Project Detail
  (`app.js:500-570`, rendered from `renderProjectDetail`). Every credential/secret
  route is project-scoped (`web.rs:96-100`, `["api","project",name,…]`). There is
  **no global surface at all**.

The consequences:

1. **Onboarding dead-end** — to enter any API key you must first register a
   project, open it, then edit secrets *through* it, even though the key is global.
   "I just cloned palugada and want to paste my GitHub token" has no home.
2. **Auth profiles are invisible** — `auth_profile` is a bare editable string
   (`app.js:502`); no list/create/switch UI, and the multi-profile the model already
   supports (`default`/`staging`/`production`) is unreachable.
3. **No shared wiring** — every project re-picks `github` + `api.github.com` from
   scratch; there is no notion of a sensible default a new project inherits.

## Goal

Add a new top-level **Connectors** menu to `palugada web` that manages, globally:

- **API keys** for the 6 existing capabilities, written to `~/.palugada/secrets.yaml`
  under the `default` auth-profile, with the established masking + blank-keeps
  contract.
- **Default wiring** (provider + `base_url`) per capability, stored globally and
  **inherited per-field by projects**, which still own their `repo`.
- **In-place Verify** wherever a capability can be checked without a repo; an honest
  "verify from a project" marker where it can't.

Decisions locked during brainstorming:

| Decision | Choice |
|---|---|
| Connector scope | The **6 existing capabilities** only — no new connector type |
| Page shape | **Global key page + default wiring**; projects inherit, override `repo` |
| Auth profiles | **Focus `default`**; page structured so a switcher can be added later |
| Verify | **In-page where possible**; repo-bound caps marked "verify from a project" |
| Layout (UI) | **Compact / accordion** default (cards collapsed; click to expand) |
| Visual identity | **Reuse the Pop Workbench design system verbatim** (no new style) |

### Non-goals (YAGNI for v1)

- **New connector kinds** (LLM/AI keys, other trackers) — needs new traits in
  `src/clients` + new secret fields; out.
- **Full multi auth-profile UI** (create/rename/delete/switch) — the page reads &
  writes the `default` profile; a profile selector is a later slice. The chip shows
  `default` + a `multi-profile soon` hint.
- **"Sample repo" field** for verifying repo-bound caps from the global page — those
  stay "verify from a project".
- **Inheritance indicators in the per-project editor** — `credentialsCard` in
  Project Detail is left as-is for v1 (see §7, Known nuance).
- **A new CLI command** — this is a web slice. Backend functions are pure and unit
  tested regardless; a `palugada secrets`/`config` CLI surface is deferred.

## Approach

Two stores, two concerns, one page:

```
~/.palugada.yaml            GlobalConfig.default_integrations   (provider + base_url)   ← NEW
~/.palugada/secrets.yaml    Secrets.auth_profiles["default"]    (the 9 key fields)      ← exists
```

The page edits both; runtime resolution merges the global defaults *under* each
project's explicit wiring at the single existing chokepoint, `resolve_project()`.

### Precedence (per capability, per field)

```
global default wiring   →   project .palugada/config.yaml   (project wins per-field)
   provider, base_url           provider, base_url, repo
```

- `provider` = project's if non-empty, else global default.
- `base_url`  = project's if non-empty, else global default.
- `repo`      = always the project's (global default carries no meaningful repo).

So a project can set **just `repo`** and inherit provider + base_url globally.
Because `default_integrations` is empty until the user sets it, **existing projects
are byte-for-byte unaffected** until a global default exists (backward compatible).

## Architecture & components

### 1. Config model — `src/config.rs`

- Add `default_integrations: Integrations` to `GlobalConfig` with
  `#[serde(default, skip_serializing_if = "…is_empty")]` so `~/.palugada.yaml`
  files without it parse, and an all-empty value never gets written. Reuse the
  existing `Integrations`/`Provider` structs (the `repo` sub-field is simply unused
  at global scope).
- Extend `resolve_project()` (`config.rs:383-411`) to fold `default_integrations`
  **under** the project's `integrations` with the per-field rule above, producing the
  effective `Integrations` the client factories already consume. This is the only
  resolution change; `brief`, `issue`, `pr`, `wiki`, `ci`, `notify`, `design` all
  inherit it for free because they all build clients via the resolved config.
- A small pure helper `merge_integrations(global, project) -> Integrations`
  (per-field, empty = inherit) so the merge is unit-testable in isolation.

### 2. Backend — `src/credentials.rs` (pure core + thin I/O, mirrors today's split)

New functions next to `project_config_json`/`save_project_config`/`verify_capability`,
operating on global stores with **no project required**:

- `global_view() -> serde_json::Value` — the page's read model:
  - `providers`: the `supported_providers()` whitelist (already shipped as
    `cfg.providers`), so the UI can populate per-capability dropdowns.
  - `default_integrations`: provider + base_url per capability (no repo).
  - `secrets`: every key field **masked** via the existing `mask_secret()`
    (`config.rs:467-473`) → `(unset)` / `**** (N chars)`. Email/user fields shown in
    full (matches today). **Plaintext never leaves the process.**
  - `auth_profile`: `"default"` (constant for v1).
  - Per-capability `verify`: `now` | `repo` | `none` (see §3) so the UI renders the
    right affordance without hard-coding.
- `apply_global(submit) -> Result<…>` — write path, reusing today's contracts:
  - default wiring → `~/.palugada.yaml`: provider `(none)`/blank **clears** the slot,
    an absent capability is **left as-is** (mirrors `apply_integrations`,
    `credentials.rs`).
  - secrets → `~/.palugada/secrets.yaml` under `default`: a token overwrites **only
    if non-empty** ("blank = keep"); non-secret fields (email/user) overwrite
    directly (mirrors `apply_secrets`, `credentials.rs:136-152`). The 0600 atomic
    write is the existing `Secrets` save path.
- `global_verify(cap) -> serde_json::Value` — see §3.

Save granularity is **per connector (capability)**: the handler for one capability
writes exactly the wiring + secret fields that capability owns (table in §4).

### 3. Verify behaviour — `global_verify(cap)`

Classify each capability by whether `.verify()` needs a `repo`:

| Needs repo? | Capabilities | Global verify result |
|---|---|---|
| **No** | `issue_tracker`(jira), `wiki`(confluence/notion), `design`(figma) | build an **ephemeral `ProjectConfig`** from `default_integrations` + the resolved secret, call the real `.verify()`, return `{ok, message}` / `{ok:false, error}` as data |
| **Yes** | `git_host`, `issue_tracker`(github_issues), `ci`(github_actions/gitlab_ci/jenkins) | return `{needs_repo:true, message:"verify from a project"}` — no network call |
| **N/A** | `chat`(slack) | `{tested:false, message:"saved; Slack isn't pinged"}` (a real verify would POST) |

> The exact repo-vs-no-repo split per provider must be confirmed against the actual
> `.verify()` bodies in `src/clients/*` during the plan's first task (e.g. whether
> Jenkins needs only base_url+user+token, whether Notion's check is repo-free). The
> table above is the design intent; the implementation reads the truth from the
> client code.

Verify stays the **only** network path, per-click and error-boxed — same security
posture as today's per-project verify, now also reachable globally for repo-free caps.

### 4. Capability → fields → secret mapping (provider-aware)

The fields a connector card shows depend on the selected provider:

| Capability (card) | Provider | base_url | Other field | Secret field(s) written |
|---|---|---|---|---|
| Git Host | github / gitlab | optional | — | `git_token` |
| Issue Tracker | jira | required | `jira_email` | `jira_token`, `jira_email` |
| Issue Tracker | github_issues | — | — | *(inherits `git_token`)* |
| Docs & Wiki | confluence | required | `wiki_email` | `wiki_token`, `wiki_email` |
| Docs & Wiki | notion | — | — | `wiki_token` |
| Design | figma | — | — | `figma_token` |
| CI / Pipelines | github_actions / gitlab_ci | — | — | *(inherits `git_token`)* |
| CI / Pipelines | jenkins | required | `jenkins_user` | `jenkins_user`, `jenkins_token` |
| Chat & Notify | slack | — | — | `chat_webhook` |

When a provider *inherits* a key, the card shows
`↳ Inherited from Git Host (git_token)` instead of a key input.

### 5. Routes — `src/web.rs` (global, no project segment)

Add to the `route()` pattern-match next to the other global GETs (`/api/overview`,
`/api/profiles`, ~`web.rs:65-67`), with new `Route` enum variants (~`web.rs:53`):

| Method | Path | Route | Handler |
|---|---|---|---|
| GET | `/api/connectors` | `Connectors` | `credentials::global_view()` |
| POST | `/api/connectors/{cap}` | `SaveConnector(cap)` | `credentials::apply_global()` (one capability) |
| POST | `/api/connectors/{cap}/verify` | `VerifyConnector(cap)` | `credentials::global_verify(cap)` |

`{cap}` is validated against the 6 known capability ids (unknown → `NotFound` / 400).
Extend `route_parses_paths()` (`web.rs:666-726`). Host-guard + loopback unchanged.

### 6. Frontend — `src/web/index.html` + `src/web/app.js`

- **`index.html:24-28`** — add `<a class="nav-item" data-view="connectors">…plug
  icon…Connectors</a>`. Click wiring auto-binds (`app.js:273-275`); no JS wiring.
- **`app.js:272`** — add `connectors: renderConnectors` to the `VIEWS` map.
- **`renderConnectors()`** — fetch `/api/connectors`, render with existing helpers
  (`viewHead`, `.card`, `.field`, `.pill`, `toast`):
  - view-head: eyebrow `CONNECTORS`, h1 "Connectors & API Keys", subtitle, the
    `default` auth-profile chip (`+ multi-profile soon`).
  - a summary stat strip (Connectors / Configured / Verified / Not set) reusing
    `.stat-grid`.
  - 6 connector cards, **accordion, collapsed by default** (header = icon + title +
    `cap · provider` + status pill + chevron; body = provider-aware fields). Status
    pill: green `Connected`, blue `Ready` (repo-bound), grey `Saved` (slack), dashed
    `Not set`.
  - per card: **Save** → `POST /api/connectors/{cap}` then re-render; **Verify** →
    `POST /api/connectors/{cap}/verify`, render the `now/repo/none` result inline.
  - key inputs: `type=password`, placeholder `•••• N chars · blank = keep`, a
    **Show/Hide** reveal toggle; the established secret contract.
  - a "How keys are stored" note card (path, 0600, never-returned, blank-keeps).

All new CSS reuses existing tokens; net-new classes only for the connector card
internals (icon box, status pill variants, key row) — same approach as
`credentialsCard`/`importCard`. No palette or font change.

### 7. Known nuance (documented, deferred)

With runtime merge, a project that leaves a capability empty will now *use* the
global default at runtime while its per-project `credentialsCard` still renders the
slot as empty. For v1 this is acceptable (defaults start empty; the global page is
the source of truth). A later slice can surface `inherited: github · api.github.com`
placeholders in the project editor. Called out so it isn't a surprise.

## Testing

Follow the repo's existing discipline (~140 tests, `cargo test`; **no `cargo fmt`** —
the repo hand-formats wide-style; CI = build/test/smoke):

- `merge_integrations` — per-field inherit; empty inherits, non-empty wins; repo
  always project; all-empty global = no change (existing-project parity).
- `global_view` — masking applied to every token field; emails/users in full;
  `verify` classification per capability correct; `providers` whitelist present.
- `apply_global` — blank token keeps existing; non-empty overwrites; `(none)`/blank
  provider clears the default slot; absent capability untouched; 0600 preserved.
- `global_verify` — repo-bound caps return `needs_repo` with **no** network attempt;
  slack returns `tested:false`; (repo-free verify exercised behind the existing
  `--insecure`/mock seam used by today's verify tests, if any).
- `route_parses_paths` — the 3 new global routes parse; unknown `{cap}` rejected.
- Live e2e (curl, like prior web work): GET masked read, POST a key (blank-keep
  proven by re-read), POST verify for a repo-free cap and a repo-bound cap; confirm
  the page serves and `/api/*` JSON intact. Frontend smoke via `node --check` + a
  manual browser click-through (per the inheritance-Plan-B reinstall note).

## Security

Unchanged posture, now with one new global network path:

- **Read always masked** — `global_view` never emits a plaintext token (reuses
  `mask_secret`).
- **Write-only secrets** — blank keeps; tokens flow browser → disk only, never back.
- **Loopback + host-guard** — the new routes are ordinary global routes under the
  existing 127.0.0.1 + Host-header guard.
- **Verify is the only outbound call** — per-click, error-boxed, repo-free caps only
  from the global page.
- `~/.palugada/secrets.yaml` stays `0600`, atomic write.

## Files touched (estimate)

- `src/config.rs` — `GlobalConfig.default_integrations`, `merge_integrations`,
  `resolve_project()` merge.
- `src/credentials.rs` — `global_view`, `apply_global`, `global_verify`.
- `src/web.rs` — 3 `Route` variants + dispatch + `route_parses_paths`.
- `src/web/index.html` — Connectors nav item.
- `src/web/app.js` — `renderConnectors` + `VIEWS` entry + connector-card render
  helpers + CSS-class usage.
- `src/web/style.css` — connector-card-specific classes (status pill variants, key
  row, accordion) in the existing token language.
