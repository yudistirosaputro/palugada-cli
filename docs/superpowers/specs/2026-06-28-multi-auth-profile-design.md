# Multi auth-profile (F1) — design

> Status: approved · Date: 2026-06-28 · Branch: `feat/multi-auth-profile`
> PRD: `docs/PRD-onboarding-connectors.md` §7 F1.

## Problem

A freelancer needs a separate credential set per client. Auth-profiles (named token
bundles in `~/.palugada/secrets.yaml`) already exist and a project picks one via
`auth_profile`, but the only UI manages the single `default` profile (`app.js` renders
"multi-profile soon"; `credentials.rs` hardcodes `"default"` in `global_view`/`apply_global`/
`global_verify`). A second client forces hand-editing `secrets.yaml`.

## Decisions (locked)

- **Web UI = switcher on the Connectors page.** A profile dropdown above the 6 connector
  cards + New/Delete. Switching reloads/saves/verifies the *selected* profile's tokens.
  Wiring (provider/base_url) stays the **global default** (shared); only **tokens** are
  per-profile — a UI note states this. Per-client *wiring* remains the per-project Credentials card.
- **CLI = lifecycle only.** `config auth list | add <name> | rm <name> | show <name>` (show is
  masked). Token entry stays in the web console (masked; safer than shell history). No `set`.
- **Delete blocks if in use.** Refuse to delete a profile any registered project references
  (resolved: empty `auth_profile` counts as `default`); error names the projects.
- **Create = empty profile** (no clone — auth-profiles are pure secret bundles; copying tokens
  across clients is a footgun). **Rename deferred** (delete+add for v1).
- Per-project Credentials card's `auth profile` field becomes a **dropdown** of existing profiles.

## Design

### Backend
- `src/config.rs` (`Secrets`): add `valid_auth_profile_name(name)` (`[A-Za-z0-9_-]`, 1–64),
  `list_auth_profiles() -> Vec<String>`, `add_auth_profile(name)` (err if exists/invalid),
  `delete_auth_profile(name)` (err if missing). Pure helper `projects_using_profile(pairs, name)`
  for the in-use guard (resolves empty → `default`).
- `src/credentials.rs`: parameterize `global_view(profile)`, `apply_global(profile, cap, body)`,
  `global_verify(profile, cap, body)` off the hardcoded `"default"`. New handlers:
  `list_auth_profiles_view()` → `{profiles:[{name, in_use_by:[...]}]}`; `create_auth_profile(body{name})`;
  `delete_auth_profile_guarded(name)` (loads `GlobalConfig` + each `ProjectConfig`, blocks if in use).

### Web (`src/web.rs` + `src/web/app.js`)
- Routes: `GET /api/auth-profiles`, `POST /api/auth-profiles` `{name}`, `DELETE /api/auth-profiles/{name}`,
  `GET /api/auth-profiles/{name}/connectors`, `POST /api/auth-profiles/{name}/connectors/{cap}`,
  `POST /api/auth-profiles/{name}/connectors/{cap}/verify`. Old `/api/connectors*` stay as
  `default`-profile aliases (back-compat).
- `app.js renderConnectors`: replace the "multi-profile soon" chip with a profile `<select>`
  (from `/api/auth-profiles`) + New/Delete; cards read/save/verify against the selected profile;
  a one-line note that wiring is the global default. New-profile prompt (name only). Delete
  confirms; surfaces the in-use error. Per-project `credentialsCard` auth-profile input → `<select>`.

### CLI (`src/main.rs`)
- `ConfigCmd::Auth { action: AuthCmd }`; `AuthCmd::{List, Add{name}, Rm{name}, Show{name}}`;
  `cmd_config_auth` handler reusing the config.rs CRUD + `masked` print for `show`.

## Acceptance criteria

1. Create `client-a` + `client-b` in the web UI (and via `config auth add`); both round-trip to
   `secrets.yaml` with no hand-editing; `config auth list` enumerates them.
2. Switching the Connectors profile dropdown reloads that profile's masked tokens; saving a
   connector under one profile never mutates another's tokens.
3. `config auth rm <name>` / web delete is **blocked** when a registered project references it,
   with an error naming the project(s); succeeds once unreferenced.
4. Old `/api/connectors*` still behave (= `default`); existing tests stay green.

## Constraints
No async (sync `ureq`), `Result<T,String>`, inline tests, secrets masked on read / blank=keep /
0600 file, loopback+host-guarded. **No `cargo fmt`** (repo hand-formats wide-style).
