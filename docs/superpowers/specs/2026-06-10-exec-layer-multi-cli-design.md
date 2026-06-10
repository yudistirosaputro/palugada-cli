# Design — palugada v2: universal substrate with an execution toolbelt

> **Status:** Approved design · **Date:** 2026-06-10 · **Author:** brainstormed with Claude Code
> **Extends:** `PRD-unified-palugada.md` (adds the exec layer, plan/test flows, and foundation fixes; the PRD will gain matching sections)

## 0. Goal

Make palugada the substrate for a complete AI-agent loop — **plan → code → execute → test → bugfix → review** — usable in **any project**, from **any AI CLI** (Claude Code, Codex, Gemini CLI, Cursor), with **android-cli** (`android` from developer.android.com/tools/agents/android-cli) as the execution backend on Android stacks.

Decisions locked during brainstorming:

1. **Role:** palugada stays the "substrate under the agent" (context + execution toolbelt). It does NOT orchestrate the AI loop itself; the AI CLI is the brain and shells out.
2. **AI CLI targets:** the current four (Claude, Codex/AGENTS.md, Gemini, Cursor) — improved content, no new targets.
3. **android-cli depth:** exec + verify wrappers only (build/run/test/ui-dump/screenshot/doctor), not a full passthrough of the `android` surface.
4. **Exec implementation:** profile-declared command templates (data), not Rust traits per stack.
5. **Session scope:** spec + PRD update + full implementation with tests.

## 1. Architecture

```
 AI CLI (Claude / Codex / Gemini / Cursor — any brain)
    │ reads CLAUDE.md / AGENTS.md / GEMINI.md / .cursor rules  (scaffolded, accurate)
    ▼
 palugada (one cold-start binary)
 ├─ KNOW    q / for / s / index / symbol / brief    ← exists; fixed + new flows
 ├─ CONNECT issue / wiki / design / ci / git        ← exists; + Atlassian Cloud auth
 └─ EXEC    exec <verb> / exec --list / doctor      ← NEW; profile-declared commands
```

Loop mapping (what the scaffolded agent files teach):

| Phase | Command |
|---|---|
| plan | `palugada brief plan <ticket-or-goal>` |
| code | the agent edits, guided by `q`/`for`/`symbol` |
| execute | `palugada exec build` · `palugada exec run apk=…` |
| test | `palugada exec test` · `palugada exec ui-dump` (Android UI checks) |
| bugfix | on failure: `palugada brief bugfix <file>` → fix → re-run exec |
| review | `palugada brief review --diff <ref>` |

## 2. Exec layer (new module `src/exec.rs`)

### 2.1 Configuration shape

`profile.yaml` gains an `exec:` map; `<repo>/.palugada/config.yaml` may override/extend it (project wins per-verb). Each verb is either a string (shorthand for `{ cmd: "…" }`) or a table:

```yaml
# knowledge/profiles/android-mvvm/profile.yaml
exec:
  build:      { cmd: "./gradlew assembleDebug", timeout_secs: 600 }
  test:       { cmd: "./gradlew testDebugUnitTest", timeout_secs: 900 }
  run:        { cmd: "android run --apks={apk}" }
  ui-dump:    { cmd: "android layout --pretty" }
  screenshot: { cmd: "android screen capture --output={out}" }
  doctor:     { cmd: ["android -V", "adb version", "./gradlew -v"] }
```

```yaml
# knowledge/profiles/web-react/profile.yaml
exec:
  build:  "npm run build"
  test:   "npm test -- --run"
  run:    "npm run dev"
  doctor: { cmd: ["node -v", "npm -v"] }
```

- `cmd`: string or list of strings (list = run sequentially, stop on first failure).
- `timeout_secs` (optional, default 600): kill + non-zero exit on expiry.
- A repo with no profile coverage can define `exec:` purely in its project config.

### 2.2 CLI surface

- `palugada exec <verb> [k=v …] [--json]`
  - Resolves verb from merged project+profile maps; unknown verb → `error: no exec verb 'x' — available: build, test, … (define it under exec: in .palugada/config.yaml)`.
  - `{placeholder}` substitution from `k=v` args. Missing placeholder → error listing every required key. `k=v` keys must be `[a-z0-9_-]+`.
  - Runs via `sh -c` with cwd = repo root, streaming stdout/stderr live.
  - **Exits with the child's exit code** (agents branch on it).
  - `--json` (machine-first): `{"verb","command","exit_code","duration_ms","tail"}` where `tail` = last 40 lines of combined output. JSON goes to stdout; live streaming is suppressed in JSON mode.
