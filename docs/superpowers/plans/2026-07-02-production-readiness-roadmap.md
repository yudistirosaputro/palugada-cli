# palugada Production-Readiness Roadmap

> **Scope note (per writing-plans scope check):** this roadmap spans multiple
> independent subsystems. It is NOT a single implementation plan — each work
> package (WP) below becomes its own branch + spec + implementation plan via
> the full superpowers flow at execution time. This document fixes the
> sequence, the acceptance criteria, and the release train.

**Goal:** take palugada v0.2.3 from "excellent solo beta" to a product the
team can adopt as the AI-agent context layer for daily development.

**Input:** 5-agent audit, 2026-07-02 (architecture, correctness, security,
test/CI/release, product). All findings verified against source at `ff83566`;
correctness items reproduced empirically with the built binary.

**Verdict at audit time:** core idea is sound and differentiated
(skills-as-references + token-budgeted briefs; measured ~363-token bugfix
pack). Hygiene is strong (0 prod panics, near-clean clippy, 181 green tests,
3-OS CI, reproducible publishing docs). Blockers are: one remotely-triggerable
credential-exfiltration path in the web console, inheritance correctness bugs,
a first-run experience that breaks on every non-Android stack, an index that
silently lies about staleness, and zero tests on every I/O boundary.

---

## Definition of "production ready" (acceptance criteria for v1.0)

1. **Security:** no known path for a web page / cloned repo / tampered
   download to exfiltrate tokens or execute code without explicit consent.
2. **First run:** fresh `init` → `index` → `doctor` succeeds honestly on
   android-mvvm, rust-cli, flutter-bloc repos — and fails *clearly* (not
   silently or with `os error 2`) on unsupported stacks.
3. **Honesty:** stale index is surfaced; degraded briefs are detectable by
   agents (flag + exit code); no bundled profile that renders empty.
4. **Inheritance:** `extends` behaves identically across every read path
   (`q`/`for`/`s`/`brief`/`exec`/`project rules`/web).
5. **Contract tests:** e2e tests of the built binary cover the output shape +
   exit codes that generated skills rely on; web API and connectors have
   request-level tests; all green on 3-OS CI.
6. **Release train:** tag↔version guard, per-target smoke, CHANGELOG.

---

## Findings register (condensed; full agent reports in session transcript)

### Security (web console + supply chain)
- **S1 CRITICAL — CSRF → live token exfiltration.** `web.rs:185-218` gates
  only on Host (defeats DNS-rebinding, not CSRF). Verify endpoints
  (`credentials.rs:268-316`, `:388-403`) accept a posted `base_url` and fall
  back to the *saved* token when the posted one is blank → any web page open
  while `palugada web` runs can POST (simple request, no preflight) and make
  palugada send the real `git_token` to an attacker host. State-changing
  writes are equally forgeable.
- **S2 MEDIUM — `install.sh` verifies nothing.** `.sha256` sidecars are
  published (`release.yml:69`) but never checked (`install.sh:44-58`);
  pinned to `releases/latest`.
- **S3 MEDIUM — repo-committed `exec:` verbs = arbitrary code execution on
  clone.** `exec.rs:26-51,171-193` runs `.palugada/config.yaml` verbs via
  `sh -c`; AI agents auto-run `palugada exec build` — highest-likelihood
  code-exec path in practice.
- **S4 MEDIUM — `--insecure` disables TLS verification for ALL hosts**
  (`http.rs:12-37`); **Slack webhook (the secret IS the URL) leaks into
  error strings** (`http.rs:84-92`, `slack.rs:31`).
- Verified-good: secrets 0600 + atomic write, consistent masking, header-only
  auth, XSS-safe markdown renderer, loopback bind + Host guard.

### Correctness (empirically reproduced)
- **C1 CRITICAL — `effective_rules` ignores `extends`** (`effective.rs:169`,
  `:142-147`): child profiles show zero conventions + false review_map
  warnings in `project rules` and the web Effective Rules card.
- **M1** family `path_contains` matched against ABSOLUTE path
  (`indexer.rs:451`) → phantom facts; inconsistent with `brief`'s
  git-relative classify.
- **M2** repo root named `build`/`target` → silently empty index
  (`indexer.rs:435-437,607-612`).
