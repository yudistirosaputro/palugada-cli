# Per-project profile control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans. Steps use `- [ ]`.

**Goal:** See & switch a project's bound profile — `palugada profile use <id>` (CLI), `project list` shows it, and the web Projects view shows + switches it.

**Architecture:** A shared `config::set_profile(repo_path, id)` edits `<repo>/.palugada/config.yaml`; CLI and web validate the id against `profile::list` then call it. Pure config flip — no index, no skill regen.

**Reference spec:** `docs/superpowers/specs/2026-06-14-profile-use-design.md`

**Test:** `cargo test` · **Build:** `cargo build`

---

## Task 1: `config::set_profile` helper

**Files:** `src/config.rs` (+ test).

- [ ] **Step 1: failing test** in `config::tests`:

```rust
    #[test]
    fn set_profile_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().to_string_lossy().to_string();
        ProjectConfig { profile: "a".into(), ..Default::default() }.save_to(&repo).unwrap();
        set_profile(&repo, "b").unwrap();
        assert_eq!(ProjectConfig::load_from(&repo).unwrap().profile, "b");
    }
```

- [ ] **Step 2: implement** in `src/config.rs`:

```rust
/// Set a project's bound profile by editing its `.palugada/config.yaml`.
/// (Caller validates the profile id exists.)
pub fn set_profile(repo_path: &str, profile_id: &str) -> Result<(), String> {
    let mut pc = ProjectConfig::load_from(repo_path)?;
    pc.profile = profile_id.to_string();
    pc.save_to(repo_path)
}
```

