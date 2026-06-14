# Design — Per-project profile control (`profile use`)

> **Status:** Approved for planning · **Date:** 2026-06-14 · Sub-project A1 of the
> "profiles + skills" rework (A2 = rich skill generation, A3 = per-profile custom
> skills come later).

## 1. Problem

The active profile for a project lives in `<repo>/.palugada/config.yaml`
(`profile:`), bound once by `palugada init`. There is **no way to see or switch
it afterward** — you must hand-edit the file or re-run `init --force` (which
clobbers everything). `project use` only sets the active *project*, not its
profile, and the web console doesn't surface the binding at all. Users building
multiple profiles (kotlin-mvvm, kotlin-mvi, web, …) can't point a project at a
different one.

## 2. Goal

See and switch a project's bound profile, from the CLI and the web console — a
pure config change, with no clobbering and no implicit heavy work.

## 3. Lifecycle clarity (the model this assumes)

| Command | Role |
|---|---|
| `palugada init` | one-time, offline: register project + scaffold agent skills + bind a profile (does **not** index) |
| `palugada index` | build/refresh the symbol + fact index |
| `palugada profile use <id>` (this) | flip the bound profile in `config.yaml` |

**Switching is a config flip only.** `q`/`for`/`brief` (knowledge) read the active
profile live, and the generic `symbol` index is language-driven — both follow the
new profile **immediately, no re-index**. A re-index is **optional**, needed only
if the new profile declares *different curated fact families* (e.g. mvi's
`intent`/`state`) and you want `palugada fact <family>` populated. The code didn't
change, so re-indexing is never *required* by the switch itself.

Generated skills are unaffected when they are *references* to palugada commands
(they resolve against the active profile live) — which is the principle A2 will
build on. So `profile use` does **not** regenerate skills.

## 4. CLI

- **`palugada profile use <id> [--project <name>]`** — set a project's bound
  profile:
  1. Resolve the target project (active project, or `--project <name>`) → its
     `repo_path` from the registry. Error clearly if no project resolves.
  2. **Validate** `<id>` exists (`knowledge/profiles/<id>/profile.yaml`); else
     error listing available profiles (via `profile::list`).
  3. `ProjectConfig::load_from(repo_path)` → set `profile = id` → `save_to(repo_path)`.
  4. Print: `project '<name>' now uses profile '<id>'` + the lightweight hint:
     *"knowledge & symbols already follow it; run `palugada index` only if this
     profile adds new fact families."*
- **`palugada project list`** — augment each row to show the bound profile (read
  each registered project's `config.yaml` best-effort):
  `ttsecuritas  *  profile=android-mvvm  /Users/me/dev/ttsecuritas`.

(`palugada profile list` already lists available profiles — unchanged.)

## 5. Web console

- `GET /api/projects` — each project gains a `profile` field (read from its
  `config.yaml`; empty if unreadable).
- New `POST /api/project/{name}/profile` `{ "profile": "<id>" }` — validate the
  profile exists, load that project's `ProjectConfig`, set + save. Returns
  `{ ok, name, profile }` or `{ error }`.
- **Projects** view: each project shows its bound profile + a `<select>` of
  available profiles (from `/api/profiles`); choosing one POSTs the change and
  re-renders.

## 6. Library helper

Add `config::set_project_profile(global, name, profile_id) -> Result<(), String>`
(or do it inline in the CLI/web handlers) that resolves the repo from the
registry, validates against the knowledge dir's profiles, and writes the project
config. Keeping it in `config.rs` lets the CLI and web share it and be tested
without a live server.

## 7. Testing

- **Round-trip:** write a project `config.yaml` (tempdir) with `profile: a`,
  call the setter to `b`, reload → `profile == "b"`.
- **Validation:** setting an unknown profile id returns an error naming the
  available profiles, and does **not** modify the file.
- Web: `route("POST", "/api/project/x/profile")` parses to the new route;
  `/api/projects` JSON includes `profile`.

## 8. Non-goals (A1)

- Skill regeneration on switch (A2's `skills sync`, profile-agnostic references).
- Auto re-index on switch (the user confirmed it's unnecessary for a switch).
- Creating profiles (`profile new` already exists, CLI + web).

## 9. Affected files

| File | Change |
|---|---|
| `src/config.rs` | `set_project_profile` helper + test |
| `src/main.rs` | `Profile::Use { id }` subcommand + `cmd_profile` arm; `project list` shows bound profile |
| `src/web.rs` | `/api/projects` adds `profile`; `POST /api/project/{name}/profile` route + handler |
| `src/web/app.js` | Projects view shows + switches profile |
| `README.md` | document `profile use` + the switch-is-a-config-flip model |