- `palugada exec --list [--json]` — merged verbs with their resolved commands and source (`[profile]`/`[project]`).
- `palugada doctor [--json]` — runs the merged `doctor` verb's checks (each command: pass/fail + first output line) and then the connector `config verify` rollup. One readiness report; non-zero exit if any check fails. This is how an agent confirms android-cli/gradle/node are present before relying on them.

### 2.3 Security posture

Exec templates come from the bundled profile (repo-controlled) and the project's own committed config — the same trust level as a Makefile. No shell-escaping of `k=v` values is attempted beyond key validation; values are substituted verbatim (documented; equivalent trust to running `make`).

## 3. Flows and brief

### 3.1 New flows (six total)

```yaml
flows:
  plan:     [issue.context, convention(architecture), recipe(feature), module.info]
  bugfix:   [code.recent, symbol.find, convention(errorhandling), convention(testing)]
  feature:  [issue.context, recipe(feature), symbol.find, convention(architecture)]
  refactor: [symbol.find, convention(architecture), convention(style), recipe(refactor)]
  review:   [diff.scan, convention(by-file-kind)]
  test:     [convention(testing), symbol.find, exec.hints]
```

### 3.2 New brief step kinds (join connectors ↔ knowledge for the first time)

- `issue.context` — if the target matches a ticket pattern (`[A-Z][A-Z0-9]+-\d+`), fetch it via the configured `IssueTracker` and pack summary+description excerpt; otherwise emit a skip note. Network errors degrade to a note, never fail the pack.
- `diff.scan` — `git diff --name-only <ref>` (default `HEAD`); map changed file extensions/kinds to relevant convention topics via the profile's fact families; pack the changed-file list + matched convention sections. Makes `brief review` real. `brief` gains a `--diff <ref>` flag (review flow's target).
- `exec.hints` — embed the repo's merged exec verb list (`build → ./gradlew assembleDebug`, …) so the pack tells the agent exactly how to build/test *this* repo.

### 3.3 Budget fixes (confirmed review findings)

- First pack no longer bypasses the budget (truncate to fit instead).
- `--json` honors the budget.
- Cost computed on exactly what is printed (title + trimmed content + separators); omission notices counted.

### 3.4 Knowledge content

- Author for `android-mvvm`: `conventions/{errorhandling,testing,style}.md` + `recipes/refactor.md` (from current official Android guidance — Hilt/Coroutines/Flow, JUnit/Turbine/MockK; no company specifics), updating `_index.json`s.
- New **`web-react`** profile: profile.yaml (flows + exec + fact families), `conventions/{architecture,testing}.md`, `recipes/feature.md`, regex extractors (component/hook/route).
- New **`generic`** profile: defines all six flow names using only stack-agnostic steps (`code.recent`, `symbol.find`, `diff.scan`, `issue.context`, `exec.hints`), no stack assumptions, empty exec map (the project config supplies verbs), generic extractors (function/class via common-language regexes).

## 4. Agent files (scaffold)

- **Managed sections:** all root agent files are written between `<!-- palugada:begin -->` / `<!-- palugada:end -->` markers. Existing files: append the managed block; re-run: replace only the block. Never clobber user content; `--force` only forces replacement of the managed block. Skill files / cursor rules (palugada-owned paths) are written whole, but namespaced.
- **`palugada skills sync`:** new command — regenerates only the agent files from the current binding (no config/registration changes).
- **Data-driven targets:** the four targets move from a hardcoded `match` into a const target table `(name, root_file, wrapper, skills_dir?)`; adding a fifth CLI later = one row.
- **Rewritten guide content:** the guide documents only commands that exist — both previously-phantom commands (`brief review --diff`, `skills sync`) become real in this design, and the stale "being rolled out" disclaimer is removed. It documents the six flows, exec verbs, `doctor`, and the loop recipe from §1.
- **Claude skills:** namespaced `palugada-{plan,bugfix,feature,refactor,review,test}` (avoids collision with built-in `/review`); same per-flow guidance embedded as sections in AGENTS.md / GEMINI.md / cursor rules so all four CLIs behave alike.

