# PRD — Frictionless Onboarding & Per-Project Connectors (`palugada`)

> **Status:** Draft for review · **Author:** yudistiro.saputro@tuntun.co.id · **Date:** 2026-06-28
> **Builds on:** [`PRD-unified-palugada.md`](../PRD-unified-palugada.md) (the unification vision, shipped as of v0.2.1)
> **Scope:** the *adoption surface* — how a brand-new project goes from `git clone` to a
> working, connected, AI-ready repo with the least possible friction.

---

## 0. TL;DR

A freelancer (or any consultant) cycles through client projects on different stacks with
different toolchains: client A keeps docs in Notion and issues in Jira; client B uses
Confluence and GitHub Issues. The dream is: drop into a new repo, run **one** onboarding
step, pick the AI agent, get the right skills installed, choose **per-section connectors**
(which tool feeds *issues*, which feeds *docs*, which feeds *CI*…) with URL + token, verify
them, and start working — **without hand-wiring skills or hand-editing credential files for
every new project.**

Most of the foundation already ships in **v0.2.1**: offline `init`, profile-agnostic skill
generation, the 3-file config model, six pluggable connector slots, per-project
verify-before-save, profiles with inheritance, and budgeted `brief` context packs. This PRD
**snapshots what is shipped (✓)** and specifies the **roadmap of the missing "frictionless"
pieces** — reusable per-client credential sets with a real UI, a guided onboarding wizard,
and a graceful path when a stack has no profile yet.

This document does **not** replace [`PRD-unified-palugada.md`](../PRD-unified-palugada.md);
that PRD is the architectural foundation. This one narrows the lens to onboarding and
connectors, the surface a new user actually touches first.

---

## 1. Context — relationship to the unified PRD

The unified PRD set out to merge two repos into one project-agnostic tool, package
stack-specific knowledge as **profiles**, make integrations **pluggable per project**, and
add **budgeted retrieval flows**. As of **v0.2.1** that vision is largely realized:

- One repo, one binary, four bundled profiles (`rust-cli`, `android-mvvm`, `flutter-bloc`, `kmp`).
- Six connector capabilities behind traits, configured per project.
- Offline `init` that scaffolds config + generates agent skill files.
- `brief <flow> <target>` budgeted context packs.
- A local web console (`palugada web`) for authoring profiles and wiring connectors.

What remains is not architecture — it is **ergonomics at the moment of adoption**. The
unified PRD assumed a one-time install wizard and a single team. A freelancer is a different
shape: *many* short-lived projects, *many* credential sets, *many* stacks, and a strong
allergy to repeating setup. This PRD addresses that shape.

---

## 2. The freelance dream (problem statement)

**Persona — "Yudi the consultant."** Picks up a new client repo every few weeks. Each client
has its own Jira/GitHub, its own wiki (Confluence or Notion), its own CI, its own git host,
and its own access tokens. Yudi wants palugada to make every new repo *AI-ready in minutes*,
reusing the knowledge and the credential sets he already set up — never re-doing the plumbing.

**Today, onboarding a new client repo forces manual steps:**

| # | Friction today | Why it hurts the freelancer |
|---|---|---|
| T1 | After `init`, the integration `base_url`s are written **empty**; the user must hand-edit `<repo>/.palugada/config.yaml`. | Per-project, repeated every time, easy to typo. |
| T2 | Credentials for a *new client* must be added by **hand-editing `~/.palugada/secrets.yaml`** — the web console only manages the single `default` auth-profile. | No reusable "client-A credential set"; the core promise breaks at the second client. |
| T3 | If the stack has **no matching profile** (e.g. a React/JS repo), onboarding silently binds a profile that doesn't exist → downstream commands fail. | The most common freelance stack (web) is currently a dead end. |
| T4 | There is **no guided "pick a connector for each section"** step — the user assembles the six slots one card at a time, with no single "you're ready" signal. | Setup is a scavenger hunt, not a wizard. |
| T5 | Authoring/reusing knowledge (conventions/recipes) across projects works, but there's **no nudge** connecting "I edited a profile" → "re-sync skills into the projects that use it." | Knowledge improvements don't propagate without manual recall. |

