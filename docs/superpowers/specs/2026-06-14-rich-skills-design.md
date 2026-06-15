# Design — Rich, profile-agnostic skill generation (A2)

> **Status:** Approved for planning · **Date:** 2026-06-14 · Sub-project A2.
> (A3 = per-profile custom skills is a later cycle.)

## 1. Problem

`palugada init` generates a thin skill set: a `CLAUDE.md` guide + four near-empty
`SKILL.md` flow files that hardcode the profile name. It's nothing like the rich,
opinionated ttsecuritas `.claude/skills/` set (a router with a "use the CLI before
grep" hard rule, `allowed-tools`, routing, and deep tool skills for git/wiki/ci).
There's no `skills sync` to refresh them, no tool/integration skills, and the
content isn't profile-agnostic (so it implies regen on profile switch).

## 2. Goal

Generate a rich skill set whose content is **references to palugada commands**
(never inlined knowledge), so it is token-cheap, profile-switch-free, and turns
palugada into the search/standard backend the agent calls instead of grepping.
Add `palugada skills sync` to (re)generate it **without clobbering** user edits.

## 3. Principles

- **Reference, don't inline.** Skills say `palugada for --list`, `q --list`,
  `brief <flow>`, `symbol`, `fact` — never paste convention/recipe text. Knowledge
  stays token-split in the profile; switching profile needs no regen.
- **CLAUDE.md is a thin pointer, not a dump.** The depth lives in on-demand skills.
- **Search-first is the headline rule.** A dedicated skill makes the agent run
  `palugada symbol`/`fact` before any grep/find/rg.
- **Expand, don't overwrite.** `skills sync` writes missing files and skips
  existing ones; `--force` is the only way to overwrite.

## 4. Generated skill set

### Always (profile-agnostic)
- **`CLAUDE.md`** — short pointer: project uses palugada; the search-first rule in
  one line; the list of `palugada-*` skills; `palugada <cmd> --help`. (Same short
  body also drives `AGENTS.md`/`GEMINI.md`/cursor — see §5.)
- **`.claude/skills/palugada-search/SKILL.md`** — the grep-replacement standard.
  TRIGGER: locating a function/class/symbol, "where is X defined", before
  grep/find/rg. `allowed-tools: Bash(palugada *), Grep, Glob, Read`. Discovery
  order: `palugada symbol <name>` → `palugada symbol <name> --kind <k>` →
  `palugada fact <family>` → fall back to grep only if empty (and say so).
- **`.claude/skills/palugada-{bugfix,feature,refactor,review}/SKILL.md`** — four
  rich flow skills. Each: a task-specific TRIGGER, `allowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit`,
  and steps: get the pack (`palugada brief <flow> <target>`), then the rules
  (`palugada for <task>` / `palugada q <topic>`; `--list` to discover), then act.

### Gated by configured integration (read from the project's `config.yaml integrations`)
- **`palugada-git`** (if `git_host`) — `palugada git whoami`, `palugada pr recent <file>`,
  commit/branch conventions, push safety; uses real `git` + `gh`/`glab`.
- **`palugada-docs`** (if `issue_tracker` or `wiki`) — `palugada issue view <KEY>`,
  `palugada wiki page <ID>`, `palugada prd fetch/search/cat`.
- **`palugada-ci`** (if `ci` or `chat`) — `palugada ci status <JOB>`, `palugada notify <msg>`.
- **`palugada-design`** (if `design`) — `palugada design file <KEY>`.

## 5. Per-agent emission

Only Claude Code has a skills directory. So:
- **claude**: the thin `CLAUDE.md` pointer **+** the `.claude/skills/palugada-*`
  set above (gated).
- **codex / gemini / cursor**: a single richer guide file (`AGENTS.md` /
  `GEMINI.md` / `.cursor/rules/palugada.mdc`) that **inlines** the search-first
  rule + a compact command reference (flows + the configured connectors), because
  these tools can't load a multi-skill directory. Still references-only (lists
  `palugada` commands), never profile knowledge.

## 6. Mechanism

- `scaffold::generate` (already split from `run`) is extended to emit the rich
  set, gating tool skills on the resolved `ProjectConfig.integrations`.
- New **`palugada skills sync [--agents <list>] [--force]`** → `cmd_skills_sync`:
  resolves the active (or `--project`) project's repo + its config, regenerates
  the skill set there. **Additive by default**: writes files that don't exist,
  **skips existing** (reports them); `--force` overwrites. Reuses the same
  generator as `init` (so they never drift). `init` keeps its current `--force`
  semantics.
- A shared `scaffold::skill_files(profile, integrations, agents) -> Vec<(path, body)>`
  produces the (relative path, content) pairs; both `init`/`generate` and
  `skills sync` write them through the same `write_file` (exists/force logic).

## 7. Testing

- `skill_files` for a config with all integrations + agents=[claude] includes
  `palugada-search`, the 4 flow skills, and `palugada-git`/`-docs`/`-ci`/`-design`;
  with only `git_host` set, it includes `palugada-git` but **not** `-docs`/`-ci`/`-design`.
- Generated skill bodies are profile-agnostic: they contain `palugada for --list`
  / `brief` / `symbol` and do **not** contain a hardcoded profile id or convention
  text. (Assert the search skill body contains the "before grep" rule.)
- `skills sync` additive: a pre-existing skill file is left untouched (byte-equal)
  unless `--force`; a missing one is written. (Tested via `scaffold`'s write path
  with a tempdir.)
- `cargo test` green; `init` still works (smoke).

## 8. Non-goals

- A3 per-profile custom skills (`profiles/<id>/skills/` emitted on init).
- Managed-marker partial-overwrite (we use whole-file additive + `--force`).
- Inlining any profile knowledge into skills.

## 9. Affected files

| File | Change |
|---|---|
| `src/scaffold.rs` | rich templates (search/flows/tools + thin guide), `skill_files` builder gated by integrations, reused by `generate` + sync |
| `src/main.rs` | `skills sync` command + `cmd_skills_sync`; `init` uses the new generator |
| `README.md` | document the skill set + `skills sync` |
