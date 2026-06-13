# Design — Complete `brief` flows + generic `fact` lookup

> **Status:** Approved for planning · **Date:** 2026-06-13
> **Scope:** PRD §7.3 (generic fact lookup) and §9.1–9.3 (retrieval flows, budgeted
> output, diff-scoped review) — the parts of bucket A that make `brief`'s
> "budgeted context pack" claim true for all four flows.

## 1. Problem

`brief` is the headline value of palugada — "one budgeted context pack per flow" —
but only the `bugfix` flow is wired end to end. Of the declared flow steps, four
are stubbed (`brief.rs:5`): `prd.context`, `module.info`, `diff.scan`, and the
`convention(by-file-kind)` resolution that `review` depends on. As a result
`feature`, `refactor`, and `review` return mostly `(step '…' not yet available)`
placeholders. The budget logic is also naive (`len/4`, drop-on-overflow in flow
order), so `--budget` does not actually prioritise the most valuable content.

Separately, the typed fact lookup promised in PRD §7.3 (`fact <family> <name>`)
does not exist; today only generic `symbol` substring search is available.

## 2. Goals

- Implement the four missing `brief` step handlers so `feature`, `refactor`, and
  `review` produce real packs.
- Replace the naive budget with **priority-fill + truncation** so `--budget`
  keeps the highest-value content and points to the rest.
- Add a generic `fact <family> [name]` command, validated against the active
  profile's declared `fact_families` (stack-agnostic, zero Rust per new stack).
- Preserve `brief`'s "never dies on a network error" property: any networked or
  missing-data step degrades to an inline note, never aborts the pack.

## 3. Non-goals

- **Wiki tie-in for `prd.context`.** The `Issue` struct carries no linked-page
  metadata, so resolving a ticket's Confluence/Notion spec is out of scope here.
  `prd.context` returns the issue summary + a truncated description only.
- **Typed fact aliases** (`viewmodel`, `service`, `route` as their own
  subcommands). Hard-coding stack-specific verbs into the binary contradicts the
  stack-agnostic design (PRD G3); users wanting a short form add a shell alias.
- Token telemetry / `stats`, query cache, `skills sync`, second profile —
  separate specs.

## 4. Architecture changes

### 4.1 Thread connectors into `brief`

`brief::run` is currently `(kn, repo, profile, opts)` and purely local. To run
`prd.context` it needs the project's `IssueTracker`.

- `cmd_brief` (`main.rs`) resolves `ProjectConfig` + `AuthProfile` + `insecure`
  the same way `cmd_issue` does (`main.rs:757`), and passes them into
  `brief::run` as **optional** context (a `BriefConnectors { pc, auth, insecure }`
  wrapped in `Option`). When the active project or auth cannot be resolved, the
  context is `None`.
- `prd.context` builds the `IssueTracker` **lazily** via the existing
  `clients::issue_tracker(pc, auth, insecure)` factory — only when that step
  actually runs, so flows without `prd.context` (e.g. `bugfix`) make zero network
  calls and need no project.
- Any failure (no context, no `issue_tracker` configured, auth/network error)
  resolves to an inline `(…)` note, consistent with the existing
  `.unwrap_or_else(|e| format!("({e})"))` pattern. The pack still returns every
  local step.

### 4.2 `BriefContext` accumulator

Steps are no longer fully independent: `diff.scan` discovers which fact families
the changed files belong to, and `convention(by-file-kind)` consumes that set. A
small mutable accumulator is threaded through the step loop:

```rust
#[derive(Default)]
struct BriefContext {
    touched_families: BTreeSet<String>, // filled by diff.scan, read by by-file-kind
}
```

If `by-file-kind` runs without a preceding `diff.scan` (empty set), it emits a
note rather than dumping every convention.

### 4.3 Reusable `families_for_path()`

The "extension + `path_contains` → family" matching currently lives inline in the
indexer (`indexer.rs:111`). Extract it into a shared function so the indexer and
`diff.scan` classify files identically:

```rust
/// Returns the ids of every family whose ext/path_contains rules match `path`.
pub fn families_for_path(path: &str, ext: &str, families: &[CompiledFamily]) -> Vec<String>;
```

Loading + compiling `extractors.yaml` is also factored into a helper both call
sites share.

## 5. The four step handlers

All handlers return `(title, content)` and follow the existing `match kind` arm
shape in `brief::run`.

