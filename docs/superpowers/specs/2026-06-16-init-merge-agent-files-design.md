# `init`/`skills sync` merge into existing agent files + auto-detect agents

**Date:** 2026-06-16
**Status:** approved (design)
**Branch:** `fix/init-merge-agent-files`

## Problem

Two issues hit when running `palugada init` on a repo that already has an agent
guide file:

1. **Existing guide files are skipped, not merged.** `scaffold::write_file`
   skips any path that already exists (unless `--force`). So if a repo already
   has `AGENTS.md` (or `CLAUDE.md`/`GEMINI.md`), `init`/`skills sync` write
   nothing into it — palugada guidance never lands. Symptom: "skills gak ke-copy"
   on a machine whose project already had `AGENTS.md`.
2. **`init` defaults to `claude` only.** On a codex-only setup, the default
   targets `CLAUDE.md` instead of the `AGENTS.md` the user actually uses.

## Goal

`palugada init` (and `skills sync`) should, on a repo that already has agent
guide files, **append/replace a palugada-managed section** inside those files
(preserving the user's content), and by default **target whichever agents the
repo already uses**.

## Approach

- **Merge** the three user-owned root guides (`CLAUDE.md`, `AGENTS.md`,
  `GEMINI.md`) via a marker-delimited block; leave palugada-owned files
  (`.cursor/rules/palugada.mdc`, `.claude/skills/palugada-*`) on the existing
  write-if-missing / `--force` path.
- **Auto-detect** agents from existing guide files when `--agents` is `auto`
  (the new default).

## Components

### 1. `scaffold::upsert_marked_section` (new) + `SectionWrite`

```
const MARK_START: &str = "<!-- palugada:start -->";
const MARK_END:   &str = "<!-- palugada:end -->";
enum SectionWrite { Created, Merged, Unchanged }
fn upsert_marked_section(path, content) -> Result<SectionWrite, String>
```
Block = `MARK_START\n{content.trim_end()}\nMARK_END\n`.
- path absent → create dirs + write the block → `Created`.
- exists with both markers (end after start) → replace the span `[start ..= end]`
  with the block → `Merged` (idempotent: a re-run yields byte-identical output →
  `Unchanged`).
- exists without markers → append the block after the original content
  (separated by a blank line) → `Merged`.
- resulting text identical to existing → `Unchanged`.

### 2. `scaffold::write_agent_file` (new)

Routes by file name: `CLAUDE.md`/`AGENTS.md`/`GEMINI.md` → `upsert_marked_section`
(buckets into `written` on Created, `merged` on Merged, `skipped` on Unchanged);
any other path → existing `write_file` (write-if-missing, `--force`).

### 3. `scaffold::detect_agents` (new)

```
pub fn detect_agents(repo: &Path) -> Vec<String>
```
Pushes `claude` if `CLAUDE.md` exists, `codex` if `AGENTS.md`, `gemini` if
`GEMINI.md`, `cursor` if `.cursor/` exists; falls back to `["claude"]` when none
are present.

### 4. Wiring

- `GenerateOutcome` gains `pub merged: Vec<String>`.
- `generate()`: resolve agents as — explicit list if given and not `["auto"]`,
  else `detect_agents(&repo)`. Write the agent files through `write_agent_file`
  (custom skills + config skeleton stay on `write_file`). Return `merged`.
- `run()` prints a `merged   <path> (palugada section)` line per merged file.
- `cmd_init` / `skills sync` (`src/main.rs`): `--agents` default becomes `auto`;
  when the raw value is `auto`, resolve via `detect_agents`. `skills sync` writes
  via `write_agent_file` and reports merged counts/paths.
- Web `init_op` (`src/web.rs`) adds `merged` to its JSON response.

### 5. `--force` semantics

Guide files always upsert the marked block (never clobber user content), so
`--force` is irrelevant there. `--force` keeps its current meaning for
palugada-owned files (`.cursor/rules/palugada.mdc`, `.claude/skills/*`).

## Testing

- **Unit (`scaffold`):**
  - `upsert_marked_section`: absent→Created; existing-no-marker→Merged with the
    original text preserved and the block appended; existing-with-marker→Merged
    replacing in place (no duplicate block); identical re-run→Unchanged.
  - `detect_agents`: `AGENTS.md` only → `["codex"]`; `CLAUDE.md`+`GEMINI.md` →
    `["claude","gemini"]`; empty repo → `["claude"]`.
- **Manual e2e:** temp repo with a dummy `AGENTS.md` → `palugada init .` →
  `AGENTS.md` retains its original text and gains a `<!-- palugada:start -->…`
  section; agents auto-resolved to `codex`. Re-run → reported `Unchanged`.

## Files

| File | Action |
|---|---|
| `src/scaffold.rs` | `upsert_marked_section`, `SectionWrite`, `write_agent_file`, `detect_agents`, `GenerateOutcome.merged`, generate/run wiring, tests |
| `src/main.rs` | `--agents` default `auto` (init + skills sync), detect wiring, skills-sync merge + reporting |
| `src/web.rs` | `init_op` returns `merged` |

## Risk / notes

- Only the three markdown root guides merge; cursor `.mdc` (palugada-named) and
  `.claude/skills/*` stay palugada-owned. This keeps merge logic off files that
  carry their own frontmatter semantics.
- Auto-detect only triggers for `--agents auto` (the default); any explicit
  `--agents` value is honored unchanged.