The goal of this PRD is to erase T1–T5.

---

## 3. Goals, non-goals, and success metrics

### 3.1 Goals

- **G1 — One-command onboarding.** From a fresh repo to a working `brief` with **zero file
  hand-edits**.
- **G2 — Per-section connector picker with verify.** Choose the provider + URL + token for
  each of the six capability slots, verified before save, in a guided flow.
- **G3 — Reusable named credential sets.** Create, switch, and manage **auth-profiles** ("client-A",
  "client-B") through a real UI and CLI — not by editing YAML.
- **G4 — Graceful "no profile yet" path.** When a stack has no profile, offer a one-click
  starter (scaffold or `extends` a base) instead of a dead end.
- **G5 — Author once, install everywhere.** Editing a profile's knowledge should make it easy
  to re-sync skills into every project bound to it.

### 3.2 Non-goals (v1)

- **No OAuth / secret managers.** Personal Access Tokens (PATs) and webhooks only; secrets
  stay in the 0600 file (keychain/`env:` remain the deferred options from the unified PRD §16 O4).
- **Not building the `web-react` profile content.** Fixing the broken auto-detect is in
  scope; authoring a full React profile is a separate effort.
- **No MCP server.** Consumption stays cold-invoked CLI + per-project skill files.
- **No team/multi-user credential sharing.** Auth-profiles are per-machine, local.

### 3.3 Success metrics

| Metric | Baseline (today) | Target |
|---|---|---|
| Steps to first `palugada brief` on a fresh repo | several manual file edits | ≤ 1 guided flow, **0 hand-edits** |
| Manual `~/.palugada/secrets.yaml` edits to add a new client | 1+ (required) | **0** |
| Connector readiness signal before the first real command | none | one "verify all" pass/fail per slot |
| React/JS repo onboarding | broken (binds a non-existent profile) | works, or a clear actionable message |
| Knowledge edit → propagated to bound projects | manual recall | one-click re-sync offered |

---

## 4. Current-state snapshot (verified against v0.2.1 source on 2026-06-28)

The table is the honest baseline: what already works (✓) and where the gap is (→ feature F#
in §7). The roadmap falls out of the right-hand column.

| Capability area | Shipped (✓) | Gap |
|---|---|---|
| **Project init / scaffold** | offline `init`; registers project; `--agents auto` detects existing agent files | empty connector URLs; no guided wizard → **F2** |
| **Skill generation** | profile-agnostic router skills + tool skills gated by integrations; `skills sync` expand-not-overwrite; Claude/Codex dirs + gemini/cursor guide; marker-block merge | no "re-sync into bound projects" nudge → **F4** |
| **Config model** | 3 files (`~/.palugada.yaml`, `~/.palugada/secrets.yaml` 0600, `<repo>/.palugada/config.yaml`); `merge_integrations()` (project wins per-field); `resolve_project()` chokepoint | — (solid) |
| **Connectors (6 slots)** | jira/github_issues · confluence/notion · figma · jenkins/github_actions/gitlab_ci · github/gitlab · slack; per-project edit + **verify-before-save**; Notion DocSource renders full pages (paginate + tables + code + nesting, shipped 2026-06-28) | no per-section picker UX → **F2**; no "verify all" sweep in onboarding → **F5** |
| **Credentials** | `auth_profiles` map; masked-on-read; blank=keep; loopback + host-guarded web console | only the `default` profile has a UI → **F1** |
| **Profiles** | rust-cli / android-mvvm / flutter-bloc / kmp; `extends:` live inheritance; per-project convention overlay + `review_map` override; `profile new/list/validate/use` | `web-react` auto-detected but not on disk (🐞) → **F3** |
| **Knowledge authoring** | web: create profile, add convention/recipe, flow editor, markdown import; CLI `convention add` / `recipe add` | import→validate→use not one guided path → **F4** |
| **Context packs** | `brief <flow> <target> [--budget N] [--json]`; priority-fill + truncate/omit | token telemetry (`stats`) does not exist → optional roadmap |
| **Readiness** | `doctor` project-wide connector + tool sweep | not surfaced in onboarding → **F5** |

