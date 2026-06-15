# Design — Per-profile custom skills (A3)

> **Status:** Approved for planning · **Date:** 2026-06-14 · Sub-project A3 (final
> piece of the skill-integration model; builds on A2's `skill_files`).

## 1. Problem

A2 generates a rich *standard* skill set, but every stack also has bespoke
guidance the user wants as skills (e.g. a project's MVI conventions, a custom
review checklist). Today there's nowhere to put stack-specific custom skills that
travel with the profile and get emitted into projects bound to it.

## 2. Goal

Let a profile carry **user-authored custom skills** that `init` / `skills sync`
emit into a bound project's `.claude/skills/`, alongside the standard A2 set.
Plus a scaffolder to create one.

## 3. Design

- **Storage:** `knowledge/profiles/<id>/skills/<name>/SKILL.md`. These live with
  the profile (so they travel with it) and are authored by the user.
- **Emission:** a new pure-ish reader
  `scaffold::custom_skill_files(kn, profile) -> Vec<(String, String)>` returns
  `(".claude/skills/<name>/SKILL.md", body)` pairs for every
  `profiles/<profile>/skills/<name>/SKILL.md`. `generate` (init) and
  `cmd_skills_sync` append these **after** the standard `skill_files` set and
  write them through the same path (so `skills sync`'s expand-not-overwrite and
  `init`'s `--force` both apply). **Claude only** — codex/gemini/cursor use a
  single guide file and can't host a skills directory (documented limitation).
  Custom skills are emitted only when `claude` is among the target agents.
- **Scaffolder:** `palugada skills new <name> [--profile <id>]` writes a starter
  custom skill to `knowledge/profiles/<id>/skills/<name>/SKILL.md`.
  - Profile resolution: `--profile`, else the active/`--project` project's bound
    profile (reuse `resolve_profile`).
  - Validate `<name>`: `[a-z0-9-_]+`, and **reject a `palugada-` prefix**
    (reserved for the standard set, prevents collisions/overwrites).
  - Refuse if the skill already exists.
  - Starter body: valid SKILL.md front-matter (`name`, a TRIGGER `description`,
    `allowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit`) + a short
    "edit me" body that references `palugada for`/`q`/`brief` like the standard
    skills (so it's profile-agnostic-friendly by example).

## 4. Collision rule

Standard skills are named `palugada-*`. Custom skills may not start with
`palugada-` (enforced by `skills new`; if a hand-authored one does, the standard
set wins because it's emitted first and `skills sync` skips the later duplicate).

## 5. Testing

- `custom_skill_files` over a fixture knowledge dir with
  `profiles/p/skills/mvi-state/SKILL.md` returns one pair with path
  `.claude/skills/mvi-state/SKILL.md` and the file's body; returns empty when the
  profile has no `skills/` dir.
- `skills new` scaffolds a valid SKILL.md (front-matter parses; contains the
  given name); **rejects** a `palugada-` prefix and an invalid name; **refuses**
  to overwrite an existing one.
- Integration: `skills new` then `custom_skill_files` includes it; (smoke) `init`
  / `skills sync` for a project on that profile writes it into `.claude/skills/`.

## 6. Non-goals

- Web-console authoring/editing of custom skills (later).
- Custom skills for codex/gemini/cursor (single-file guide limitation).
- Templated/profile-substituted custom-skill bodies (the user writes them).

## 7. Affected files

| File | Change |
|---|---|
| `src/scaffold.rs` | `custom_skill_files(kn, profile)` reader + a `new_custom_skill(kn, profile, name)` scaffolder (+ name validation) |
| `src/main.rs` | `skills new <name>` subcommand + handler; `generate` and `cmd_skills_sync` append custom skills (need `kn` + profile) |
| `README.md` | document per-profile custom skills + `skills new` |

Note: `generate`/`cmd_skills_sync` must resolve the knowledge dir to read custom
skills. `generate` currently doesn't load `kn`; it will resolve it
best-effort (`knowledge::knowledge_dir`) and skip custom skills if unavailable
(custom skills are additive, never fatal to `init`).
