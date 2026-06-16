# Codex skill parity — emit granular skills to `.agents/skills/`

**Date:** 2026-06-16
**Status:** approved (design)
**Branch:** `fix/codex-agents-skills`

## Problem

`palugada init`/`skills sync` emit the granular per-task skill files
(`palugada-search`/`-bugfix`/`-feature`/`-refactor`/`-review` + gated tool
skills + per-profile custom skills) **only for the `claude` agent**, under
`.claude/skills/`. Non-claude agents get a single consolidated guide file
(`AGENTS.md`/`GEMINI.md`/`.cursor/rules/palugada.mdc`). Users on Codex saw
"skills don't copy" — they were never generated.

Codex CLI supports the **same skill mechanism**: repo-level skills live in
`.agents/skills/<name>/SKILL.md` (frontmatter `name` + `description`;
progressive disclosure), per the official docs
(<https://developers.openai.com/codex/skills>).

## Goal

Give Codex the same granular skill set Claude gets, written to
`.agents/skills/`, and keep `AGENTS.md`/`GEMINI.md` as a rich consolidated
guide. Gemini/Cursor keep a single guide (they don't read a `SKILL.md` skills
folder).

## Approach

Factor the skill set into a base-dir-parameterized builder reused by both
Claude (`.claude/skills`) and Codex (`.agents/skills`). Same `SKILL.md` bodies
— Codex requires only `name`+`description` and ignores the extra
`allowed-tools` line Claude uses, so no body divergence.

## Components

### `src/scaffold.rs`

- **`standard_skill_set(profile, kinds, base) -> Vec<(String, String)>`** (new):
  builds `<base>/palugada-search/SKILL.md`, `<base>/palugada-{bugfix,feature,
  refactor,review}/SKILL.md`, and the gated tool skills
  `<base>/palugada-{git,docs,ci,design}/SKILL.md` (same gating as today). Bodies
  reuse `SKILL_SEARCH` / `skill_flow(...)` / `SKILL_GIT` / `SKILL_DOCS` /
  `SKILL_CI` / `SKILL_DESIGN`.
- **`skill_files`** refactor:
  - `claude` → `CLAUDE.md` (pointer) + `standard_skill_set(.., ".claude/skills")`.
  - `codex` → `AGENTS.md` (guide) + `standard_skill_set(.., ".agents/skills")`.
  - `gemini` → `GEMINI.md` (guide). `cursor` → `.cursor/rules/palugada.mdc`.
  - Remove the now-unused `skill_path` helper.
- **`custom_skill_files(kn, profile, base)`** — add a `base` parameter so custom
  per-profile skills can target `.claude/skills` or `.agents/skills`.
- **`generate()`** — emit custom skills for claude (`.claude/skills`) AND codex
  (`.agents/skills`) when those agents are targeted (today: claude only).
- **`single_guide`** — append one line noting on-demand granular skills live in
  `.agents/skills/` for agents that support skills (accurate for Codex; inert
  but harmless for others).

### `src/main.rs` — `skills sync`

Load the knowledge dir once; extend custom-skill emission to both claude
(`.claude/skills`) and codex (`.agents/skills`) bases. The standard set is
already covered because `skills sync` iterates `skill_files` output (now
includes `.agents/skills/*` for codex).

## Testing

- **Unit (`scaffold`):**
  - `skill_files` for `["codex"]` includes `AGENTS.md` and
    `.agents/skills/palugada-search/SKILL.md` + the four flow skills; for
    `["claude"]` still includes `.claude/skills/palugada-search/SKILL.md`
    (no regression).
  - tool-skill gating still applies under the codex base (e.g. `git_host` kind
    → `.agents/skills/palugada-git/SKILL.md`).
  - `custom_skill_files(.., ".agents/skills")` returns
    `.agents/skills/<name>/SKILL.md` paths (update the existing claude-base test
    to the new signature + add a codex-base assertion).
- **Manual e2e:** `palugada init --repo <tmp> --agents codex` on an empty repo →
  `AGENTS.md` + `.agents/skills/palugada-{search,bugfix,feature,refactor,review}/SKILL.md`;
  `--agents claude` unchanged (`.claude/skills/*`).

## Files

| File | Action |
|---|---|
| `src/scaffold.rs` | `standard_skill_set`, refactor `skill_files`, `custom_skill_files(base)`, `generate()` codex custom skills, `single_guide` line, tests |
| `src/main.rs` | `skills sync` custom-skill emission for claude + codex |

## Risk / notes

- Codex skill discovery path is `.agents/skills/` per current official docs
  (user confirmed); some third-party sources mention `.codex/skills/`. If a
  future Codex changes this, it's a one-line base-dir change.
- `allowed-tools` frontmatter is Claude-specific; Codex ignores unknown
  frontmatter, so reusing the same `SKILL.md` bodies is safe and DRY.
- Gemini has no `SKILL.md` folder convention (uses `GEMINI.md` + `.gemini/
  commands/*.toml`); it stays a single guide. Cursor stays a single `.mdc`
  (splitting into per-skill `.mdc` was not selected).