---

## 5. The mental model — kitchen → plate

palugada has four knowledge concepts. A cooking metaphor keeps the **direction** straight
(the common confusion is inverting *flow* and *recipe*).

```
Skill  ──(palugada brief)──▶  Flow  ──▶  [ Recipe + Conventions + live engine facts ]  ──▶  Convention sections
(waiter)                      (prep order)   (recipe card) (pantry ingredients)
```

| Concept | Kitchen analogy | What it actually is |
|---|---|---|
| **Convention** | a pantry ingredient with its spec card | a reusable knowledge doc (markdown, split into token-sized `## sections`, indexed in `_index.json`). "How we handle errors," "our architecture." Used by many dishes. |
| **Recipe** | the recipe card | a task playbook (markdown) that **references** convention sections (`convention_refs`) and related recipes. It *points at* ingredients; it does not contain them. |
| **Flow** | the prep/plating order on the ticket rail | an ordered step list in `profile.yaml` → `flows:`. Each step is an engine lookup (`symbol.find`, `module.info`, `diff.scan`, `prd.context`, `code.recent`), a `convention(<id>)`, a `recipe(<id>)`, or `convention(by-file-kind)` (expands via `review_map`). **The flow sits *above* the recipe** and orders the steps. |
| **Skill** | the waiter who fires the order | a generated agent file (`.claude/skills/palugada-feature/SKILL.md`) whose whole job is to run `palugada brief <flow> <target>` at the right moment, then pull specific cards on demand. |
| **`brief`** | the expediter at the pass | runs the flow, assembles a **budgeted plate**: priority-fill (`prd.context`=5, `symbol.find`/`module.info`/`diff.scan`=4, `convention`=3, `recipe`=2, `code.recent`=1), truncate the lowest-priority pack that overflows, and never fully drop the top-priority pack. |

> **The one sentence to remember:** *A recipe is a card that points at ingredients; a flow is
> the order you fire the steps; the skill is the waiter who fires the flow.*

**Worked example** — the `rust-cli` profile's `feature` flow is
`[recipe(feature), module.info, convention(architecture)]`. The `palugada-feature` skill runs
`palugada brief feature <target>`, which plates the *feature* recipe card + the live module
facts + the *architecture* convention — within the token budget.

---

## 6. Architecture & config model (delta only)

The full three-home model is in the unified PRD §4–§5. This PRD touches only the pieces the
new features build on.

**A "client" in palugada =** a **profile binding** (what stack knowledge to use) **+** an
**auth-profile** (which credential set) **+** the **six wired connector slots** (where issues,
docs, design, CI, git, chat come from).

```
~/.palugada/secrets.yaml                 <repo>/.palugada/config.yaml
  auth_profiles:                           profile: <stack>
    client-a:        ◀── referenced by ──  auth_profile: client-a
      jira_token …                         integrations:
      wiki_token …                           issue_tracker: { provider, base_url }
    client-b:                                wiki:          { provider, base_url }
      git_token …                            design / ci / git_host / chat { … }
```

- The **`auth_profiles` map** is the unit that becomes a reusable "client credential set."
  Today only `default` has a UI; **F1** unlocks the rest.
- **`merge_integrations()`** folds `~/.palugada.yaml`'s `default_integrations` *under* a
  project's explicit slots — project wins per-field, `repo` is always the project's. So a
  freelancer can set common wiring once globally and override only what differs per client.
- **`resolve_project()`** is the single chokepoint where name → config → merged integrations →
  auth-profile resolve; every command (`brief`/`issue`/`wiki`/`ci`/`pr`/`notify`) inherits it.
- **Project resolution order** is `--project <name>` → **cwd inside a registered repo** (deepest
  match) → `projects.active`. So when you work *inside* a client repo, palugada auto-targets it
  and `active` is ignored; `active` is only the last-resort default for commands run outside any
  registered repo. For the freelance flow, prefer cwd + `--project` and treat `active` as a
  convenience fallback (see §13 O5).

---

## 7. Feature specifications (roadmap)