| Step | Target | Behaviour | Degradation |
|---|---|---|---|
| `prd.context` | ticket key (feature flow) | lazily build `IssueTracker`, `get_issue(target)` → `KEY — summary` / `Status · Type · Assignee` / `Spec excerpt: <description, truncated>` | no context / no tracker / empty target / fetch error → `(…)` note |
| `module.info` | file or dir | derive module prefix = directory portion of target (or target itself if a dir); filter `symbols.json` to symbols whose `file` starts with the prefix; output per-family counts + symbol list | no index → "(no index — run `palugada index`)"; empty target → "(module.info needs a target path)" |
| `diff.scan` | git ref/range (default `HEAD`) | `git diff --name-only <ref>`; classify each file via `families_for_path`; group output by family and record families into `BriefContext.touched_families` | git unavailable → "(git diff unavailable)"; no changes → "(no changed files vs <ref>)" |
| `convention(by-file-kind)` | — (reads context) | read `review_map` from `profile.yaml`; for each family in `touched_families`, collect its mapped convention topics, dedupe, emit each topic's outline via `knowledge::convention_outline` | empty `touched_families` → "(run diff.scan first)"; family absent from map → skipped |

**`prd.context` target detection:** the handler simply attempts `get_issue(target)`
when `target` is non-empty; it does not try to distinguish "is this a ticket key"
— a non-ticket target just yields a fetch-error note.

**`diff.scan` default ref:** when `target` is empty, diff against `HEAD` (working
tree vs last commit). `palugada brief review main` diffs against `main`.

## 6. Budget: priority-fill + truncation

Replaces the current sequential drop-on-overflow logic.

**Per-kind priority** (hard-coded default table in `brief.rs`; not profile data):

```
prd.context  >  symbol.find = module.info = diff.scan  >  convention  >  recipe  >  code.recent
   (spec)                  (target facts)                   (rules)       (how-to)   (history)
```

**Algorithm:**

1. Build every pack and estimate cost (`content.len()/4 + 8`, unchanged).
2. **Inclusion pass, by descending priority:** include a pack in full while it
   fits the remaining budget. When a pack would overflow, **truncate its content**
   to the remaining budget and append a pointer
   `(+N lines truncated — run \`<cmd>\` for the rest)`. Once budget is exhausted,
   lower-priority packs are marked `(omitted — over budget; run \`<cmd>\`)`.
3. **Render pass, in flow-declared order** (for readability), showing each pack's
   full / truncated / omitted state from step 2.

**Re-run pointers per kind:** `convention → palugada q <topic>`,
`recipe → palugada for <task>`, `symbol.find → palugada symbol <target>`,
`prd.context → palugada issue view <KEY>`, `module.info → palugada fact … / index`,
`code.recent → git log -- <target>`, `diff.scan → git diff <ref>`.

The first pack is always included even if it alone exceeds budget (truncated), so
a pack is never empty.

## 7. `fact <family> [name]` command

New clap subcommand `Fact { family: String, name: Option<String> }`.

- Parse the active profile's `profile.yaml`; validate `family` against the
  declared `fact_families` ids. Unknown family → error listing available families.
- Read `<repo>/.palugada/index/<family>.json`. Missing → "(no index — run
  `palugada index`)".
- With `name`: case-insensitive substring filter; without: list all (capped at
  the existing 30-row limit with a "narrow the query" note).
- Output reuses the `symbol_report` row format, scoped to the one kind. Shared
  helper between `symbol` and `fact`.

## 8. profile.yaml additions

`knowledge/profiles/android-mvvm/profile.yaml` gains a `review_map` block keyed by
family id → list of convention topics:

```yaml
review_map:
  viewmodel:  [architecture]
  repository: [architecture]
  service:    [architecture]
  route:      [architecture]
  i18n:       [architecture]
```

`architecture` is the only convention authored today; the map grows as
conventions are added. Parsing reuses the `serde_yaml` profile read already in
`brief.rs`.

## 9. Testing

Following the `#[cfg(test)]` + `tempfile` pattern in `indexer.rs:248`:

- **`families_for_path`** maps extensions/path_contains to the right family ids;
  non-matching files map to none.
- **priority-fill** truncates/omits lower-priority packs before higher-priority
  ones and inserts the re-run pointer; the highest-priority pack survives a tight
  budget (truncated, never dropped).
- **`diff.scan` → `by-file-kind`**: given a fixture repo with changed files and a
  `review_map`, `by-file-kind` emits exactly the mapped convention topics, deduped.
- **`fact`** rejects an unknown family (error lists known families) and filters by
  name substring against a fixture `<family>.json`.
- **`prd.context`** degrades to a note when connectors are `None` — exercised
  without any network call.

## 10. Affected files

| File | Change |
|---|---|
| `src/brief.rs` | new step handlers, `BriefContext`, priority-fill budget, `review_map` parse, connector plumbing |
| `src/indexer.rs` | extract `families_for_path` + extractors loader; `fact` family filter helper |
| `src/main.rs` | `cmd_brief` resolves project/auth and passes connectors; new `Fact` subcommand + `cmd_fact` |
| `knowledge/profiles/android-mvvm/profile.yaml` | add `review_map` |
| `README.md` | document `fact`; update the roadmap line now that flows are complete |