## 5. Foundation fixes (from the 53-agent review; all confirmed findings)

| Fix | File(s) |
|---|---|
| cwd-first project resolution: if cwd is inside a registered repo, prefer it over `projects.active`; unknown `--project` → hard error naming known projects; expand `~` in repo paths | `src/main.rs:348-372` |
| Profile detection only emits profiles that exist on disk; unknown stack → `generic` + printed warning; `resolve_profile` stops swallowing project-config parse errors | `src/scaffold.rs:120-130`, `src/main.rs:304-328` |
| Atlassian Cloud auth: optional `jira_email`/`wiki_email` in AuthProfile → `Basic base64(email:token)` when present, else Bearer | `src/config.rs`, `src/clients/{jira,confluence}.rs` |
| `project add`/init: canonicalize + validate path exists; warn on name collision with a different repo_path | `src/main.rs:481-498`, `src/scaffold.rs:90-93` |
| Extractor family id sanitized (`[a-z0-9_-]+`) — path-traversal write fix | `src/indexer.rs:147-150` |
| Secrets created atomically with mode 0600 (`OpenOptionsExt`); error (not `.`) when `$HOME` unset; `config init` message claims 0600 only for secrets | `src/config.rs:154-163,266-268`, `src/main.rs:411` |
| `--insecure` prints a one-line warning banner to stderr | `src/http.rs`, `src/main.rs` |
| Unknown `auth_profile` → error naming it + known names (no empty-token fallback) | `src/config.rs:254-258` |
| URL-encode user-supplied identifiers in connector paths; include URL in HTTP status errors; overall request timeout (90s) | `src/clients/*`, `src/http.rs` |
| Index dir cleared at start of `index` run (no stale per-kind files) | `src/indexer.rs:138-150` |
| `mask_secret` char-safe (no byte-slicing panic on UTF-8); mask shows no leading chars | `src/config.rs:279-287` |
| Section splitter ignores `## ` inside fenced code blocks | `src/knowledge.rs:336-355` |
| `expand_home` handles bare `~` | `src/config.rs:270-276` |
| `project remove <name>` subcommand | `src/main.rs` |
| `profile list` + `profile validate <id>` (parse profile.yaml + extractors, compile regexes, check every flow step's convention/recipe exists, check exec shape) | new `src/profile_cmd.rs` or in `main.rs` |

Deliberately NOT fixed now (deferred, noted in PRD): connector write ops (PR create, CI trigger/log), personal PRD corpus, conventions-overlay merge, tree-sitter extractors, `stats` telemetry, chat/notify, serde_yaml migration.

## 6. Error handling

- Exec failures are data: child's non-zero exit propagates as palugada's exit code; `--json` is still emitted on failure. Palugada-level errors (unknown verb, missing placeholder, missing tool in doctor) use the normal `error:` path with actionable messages.
- `brief` connector steps degrade to in-pack notes on network failure — a pack is always produced.
- `doctor` exits non-zero if any check fails, listing each check as pass/fail.

## 7. Testing (repo currently has zero tests)

- **Unit:** placeholder substitution (missing/extra/multiple), exec config merge precedence, budget accounting, managed-section insert/replace/idempotency, profile detection matrix, auth-header selection (Bearer vs Basic), family-id sanitization, `expand_home`, `mask_secret` UTF-8.
- **Integration (`tests/`):** run the built binary against a tempdir fixture repo + fixture profile (exec verbs = `echo`/`false`) with `HOME` pointed at a tempdir: `init` → `index` → `brief` (each flow) → `exec` (success, failure exit-code propagation, JSON shape, timeout) → `skills sync` idempotency → `doctor`.
- **Golden files:** scaffolded CLAUDE.md/AGENTS.md/GEMINI.md/cursor rule outputs.
- Completion gate: `cargo test` green + `cargo build --release` clean + a manual smoke of `palugada doctor` in this repo.

## 8. PRD update

`PRD-unified-palugada.md` gains: §4.6 (exec layer — the third pillar), §7.6 (exec/doctor command table), §9.6 (the end-to-end loop story + android-cli appendix), updated §7.2 (skills sync, project remove, profile list/validate now real), updated flows in §4.3 (plan/test), and a new migration phase for the exec layer; deferred items moved to §13 explicitly.