Ranked by impact on the freelance dream. Each feature: the user problem, the proposed
solution, testable acceptance criteria, and the code it touches. **Dependency note:** F2 and
F5 depend on F1 (they target a chosen credential set).

### F1 — Reusable per-client credential sets with a real UI  ·  HIGHEST

- **Problem.** Every client needs its own token set, but the only credential UI manages the
  single `default` auth-profile (the Connectors page even shows a "multi-profile soon" tag).
  A second client forces hand-editing `~/.palugada/secrets.yaml` (T2) — breaking the dream's
  core promise.
- **Solution.** Generalize the global connector view/apply/verify off the hardcoded
  `"default"` to take a profile name. Add an enumerate endpoint (`GET /api/auth-profiles`) and
  a **switcher + create/rename/delete** on the web Connectors page. Mirror in the CLI:
  `palugada config auth list | add <name> | use <name> | rm <name>`. Reuse the existing
  masked-read + blank=keep semantics so switching profiles is safe. The existing
  `auth_profile_secrets(name)` helper already returns masked secrets for any named profile —
  it's the seam to build on.
- **Acceptance criteria.**
  1. Create `client-a` and `client-b` in the web UI; both round-trip to `secrets.yaml` with
     **no hand-editing**.
  2. Switching the profile selector reloads *that* profile's masked tokens.
  3. `palugada config auth list` enumerates every profile.
  4. Saving a connector under one profile **never** mutates another profile's tokens.
- **Touches.** `src/credentials.rs` (parameterize off `"default"`; add enumerate),
  `src/web.rs` (`/api/auth-profiles` + profile-scoped routes), `src/web/app.js` (switcher/CRUD,
  replacing the "multi-profile soon" tag), `src/main.rs` (`config auth` subcommands).

### F2 — Per-section connector picker / onboarding wizard  ·  HIGH

- **Problem.** `init` writes empty `base_url`s and prints a static 3-step "Next:" TODO (T1, T4).
  There is no "pick a provider for each of the six sections → enter URL/token → verify" flow.
- **Solution.** A guided flow (web-first; optional `palugada init --wizard` for the terminal)
  that, after scaffold, walks the six capability slots: choose provider (from the supported
  list), enter `base_url`/`repo` + token, **verify before save** (reuse `project_verify`),
  with skip allowed. It targets a chosen or newly created auth-profile (depends on F1) and
  ends with a project-wide verify sweep (F5).
- **Acceptance criteria.**
  1. From a fresh repo, complete onboarding **entirely in the wizard** with zero file edits.
  2. Each connector can be verified inline before commit; a failing verify blocks save but
     allows skip.
  3. The final screen shows green/red per slot.
  4. A skipped slot is left **empty**, never a broken partial provider.
- **Touches.** `src/scaffold.rs` (wizard hook after `generate`), `src/web/app.js`
  (`generateForm` → multi-step), `src/web.rs` (`init_op` + onboarding endpoints), `src/main.rs`
  (`init --wizard`).

### F3 — Graceful "no profile yet" path (+ fix the `web-react` bug)  ·  HIGH

- **Problem.** 🐞 `detect_profile()` returns `"web-react"` for any `package.json`, but that
  profile **does not exist on disk** — so `q`/`for`/`brief`/`index` fail for the most common
  freelance stack (T3). Onboarding a genuinely new stack also dead-ends into manual
  `profile new`/`extends`/import.
- **Solution.**
  (a) **Fix detect:** only bind a profile that actually resolves; otherwise fall back to a real
      default *or* an explicit "no profile bound" state with a clear message.
  (b) **Onboarding branch:** when no matching profile exists, offer "start from a minimal/base
      profile" or "extend an existing one" with a one-click scaffold (reuse `profile new` +
      `extends:` live inheritance), plus a pointer to web markdown import. Profile-free steps
      (`search`, `diff.scan`-based review) still work meanwhile.
- **Acceptance criteria.**
  1. `init` in a React/JS repo never binds a non-existent profile; downstream commands either
     work or give a clear "no profile bound, run X" message (no opaque file-not-found).
  2. The wizard offers a "no profile yet → scaffold a starter / extend a base" choice.
  3. After choosing, `palugada brief feature` runs (even on stub conventions).
  4. `palugada profile validate` passes on the scaffolded profile.