- [ ] **Step 3:** `cargo test set_profile_round_trips` → pass. (If `save_to` doesn't create `.palugada/`, add `fs::create_dir_all` there — verify by running.) **Commit** `feat(config): set_profile helper`.

---

## Task 2: CLI — `profile use` + `project list` shows profile

**Files:** `src/main.rs`.

- [ ] **Step 1:** Add `Use { id: String }` to `enum ProfileCmd`:

```rust
    /// Bind this project to a profile: `profile use <id>`.
    Use { id: String },
```

- [ ] **Step 2:** Change `cmd_profile` to take the project override, and add the `Use` arm. Update the signature + dispatch:

`Commands::Profile { action } => cmd_profile(action, project),`

```rust
fn cmd_profile(action: ProfileCmd, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    match action {
        // ... List / Validate / New arms unchanged ...
        ProfileCmd::Use { id } => {
            let profs = profile::list(&kn)?;
            if !profs.iter().any(|(pid, _)| pid == &id) {
                return Err(format!(
                    "unknown profile '{id}' (available: {})",
                    profs.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
            let name = config::resolve_project_name(&global, project, &cwd)?;
            let entry = global.projects.registered.get(&name)
                .ok_or_else(|| format!("project '{name}' is not registered"))?;
            config::set_profile(&entry.repo_path, &id)?;
            println!("project '{name}' now uses profile '{id}'");
            println!("knowledge & symbols already follow it; run `palugada index` only if this profile adds new fact families.");
            Ok(())
        }
    }
}
```

- [ ] **Step 3:** In `cmd_project`'s `ProjectCmd::List` arm, show the bound profile:

```rust
            for (name, e) in &global.projects.registered {
                let marker = if *name == global.projects.active { "*" } else { " " };
                let prof = config::ProjectConfig::load_from(&e.repo_path)
                    .map(|c| c.profile)
                    .unwrap_or_default();
                let prof = if prof.is_empty() { "—".to_string() } else { prof };
                println!("{marker} {name}  profile={prof}  ->  {}", e.repo_path);
            }
```

- [ ] **Step 4:** `cargo test && cargo build`. Smoke:
```bash
mkdir -p /tmp/plg-pu && palugada init --repo /tmp/plg-pu --name plg-pu --agents claude --force >/dev/null
palugada profile use android-mvvm --project plg-pu
palugada profile use nonexistent --project plg-pu   # expect error listing available
palugada project list | grep plg-pu
palugada project remove plg-pu; rm -rf /tmp/plg-pu
```
Expected: switch prints the confirmation+hint; unknown profile errors with the available list; `project list` shows `profile=android-mvvm`.

- [ ] **Step 5: commit** `feat(profile): profile use <id> + project list shows bound profile`.

---

## Task 3: Web API — projects carry profile + set-profile endpoint

**Files:** `src/web.rs` (+ route test).

- [ ] **Step 1:** Add a route + test. In `enum Route`, add `SetProjectProfile(String)`. In `route()`:

```rust
        ("POST", ["api", "project", name, "profile"]) => Route::SetProjectProfile((*name).to_string()),
```

Test (extend `route_parses_paths`):
```rust
        assert_eq!(route("POST", "/api/project/x/profile"), Route::SetProjectProfile("x".into()));
```

- [ ] **Step 2:** `projects_json` — add `profile` to each project:

```rust
        .map(|(name, e)| {
            let profile = crate::config::ProjectConfig::load_from(&e.repo_path)
                .map(|c| c.profile)
                .unwrap_or_default();
            json!({ "name": name, "repo_path": e.repo_path, "active": *name == active, "profile": profile })
        })
```

- [ ] **Step 3:** In `api()`, add the write arm + handler:

```rust
        Route::SetProjectProfile(name) => write_op(|| set_project_profile(&name, body)),
```

```rust
fn set_project_profile(name: &str, body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Req { profile: String }
    let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let global = crate::config::GlobalConfig::load_or_default()?;
    let kn = crate::knowledge::knowledge_dir(&global)?;
    let profs = crate::profile::list(&kn)?;
    if !profs.iter().any(|(p, _)| *p == req.profile) {
        return Err(format!(
            "unknown profile '{}' (available: {})",
            req.profile,
            profs.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }
    let entry = global.projects.registered.get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    crate::config::set_profile(&entry.repo_path, &req.profile)?;
    Ok(json!({ "ok": true, "name": name, "profile": req.profile }))
}
```

- [ ] **Step 4:** `cargo test && cargo build`. **Commit** `feat(web): projects show bound profile + set-profile endpoint`.

---

## Task 4: Web UI — Projects view shows + switches profile

**Files:** `src/web/app.js`.

- [ ] **Step 1:** Rewrite `renderProjects` to fetch projects **and** profiles, and render a profile `<select>` per project that POSTs on change:

```js
async function renderProjects() {
  view.innerHTML = "<h2>Projects</h2>";
  let d, profs;
  try {
    d = await api("/api/projects");
    profs = (await api("/api/profiles")).profiles;
  } catch (e) { toast(e.message, true); return; }
  if (!d.projects.length) {
    view.appendChild(h(`<p class="muted">No registered projects yet. Use <code>palugada init</code> or generate skills under a profile.</p>`));
  }
  d.projects.forEach(p => {
    const opts = profs.map(pr =>
      `<option value="${esc(pr.id)}"${pr.id === p.profile ? " selected" : ""}>${esc(pr.id)}</option>`).join("");
    const card = h(`<div class="card"><strong>${esc(p.name)}</strong>${p.active ? ' <span class="pill">active</span>' : ""}
      <div class="muted">${esc(p.repo_path)}</div>
      <div class="row" style="margin-top:6px"><label style="margin:0">profile</label>
        <select class="proj-profile" style="max-width:240px">${opts}</select></div></div>`);
    card.querySelector(".proj-profile").onchange = async (e) => {
      try { await api(`/api/project/${encodeURIComponent(p.name)}/profile`, "POST", { profile: e.target.value });
        toast(`${p.name} → ${e.target.value}`); }
      catch (err) { toast(err.message, true); }
    };
    view.appendChild(card);
  });
}
```

- [ ] **Step 2:** `cargo build`; manual smoke: `palugada web --open`, register a project (or use an existing one), open **Projects**, change its profile dropdown → toast confirms; `palugada project list` shows the new profile.

- [ ] **Step 3: commit** `feat(web): Projects view shows + switches a project's profile`.

---

## Task 5: Docs + final verify + adversarial review

**Files:** `README.md`.

- [ ] **Step 1:** README — add `palugada profile use <id>` to the commands table; add a short note that switching a profile is a config flip (knowledge/symbols follow live; re-index only for new fact families).

- [ ] **Step 2:** `cargo test && cargo build --release`.

- [ ] **Step 3 (ultracode):** Run an adversarial code-review Workflow over the branch diff (lenses: correctness/no-clobber, validation/error paths, web route + JSON, CLI resolution edge cases). Fix any confirmed findings.

- [ ] **Step 4: commit** `docs: document profile use`.

---

## Self-review notes

- **Spec coverage:** §4 CLI → T2; §5 web → T3+T4; §6 helper → T1; §7 tests → T1 (round-trip) + T3 (route); §8 non-goals respected (no index/skill regen).
- **Type consistency:** `config::set_profile(repo_path, id)` defined T1, used T2 (CLI) + T3 (web). `Route::SetProjectProfile` added in `route()` + matched in `api()` (T3). `projects_json` `profile` field consumed by `renderProjects` (T4).
- **Validation is in callers** (CLI + web both check `profile::list`); the helper is mechanical.
