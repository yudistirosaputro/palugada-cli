# Design — Profile tooling (`profile list/validate/new`)

> **Status:** Built · **Date:** 2026-06-14 · Group C (first slice). Autonomous.

## Goal

Make stack profiles first-class to author and check: list bundled/local
profiles, validate one against the schema the engine expects, and scaffold a new
one. PRD §7.2.

## Design

New module `src/profile.rs` (all helpers take `kn: &Path` for testability) + a
`Profile` clap command with `list` / `validate <id>` / `new <id>`.

- **`profile list`** — scan `kn/profiles/*` for dirs containing `profile.yaml`;
  print each `id` + `title` (parsed from the yaml). Sorted.
- **`profile validate <id>`** — run checks and report `ok`/`FAIL` per check;
  exit non-zero if any fail (like `doctor`):
  1. profile dir exists;
  2. `profile.yaml` parses and has a non-empty `id`;
  3. `extractors.yaml` compiles via `indexer::load_families` (regex compiles,
     `.scm` query files exist + compile + have `@name`, family ids valid);
  4. `fact_families` is non-empty (`indexer::fact_families`);
  5. `conventions/_index.json` and `recipes/_index.json`, when present, are valid
     JSON.
- **`profile new <id>`** — refuse if the dir exists; otherwise scaffold a minimal
  **valid** profile (so `profile validate <id>` passes immediately): `profile.yaml`
  (id/title/languages/one `symbol` fact family/flows/review_map), `extractors.yaml`
  (one regex `symbol` family), and empty `conventions/_index.json` +
  `recipes/_index.json`. Returns the written paths.

## Non-goals (deferred, noted)

- `stats` + query-cache (needs telemetry infra), `skills sync`, project
  `conventions-overlay` + effective-rules merge — separate Group C pieces.

## Testing

- `list` finds a fixture profile and its title.
- `validate` passes on a well-formed fixture and fails (specific check) on a
  profile with a broken `extractors.yaml`.
- **Round-trip:** `new("p")` then `validate("p")` → all checks pass.

## Files

`src/profile.rs` (new); `src/main.rs` (`Profile` command + `cmd_profile` + `mod profile`);
`README.md`.