- **Touches.** `src/scaffold.rs` (`detect_profile` 144–154; no-profile branch), `src/main.rs`
  (clear messaging when no profile is bound).

### F4 — "Author once, install everywhere" pillar  ·  MEDIUM

- **Problem.** The authoring surface (web markdown import, flow editor, `convention add` /
  `recipe add`, overlays) is mostly shipped, but the connective tissue is missing: nothing
  links "I edited a profile" → "re-sync skills into the projects bound to it" (T5), and
  import → validate → use is not one guided path.
- **Solution.** Frame the existing authoring surface as a single pillar and add the glue: after
  editing a profile, the console lists the projects bound to it and offers a one-click
  `skills sync`; make markdown import → `profile validate` → `profile use` a single guided
  flow. UX glue over shipped primitives — **no new file-format surface**.
- **Acceptance criteria.**
  1. After editing a profile, the UI lists bound projects and offers one-click re-sync.
  2. Markdown import → validate → use is a single guided path.
  3. No new on-disk format introduced (reuses import/flow/sync primitives).
- **Touches.** `src/web/app.js`, `src/web.rs` (a "projects bound to profile X" query), reuse
  `skills sync`.

### F5 — Verify-all readiness signal  ·  MEDIUM/LOW

- **Problem.** `doctor` already does a project-wide connector + tool sweep, but it isn't part
  of onboarding, so the user never gets a single "you're ready" signal.
- **Solution.** Surface `doctor`'s sweep as the wizard's final step and as a "Verify all"
  button on the project Connectors page. Pure reuse — no new verify code.
- **Acceptance criteria.**
  1. One action verifies all wired slots and reports per-slot status.
  2. Reuses existing `verify_capability` / `doctor` logic (no new verify code path).
- **Touches.** `src/web/app.js`, `src/web.rs`, reuse `cmd_doctor`.

---

## 8. CLI & web surface changes

Legend: ✓ shipped · `+` new · `~` changed.

| Surface | Status | Behavior |
|---|---|---|
| `palugada init [--repo --name --profile --auth --agents --force]` | ✓ | offline scaffold + register + agent files |
| `palugada init --wizard` | `+` | interactive per-section connector picker (F2) |
| `palugada config auth list` | `+` | enumerate auth-profiles (F1) |
| `palugada config auth add <name>` / `rm <name>` / `use <name>` | `+` | manage / select credential sets (F1) |
| `palugada skills sync [--agents --force]` | ✓ | regenerate agent files (expand-not-overwrite) |
| `palugada doctor` | ✓ | project-wide connector + tool sweep (reused by F5) |
| `GET /api/auth-profiles` | `+` | list credential sets (F1) |
| `GET/POST /api/connectors[/{cap}]` (profile-scoped) | `~` | parameterize off `default` → selected profile (F1) |
| Connectors page: auth-profile switcher + CRUD | `+` | replaces "multi-profile soon" (F1) |
| Onboarding flow (init → pick connectors → verify-all) | `+` | guided new-project journey (F2, F5) |
| "No profile yet" branch (scaffold / extend) | `+` | new-stack path (F3) |
| Profile detail: "bound projects → re-sync" | `+` | author-once propagation (F4) |

---

## 9. The onboarding journey (worked walkthrough)

The emotional center of the PRD — the future UX, end to end.

**Client A — React app, docs in Notion, issues in Jira (a new stack + a new client):**

```
$ cd ~/clients/acme-web
$ palugada init --wizard

  Detected package.json — no matching profile yet.
    > [s] scaffold a starter profile   [e] extend a base   [n] none for now      > s
    Scaffolded profile 'acme-web' (starter). Edit knowledge later in `palugada web`.

  Credential set (auth-profile):
    > [existing: default]  [+ new]                                               > + new
    Name: > client-acme

  Wire connectors (Enter to skip a section):
    issue_tracker [jira/github_issues]: jira
      base_url: https://acme.atlassian.net/rest/api/2   token: ****   ✓ verified 200
    wiki [confluence/notion]: notion
      token: ****   ✓ verified
    design/ci/git_host/chat … (skipped / wired) …

  Verify all:  issue_tracker ✓  wiki ✓  git_host ✓  ci –  design –  chat –
  Wrote acme-web/.palugada/config.yaml, agent skills, secrets under 'client-acme'.
  Ready: try  palugada brief feature <ticket>
```