- **M3** `.palugada/index` not hard-excluded from the walk → self-indexing
  feedback loop when a profile omits it (`indexer.rs:435-485`).
- **M4** `brief`/`exec` don't resolve `flows`/`review_map`/`exec` across
  `extends` (`brief.rs:205-218`, `exec.rs:32-43`) — copy-seeded children
  silently drift from parent updates.
- **M5** `read_extends` swallows parse errors (`inherit.rs:15-25`) —
  inheritance silently off, `profile validate` stays green.
- **M6** non-UTF8 byte in exec output wipes the captured tail
  (`exec.rs:196-209`) — diagnostics lost exactly when needed.
- **M7** Rust/Dart symbols indexed without signature/scope/method-kind —
  enrichment tables are Kotlin-only (`indexer.rs:254-299`); 2 of 3 supported
  languages degraded, including this repo's own profile.
- **M8 PLAUSIBLE** — `exec` hardcodes `sh -c` (`exec.rs:181`); likely broken
  on native Windows (needs verification on the CI Windows runner).
- Plus 16 minors (register: `q topic.0`, preamble drop, empty-metadata
  clobber, `symbols` family-name collision, regex-group validation,
  early-side-effect substitution, per-command timeout, `--budget` ignored
  under `--json`, misleading diff.scan degradation, dead
  `stale_warning_days`, `expand_home` gaps, non-atomic global-config/_index
  writes, `yaml_scalar` under-quoting, symlinked sources skipped,
  front-matter hrule false-positive).

### Product / first-run
- **P1** `detect_profile` maps `package.json` → nonexistent `web-react`
  (`scaffold.rs:144-154`); no Cargo.toml/pubspec detection — palugada cannot
  detect its own stack; JS repo init succeeds then every command errors with
  raw `os error 2`.
- **P2** `kmp` profile is an empty test artifact (title "kmp-test", zero
  conventions/recipes, flows referencing nothing, exit-0 empty briefs).
  `r8-analyzer.md` has empty description + 6 dead `references/` links.
- **P3** index staleness is silent — wrong line numbers served with no
  warning; `indexed_at`/`git_sha` recorded but never checked; contradicts
  the "always-current" headline.
- **P4** `doctor` FAILs (exit 1) on unconfigured optional connectors —
  including in palugada's own repo. Needs PASS/SKIP/FAIL.
- **P5** `brief <flow> <file>` emits a dead symbol step on its own
  documented input (name-matches the raw path); degraded packs exit 0 so
  agents can't branch.
- Smaller: `palugada-git` skill template leaks internal ticket example
  "UATP-1602"; Quick start front-loads secrets before offline value; no
  shell completions / man pages; `--json` missing on `symbol`/`fact`;
  android-mvvm/flutter-bloc content ~70% textbook (moat = team-specific
  deltas, still to be poured in).

### Architecture (maintainability 6.5/10)
- **A1** stringly-typed errors (`Result<_, String>` ×199) → web serves 500
  for not-found; brief smuggles errors as content strings.
- **A2** `profile.yaml` parsed by 8 different partial structs across 8 files
  — no owning `ProfileManifest` type.
- **A3** `knowledge.rs` ⇄ `inherit.rs` circular dependency; knowledge.rs
  holds 3 layers (raw IO, authoring, presentation).
- **A4** 3 divergent fence-aware section parsers + duplicated front-matter
  parsing (+ re-implemented in app.js); token estimate copy-pasted ×4.
- **A5** 4 uncoordinated YAML emitters; `render_merged` writes unescaped
  `title:` (`inherit.rs:240`) that `parse_doc_front_matter` can choke on.
- **A6** `app.js` 1,712-line monolith, 420-line `renderCreate`, no
  URL/hash routing (refresh dumps to Overview), mutable view globals.
- **A7** main.rs copy-paste (project-resolution preamble ×9, masked-print
  ×2, verify dispatch ×3); `resolve_profile` stuck in the binary root.
- **A8** web server single-threaded blocking — one slow connector verify
  (≤90s) freezes the console.

### Test/CI/release
- **T1** 0 e2e tests of the built binary; main.rs (1,412 lines) 0 tests —
  the agent-facing contract is unguarded. No `tests/` dir, no `assert_cmd`.
- **T2** web.rs: 4 pure-fn tests, none touch HTTP; composer endpoints
  (newest, write-capable code) untested.
- **T3** HTTP clients: jira/gitlab/github/confluence/figma 0 tests each; no
  mock-HTTP dep in tree.
- **T4** CI lacks clippy gate (2 warnings live) and any fmt policy;
  release.yml has no tag↔Cargo.toml guard; cross-compiled
  `x86_64-apple-darwin` binary never executed before shipping; no
  CHANGELOG; no Linux aarch64.
- Good: 3-OS CI with `--locked` + real smoke test; `install.sh` otherwise
  robust; `docs/PUBLISHING.md` reproducible; core library well-tested
  (inherit 22, config 19, credentials/indexer/knowledge 18 each).

---

## Release train

| Release | Content | Phase |
|---|---|---|
| **0.2.4** (security patch, ASAP) | S1-S4 | Fase 0 |
| **0.3.0** | correctness + first-run trust (C1, M1-M6, P1-P5) | Fase 1 |
| **0.3.x** | test harness + CI/release hardening (T1-T4) | Fase 2 |
| **0.4.0** | architecture debt (A1-A8, M7) | Fase 3 |
| **1.0.0** | acceptance criteria above all green + team pilot feedback | Fase 4 |

---

## Fase 0 — Security hotfix → v0.2.4 (est. 2-4 hari)

### WP0.1 Web console session hardening (fixes S1)
- Mint a random per-session token at `web::run`, inject into `index.html`,
  require it (header `X-Palugada-Token`) on every `/api/*` request; reject
  missing/wrong with 403.
- Reject requests whose `Origin`/`Sec-Fetch-Site` is present and not
  same-origin.
- Never send a stored token to a `base_url` different from the one saved for
  that capability: `global_verify`/`project_verify` must verify against the
  SAVED wiring unless the request carries its own explicit token.
- Tests: forged cross-origin POST (no token) → 403; verify with mismatched
  base_url + blank token → rejected; happy path with token → 200.

### WP0.2 Installer integrity (fixes S2)
- `install.sh`: download the matching `.sha256`, `shasum -a 256 -c` (or
  `sha256sum`) before extract; abort loudly on mismatch.
- Support `PALUGADA_VERSION=vX.Y.Z` pin (default stays latest).

### WP0.3 Gate repo-committed exec verbs (fixes S3)
- Trust-on-first-use: first `exec` of verbs coming from a repo's
  `.palugada/config.yaml` prints the exact command(s) and asks for
  confirmation; decision cached per repo+verb-hash (global, not committed).
  `--yes` / env `PALUGADA_TRUST_REPO_EXEC=1` for CI. Profile-bundled verbs
  (shipped with the binary) stay ungated.