**Client B — Android app, docs in Confluence, issues in GitHub (reusing a known stack):**

```
$ cd ~/clients/globex-android
$ palugada init --wizard

  Detected gradle — profile: android-mvvm
  Credential set: > [existing: client-globex]   (already has Confluence + GitHub tokens)
  Wire connectors: inherited from credential set; only `repo` differs → globex/android-app
  Verify all: ✓ all green
  Ready.
```

The difference between the two: **a new client/stack** is a few guided choices; a **returning
client** reuses its credential set and just confirms. No file ever opened by hand.

---

## 10. Rollout phases

Each phase ships independently and is dogfoodable.

| Phase | Scope | Acceptance gate |
|---|---|---|
| **P1 — Auth-profile core** | `GET /api/auth-profiles` enumerate; CLI `config auth list/add/use/rm`; parameterize credentials off `"default"`. | Two credential sets created via CLI; saving one never touches the other. |
| **P2 — Web switcher/CRUD** | Connectors page auth-profile switcher + create/rename/delete; reload masked tokens on switch. | "multi-profile soon" gone; create client-a/client-b in the browser, zero hand-edits. |
| **P3 — Onboarding wizard** | `init --wizard` + web multi-step: per-section picker → verify-before-save → verify-all sweep (F5). | Fresh repo onboarded end-to-end with 0 file edits; per-slot green/red shown. |
| **P4 — No-profile path + web-react fix** | Fix `detect_profile`; add scaffold/extend branch + clear messaging. | React repo never binds a missing profile; `brief feature` runs after a one-click scaffold. |
| **P5 — Author-once glue (F4)** | "bound projects → re-sync"; import→validate→use single path. | Editing a profile offers one-click sync into bound projects. |

---

## 11. Dogfooding & validation plan

Validate palugada using **this very PRD** once it's in Notion. (Caveat first: there is **no
`palugada stats` command**; `brief` prints a `(~N tokens)` footer via an estimate, and there
is **no wiki-corpus ingest** — fetching the PRD from Notion is a *live* `wiki page` call.)

1. **Upload** this PRD to a Notion page; note the page id.
2. **Wire Notion** as the `wiki` connector: provider `notion`, `base_url`
   `https://api.notion.com`, token under a chosen auth-profile — **use a non-`default` profile
   to also exercise F1**. Verify it via the Connectors card (`notion.rs::verify`).
3. **Fetch:** `palugada wiki page <notion_page_id>` → confirm the DocSource renders the PRD
   (live fetch; not saved to a corpus).
4. **Budget behavior:** run `palugada brief feature <target>`; read the trailing
   `(~N tokens)` footer. Re-run with `--budget 800` vs `--budget 4000` to show packs
   truncate/omit; use `--json` to inspect which packs survived.
5. **Token caveat:** `est_tokens = len/4 + 8` is an estimate, not a real tokenizer count — so
   the success metrics are **directional**, not exact.
6. **The money shot:** point a `brief` target at a ref that resolves to this PRD and show the
   assembled pack contains the relevant PRD section **under budget** — palugada reading its own
   spec, cheaply. That is the end-to-end proof.

**Outcome (first run, 2026-06-28).** Running this plan against the uploaded PRD paid off
immediately: the Notion DocSource was *lossy* — it truncated at ~§10 of 14 (no pagination) and
dropped every table and code block. Fixed the same session (`fix/notion-docsource-fidelity`,
merged to `main`): paginate `/children`, render tables + code, recurse nested blocks, re-emit
Markdown structure. The PRD now round-trips from Notion in full (§1–§14, tables + code). The
dogfooding loop worked — testing the tool on its own spec surfaced and closed a real connector
gap.

---

## 12. Risks & dependencies