- Document the threat model in README + generated skills ("agent should not
  auto-confirm").

### WP0.4 TLS scope + webhook redaction (fixes S4)
- Replace global `--insecure` with `--insecure-host <host>` (repeatable);
  keep `--insecure` as deprecated alias emitting a warning, scoped to hosts
  in the project's configured base_urls only.
- Redact webhook URLs (scheme+host only) before they can enter any error
  string (`describe_ureq` gains a redact hook or slack.rs pre-wraps).
- Add `SECURITY.md` (reporting + threat model).

---

## Fase 1 — Correctness & first-run trust → v0.3.0 (est. ~1 minggu)

### WP1.1 Inheritance correctness (C1, M4, M5 + minors 2, 3)
- `effective_rules` uses `inherit::merged_conventions`; `review_map` merged
  across `resolve_chain` (parent-first, child overrides, per-family
  replace).
- `brief`/`exec` fold `flows`/`review_map`/`exec` across the chain the same
  way; `profile new --extends` stops copy-seeding what is now inherited.
- `read_extends` propagates read/parse errors; `profile validate` FAILs on
  unparseable/mistyped `extends`.
- Merged docs: nearest-non-empty preamble (like title/desc/tags); child
  empty metadata no longer clobbers parent in `merged_conventions`.

### WP1.2 Indexer honesty (M1, M2, M3, minors 4, 5, 11, 15)
- Family matching on repo-relative path; walk-root exempt from
  `ignore_dirs`; `<repo>/.palugada` unconditionally excluded.
- Reserve `symbols`/`manifest` family ids; validate regex families define
  `(?P<name>…)`; note skipped symlinked sources in output.
- Wire `stale_warning_days`: `symbol`/`fact`/`brief` compare manifest
  `git_sha`/`indexed_at` to the checkout and print one warning line
  ("index is N commits behind — run `palugada index`"). Fix the
  `Defaults::default()` 0-vs-7 inconsistency.

### WP1.3 First-run experience (P1, P4 + quick wins)
- `detect_profile`: `Cargo.toml`→rust-cli, `pubspec.yaml`→flutter-bloc,
  Gradle→android-mvvm; if the resolved profile doesn't exist on disk,
  hard-error listing `profile list` (no more ghost `web-react`, no silent
  android-mvvm fallback).
- `doctor` 3-state: PASS / SKIP (connector not configured) / FAIL
  (configured but broken); fresh `init`+`doctor` exits 0. Clean the empty
  base_url stubs from palugada's own committed `.palugada/config.yaml`.
- README Quick start reordered: `init` → `index` (offline value first),
  connectors/secrets after. Scrub "UATP-1602" from the git skill template.

### WP1.4 Bundled knowledge quality gate (P2)
- Decide: fill `kmp` with real content or unbundle until ready
  (recommendation: unbundle — empty stall damages trust).
- Fix `r8-analyzer.md` (description, tags, remove/inline the 6 dead
  `references/` links).
- CI gate: `palugada profile validate` must pass for every bundled profile
  on every PR (validate already exists — just run it in ci.yml).
- Either ship `bugfix`/`review` recipes or stop declaring them in flows.

### WP1.5 Brief/exec output honesty (P5, M6 + minors 6, 9, 10)
- Path-shaped brief target → list symbols defined IN that file (index has
  the data) instead of name-matching the path string.
- Degraded packs: `"degraded": true` in `--json` + distinct exit code (e.g.
  3) when steps produced no content — agents can branch.
- `--budget` respected under `--json`; substitute all commands up-front
  before running any (no side effects before a placeholder error);
  `read_to_end` + `from_utf8_lossy` for exec output tails.

---

## Fase 2 — Test harness & release hardening → v0.3.x (est. 1-2 minggu, dapat paralel dengan Fase 3)

### WP2.1 Binary contract tests (T1, settles M8)
- Add `assert_cmd` + `predicates` dev-deps; `tests/cli.rs` covering:
  `q`/`for`/`s` (hit + miss + exit codes), `symbol`/`fact` (with a fixture
  index), `brief` (pack shape, `--json` schema, degraded exit code),
  `exec` (verb run, timeout 124, missing placeholder), `init` (offline
  scaffold in tempdir), `doctor` (SKIP semantics). Runs in existing 3-OS CI
  — this empirically answers M8 (Windows `sh`); fix with `cmd /C` or
  document a Git-Bash requirement based on the result.

### WP2.2 Web API tests (T2)
- Spawn the server on an ephemeral port in-process; exercise: session-token
  enforcement (Fase 0 regression), composer save/clone, profile CRUD,
  connectors save/verify (mocked), 404 vs 500 mapping, Host guard.

### WP2.3 Connector tests with mocked HTTP (T3)
- Add `httpmock` dev-dep; per client: auth header shape, happy-path parse
  (fixture JSON), 401/404/500 error text, pagination (notion), encoding.

### WP2.4 CI & release hardening (T4)
- ci.yml: add `cargo clippy --all-targets -- -D warnings`.
- **fmt decision needed** (see Decisions): adopt rustfmt (one mechanical
  commit + `cargo fmt --check` gate) vs codify hand-format in
  CONTRIBUTING and skip the gate.
- release.yml: guard `tag == Cargo.toml version` (fail fast); smoke-run
  every binary the runner can execute (arm64 mac runner can run x86_64 via
  Rosetta: `arch -x86_64 ./palugada --help`); generate CHANGELOG (git-cliff
  from conventional commits — history already follows the convention);
  consider adding `aarch64-unknown-linux-gnu`.

---

## Fase 3 — Architecture debt → v0.4.0 (est. 1-2 minggu)

Ordered by ROI (per architecture agent):

### WP3.1 `ProfileManifest` (A2) — highest leverage
- New `src/manifest.rs`: one struct (id, title, description, extends,
  languages, fact_families, flows, review_map, exec) + one loader; delete
  the 8 partial parsers; every module takes `&ProfileManifest`.

### WP3.2 Typed errors (A1)
- `thiserror` enum (`NotFound`, `Parse`, `Io`, `Http`, `Config`, `Exec`);
  mechanical migration preserving message text; web.rs maps NotFound→404;
  main.rs maps to exit codes; brief drops the `"(…)"` content-string
  convention where the enum suffices.

### WP3.3 One markdown module (A4, A5, minor 14)
- `src/markdown.rs`: fence-aware section parser (the union of the 3),
  front-matter read/write with `yaml_scalar` escaping everywhere (incl.
  `render_merged`), slug, `est_tokens`. Delete the copies.

### WP3.4 main.rs decomposition (A7) + layering (A3)
- `Ctx { global, secrets, cwd, project }` built once; handlers move to
  `src/commands/*.rs`; `resolve_profile` → config.rs (web/palette reuse
  it); capability table in `clients/mod.rs` consumed by doctor/verify.
- Split knowledge.rs: `docstore` (raw IO) ← `inherit` ← presentation;
  breaks the cycle.

### WP3.5 Web console structure (A6, A8)
- app.js → 3-4 ES modules served via `include_str!` routes; hash routing
  (`#/profile/<id>/create`) so refresh keeps place; extract the duplicated
  palette-row renderer; tiny_http thread-per-request (or small pool).

### WP3.6 Indexer language parity (M7)
- Per-language decl/scope/signature node-kind tables (Rust:
  `function_item`/`struct_item`/`impl_item`; Dart equivalents) next to
  `language_for`; `symbol` output for Rust/Dart gains kind/scope/signature
  — dogfood quality on this very repo.

---

## Fase 4 — Product expansion (backlog menuju 1.0, urut prioritas)

1. **Team-knowledge authoring program** — the real moat. Sprint with the
   team to pour tuntun-specific conventions/recipes into the profiles
   (target: >50% team-specific content in the profiles the team uses;
   today android-mvvm/flutter-bloc are ~70% textbook). The composer +
   markdown import are ready for this.
2. **TS/JS grammar** then Python/Go/Swift — the most common agent targets;
   also unblocks a real `web-react` profile (README already promises it).
3. **Index freshness automation** — optional git post-commit/post-merge
   hook installed by `init` (or auto-reindex when stale, indexing is fast).
4. `--json` on `symbol`/`fact`; shell completions (`clap_complete`) + man
   pages (`clap_mangen`).
5. PRD roadmap: F2 onboarding wizard, F4 author-once→skills-sync glue,
   F5 verify-all.
6. Version pinning story for teams (pin minor in npm, changelog discipline).

---

## Keputusan yang dibutuhkan (recommendation first)

1. **rustfmt:** adopt in one mechanical commit + CI gate (recommended —
   team + AI contributors can't reproduce the hand-style; the 2026-06-24
   fmt incident becomes impossible once the whole tree is formatted), OR
   codify hand-format in CONTRIBUTING and skip the gate.
2. **kmp profile:** unbundle (recommended) or fill with real content now.
3. **exec gating UX:** TOFU confirm + trust cache + `--yes` (recommended)
   vs config allowlist only.
4. **Release cadence:** ship 0.2.4 as security-only fast patch
   (recommended) vs fold into 0.3.0.

---

## Estimasi keseluruhan

| Fase | Est. | Bisa paralel? |
|---|---|---|
| 0 — security | 2-4 hari | — (first) |
| 1 — correctness/trust | ~1 minggu | WP1.x independen satu sama lain |
| 2 — tests/CI | 1-2 minggu | ya, dengan Fase 3 |
| 3 — architecture | 1-2 minggu | ya, dengan Fase 2 |
| 4 — expansion | berkelanjutan | per item |

Total ke 1.0: ± 4-6 minggu kalender dengan flow superpowers +
subagent-driven development seperti biasa (branch per WP, spec + plan per
WP, review adversarial sebelum merge).