| Risk / dependency | Mitigation |
|---|---|
| **F2/F5 depend on F1.** The wizard and verify-all must target a credential set. | Sequence per §10: P1/P2 (auth-profiles) before P3 (wizard). |
| **🐞 `web-react` auto-detect binds a non-existent profile** → onboarding broken for JS/React today. | F3/P4 is the fix; flagged as both a current-state caveat and the first P4 deliverable. |
| **No wiki-corpus ingest** — dogfooding "fetch the PRD" is a live `wiki page` call, not a searchable saved doc. | Scope the test to live fetch; a "corpus ingest from wiki" item is a future option, not a dependency. |
| **`est_tokens` is an estimate**, not a tokenizer. | Treat token metrics as directional; consider `palugada stats` as an *optional* later item, never a dependency. |
| **More secrets at rest** with many credential sets. | `secrets.yaml` stays chmod 0600; web console is loopback + host-guarded and masks on read; PAT-only in v1 (no OAuth). |
| **Web-only vs CLI parity** for the wizard. | Open decision (§13); freelancers in a terminal may want `init --wizard` parity — real scope, decide before P3. |

---

## 13. Open decisions

| # | Decision | Options | Leaning |
|---|---|---|---|
| O1 | Where the onboarding wizard lives | web-only · CLI-only · both | Web-first; add `init --wizard` if terminal demand is real |
| O2 | The broken `web-react` auto-detect | delete the detect branch · build the profile · neutral fallback | Neutral fallback + clear message now; build the profile later |
| O3 | Auth secrets storage | 0600 file (today) · OS keychain · `env:` refs | Keep 0600 default; keychain/`env:` deferred (unified PRD §16 O4) |
| O4 | Auth-profile creation ergonomics | from scratch · "clone from existing" | Offer clone-from-existing (a new client often mirrors another's shape) |
| O5 | The `active` project's role | keep as today · de-emphasize (cwd-first) · warn on fallback | De-emphasize: cwd inference already resolves the project inside a repo, so `active` is only a last-resort default — and it is easy to misread as a lock. Consider a warning when a command falls back to `active`. |

---

## 14. Appendices

### Appendix A — Glossary

- **Auth-profile** — a named set of credentials (tokens, emails, webhook) in
  `~/.palugada/secrets.yaml`, referenced by name from a project config. The reusable "client
  credential set."
- **Capability slot** — one of the six connector roles (issue_tracker, wiki, design, ci,
  git_host, chat), each with a provider + base_url (+ repo for projects).
- **Profile** — a bundle of stack knowledge: conventions, recipes, flows, fact families,
  review_map, exec verbs. May `extends:` another profile.
- **Convention / Recipe / Flow / Skill** — see §5 (kitchen → plate).

### Appendix B — Current-state file map (for implementers)

| Area | Module(s) |
|---|---|
| Config / secrets / merge | `src/config.rs` (`GlobalConfig`, `ProjectConfig`, `Integrations`, `AuthProfile`, `merge_integrations`, `resolve_project`) |
| Connector view/apply/verify | `src/credentials.rs` (`global_view`/`apply_global`/`global_verify`, `verify_kind`, `auth_profile_secrets`) |
| Connector traits + providers | `src/clients/` (`mod.rs` factories; `jira.rs`, `github_issues.rs`, `confluence.rs`, `notion.rs`, `github.rs`, `gitlab.rs`, `figma.rs`, `jenkins.rs`, `github_actions.rs`, `gitlab_ci.rs`, `slack.rs`) |
| Init / scaffold / skills | `src/scaffold.rs` (`generate`, `skill_files`, `detect_profile`) |
| Web console | `src/web.rs` (routes + `init_op`), `src/web/app.js` / `index.html` / `style.css` |
| Context packs | `src/brief.rs` (flow execution, budget priorities) |
| Profiles / inheritance / overlay | `src/profile.rs`, `src/inherit.rs`, `src/effective.rs`, `knowledge/profiles/<id>/` |

---

*End of PRD. This document is the adoption-surface companion to
[`PRD-unified-palugada.md`](../PRD-unified-palugada.md); it snapshots v0.2.1 and specifies the
onboarding/connector roadmap (F1–F5). Implementation of those features lands as separate
feature branches.*
