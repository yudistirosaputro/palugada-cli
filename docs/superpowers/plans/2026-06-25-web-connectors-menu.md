# Connectors & API Keys (global web menu) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a global **Connectors** menu to `palugada web` that sets API keys + default provider wiring once; projects inherit the wiring per-field and only own their `repo`.

**Architecture:** Two stores — default wiring (`provider`+`base_url`) in `~/.palugada.yaml` (`GlobalConfig.default_integrations`), API keys in `~/.palugada/secrets.yaml` (the `default` auth-profile). Runtime resolution folds the global wiring *under* each project's explicit integrations at the single chokepoint `resolve_project()`. The web page reads/writes both via three new global routes; verify runs in-place for repo-free providers.

**Tech Stack:** Rust (single binary), `serde`/`serde_yaml`/`serde_json`, embedded `tiny_http` web server, vanilla JS frontend (no build step), Pop Workbench CSS design system.

## Global Constraints

- **No `cargo fmt`** — the repo hand-formats in a compact wide style; CI is build/test/smoke only. Match the surrounding style by hand.
- **Bin crate** — `palugada` has no lib target, so run `cargo test` / `cargo test <filter>` (NEVER `cargo test --lib`, which errors with "no library targets found"). Unit tests live in each module's `#[cfg(test)] mod tests`.
- **Secrets never echoed** — GET responses mask every token via `mask_secret()`; plaintext must never leave the process.
- **Blank = keep** — a submitted empty token leaves the stored value unchanged.
- **Loopback only** — new routes are ordinary global routes under the existing 127.0.0.1 + Host-guard; no new network surface beyond per-click Verify.
- **Reuse the Pop Workbench design system verbatim** — existing tokens/components in `src/web/style.css`; no new palette or fonts.
- **`default` auth-profile only** for v1 (the page is structured so a switcher can be added later).
- Tests live in the existing `#[cfg(test)] mod tests` of each file; run with `cargo test`.

---

### Task 1: Config model — `default_integrations` + per-field merge

**Files:**
- Modify: `src/config.rs` (`Integrations` derive + `is_empty`, `GlobalConfig` field + `Default`, new `merge_provider`/`merge_integrations`, `resolve_project` merge)
- Test: `src/config.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Produces: `config::merge_integrations(global: &Integrations, project: &Integrations) -> Integrations` (pub); `GlobalConfig.default_integrations: Integrations`; `Integrations::is_empty(&self) -> bool`; `Integrations` now derives `Clone`.

- [ ] **Step 1: Write the failing tests**

Add to `src/config.rs` `mod tests`:

```rust
    #[test]
    fn merge_integrations_inherits_per_field() {
        let global = Integrations {
            git_host: Some(Provider {
                provider: "github".into(),
                base_url: "https://api.github.com".into(),
                repo: String::new(),
            }),
            ..Default::default()
        };
        // project sets ONLY repo (empty provider/base_url) → inherit provider+base_url
        let project = Integrations {
            git_host: Some(Provider {
                provider: String::new(),
                base_url: String::new(),
                repo: "o/n".into(),
            }),
            ..Default::default()
        };
        let g = merge_integrations(&global, &project).git_host.unwrap();
        assert_eq!(g.provider, "github");
        assert_eq!(g.base_url, "https://api.github.com");
        assert_eq!(g.repo, "o/n");
    }

    #[test]
    fn merge_integrations_project_field_overrides_global() {
        let global = Integrations {
            git_host: Some(Provider {
                provider: "github".into(),
                base_url: "https://api.github.com".into(),
                repo: String::new(),
            }),
            ..Default::default()
        };
        let project = Integrations {
            git_host: Some(Provider {
                provider: "gitlab".into(),
                base_url: "https://gitlab.com".into(),
                repo: "g/p".into(),
            }),
            ..Default::default()
        };
        let g = merge_integrations(&global, &project).git_host.unwrap();
        assert_eq!(g.provider, "gitlab");
        assert_eq!(g.base_url, "https://gitlab.com");
        assert_eq!(g.repo, "g/p");
    }

    #[test]
    fn merge_integrations_empty_global_is_identity() {
        assert!(Integrations::default().is_empty());
        let project = Integrations {
            wiki: Some(Provider { provider: "notion".into(), base_url: String::new(), repo: String::new() }),
            ..Default::default()
        };
        let m = merge_integrations(&Integrations::default(), &project);
        assert_eq!(m.wiki.as_ref().unwrap().provider, "notion");
        assert!(m.git_host.is_none());
        assert!(m.ci.is_none());
    }

    #[test]
    fn merge_integrations_inherits_when_project_slot_absent() {
        let global = Integrations {
            wiki: Some(Provider { provider: "notion".into(), base_url: String::new(), repo: String::new() }),
            ..Default::default()
        };
        let m = merge_integrations(&global, &Integrations::default());
        let w = m.wiki.unwrap();
        assert_eq!(w.provider, "notion");
        assert_eq!(w.repo, ""); // global default carries no repo
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test merge_integrations 2>&1 | tail -20`
Expected: FAIL (compile error) — `cannot find function 'merge_integrations'` / `no method 'is_empty'` / `use of moved value` (Clone missing).

- [ ] **Step 3: Add `Clone` + `is_empty` to `Integrations`**

In `src/config.rs`, change the `Integrations` derive (currently `#[derive(Debug, Serialize, Deserialize, Default)]`) to add `Clone`, and add an impl right after the struct:

```rust
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Integrations {
    #[serde(default)]
    pub issue_tracker: Option<Provider>,
    #[serde(default)]
    pub wiki: Option<Provider>,
    #[serde(default)]
    pub design: Option<Provider>,
    #[serde(default)]
    pub ci: Option<Provider>,
    #[serde(default)]
    pub git_host: Option<Provider>,
    #[serde(default)]
    pub chat: Option<Provider>,
}

impl Integrations {
    /// True when no capability slot is wired — keeps `~/.palugada.yaml` clean
    /// (`skip_serializing_if`) and gives existing projects byte-for-byte parity.
    pub fn is_empty(&self) -> bool {
        self.issue_tracker.is_none()
            && self.wiki.is_none()
            && self.design.is_none()
            && self.ci.is_none()
            && self.git_host.is_none()
            && self.chat.is_none()
    }
}
```

- [ ] **Step 4: Add the `default_integrations` field to `GlobalConfig`**

In `src/config.rs`, add the field to the `GlobalConfig` struct (after `projects`):

```rust
    #[serde(default)]
    pub projects: Projects,
    /// Global default provider wiring inherited per-field by every project
    /// (the project still owns `repo`). Empty = no defaults (legacy behaviour).
    #[serde(default, skip_serializing_if = "Integrations::is_empty")]
    pub default_integrations: Integrations,
```

And add it to the manual `Default for GlobalConfig` impl:

```rust
impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            schema_version: default_schema(),
            engine: EngineSection::default(),
            defaults: Defaults::default(),
            projects: Projects::default(),
            default_integrations: Integrations::default(),
        }
    }
}
```

- [ ] **Step 5: Add the merge functions**

In `src/config.rs`, add near `resolve_project` (above it is fine):

```rust
/// Merge one capability slot per field: the project wins, an empty project field
/// inherits the global default. `repo` is always the project's (a global default
/// has none). Returns `None` when neither side names a provider.
fn merge_provider(global: &Option<Provider>, project: &Option<Provider>) -> Option<Provider> {
    let pick = |p: Option<&str>, g: Option<&str>| -> Option<String> {
        p.filter(|s| !s.is_empty()).or(g.filter(|s| !s.is_empty())).map(str::to_string)
    };
    let provider = pick(
        project.as_ref().map(|x| x.provider.as_str()),
        global.as_ref().map(|x| x.provider.as_str()),
    )?;
    let base_url = pick(
        project.as_ref().map(|x| x.base_url.as_str()),
        global.as_ref().map(|x| x.base_url.as_str()),
    )
    .unwrap_or_default();
    let repo = project.as_ref().map(|x| x.repo.clone()).unwrap_or_default();
    Some(Provider { provider, base_url, repo })
}

/// Fold global default wiring UNDER a project's explicit integrations.
pub fn merge_integrations(global: &Integrations, project: &Integrations) -> Integrations {
    Integrations {
        issue_tracker: merge_provider(&global.issue_tracker, &project.issue_tracker),
        wiki: merge_provider(&global.wiki, &project.wiki),
        design: merge_provider(&global.design, &project.design),
        ci: merge_provider(&global.ci, &project.ci),
        git_host: merge_provider(&global.git_host, &project.git_host),
        chat: merge_provider(&global.chat, &project.chat),
    }
}
```

- [ ] **Step 6: Wire the merge into `resolve_project`**

In `src/config.rs` `resolve_project`, change the project-config load to merge defaults under it:

```rust
    let mut pc = ProjectConfig::load_from(&entry.repo_path)?;
    pc.integrations = merge_integrations(&global.default_integrations, &pc.integrations);
```

(Replace the existing `let pc = ProjectConfig::load_from(&entry.repo_path)?;` line — the rest of the function uses `pc` unchanged.)

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test config:: 2>&1 | tail -20`
Expected: PASS — the 4 new `merge_*` tests plus all existing `config::tests::*` green.

- [ ] **Step 8: Build clean**

Run: `cargo build 2>&1 | tail -5`
Expected: builds with 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): global default_integrations + per-field merge under projects

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Backend — global view / apply / verify (`credentials.rs`)

**Files:**
- Modify: `src/credentials.rs` (refactor `verify_capability` to extract `run_verify`; add `verify_kind`, `integration_ref`, `global_view_of`/`global_view`, `ConnectorInput`/`apply_connector_secrets`/`apply_global`, `global_verify`)
- Test: `src/credentials.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `config::{GlobalConfig, Integrations, Provider, AuthProfile, Secrets, mask_secret}` (already imported); `clients::{issue_tracker, doc_source, git_host, design_source, ci_provider, chat_notify}`.
- Produces: `credentials::global_view() -> Result<Value, String>`; `credentials::apply_global(cap: &str, body: &str) -> Result<Value, String>`; `credentials::global_verify(cap: &str) -> Result<Value, String>`; `credentials::verify_kind(cap, provider) -> &'static str` (pub).

- [ ] **Step 1: Write the failing tests**

Add to `src/credentials.rs` `mod tests`:

```rust
    #[test]
    fn verify_kind_only_repo_bound_need_a_repo() {
        // confirmed against src/clients/*::verify() on 2026-06-25
        assert_eq!(verify_kind("git_host", "github"), "now");
        assert_eq!(verify_kind("git_host", "gitlab"), "now");
        assert_eq!(verify_kind("issue_tracker", "jira"), "now");
        assert_eq!(verify_kind("issue_tracker", "github_issues"), "repo");
        assert_eq!(verify_kind("wiki", "confluence"), "now");
        assert_eq!(verify_kind("wiki", "notion"), "now");
        assert_eq!(verify_kind("design", "figma"), "now");
        assert_eq!(verify_kind("ci", "jenkins"), "now");
        assert_eq!(verify_kind("ci", "github_actions"), "repo");
        assert_eq!(verify_kind("ci", "gitlab_ci"), "repo");
        assert_eq!(verify_kind("chat", "slack"), "now");
    }

    #[test]
    fn global_view_masks_and_shapes() {
        let defaults = Integrations {
            git_host: Some(Provider {
                provider: "github".into(),
                base_url: "https://api.github.com".into(),
                repo: String::new(),
            }),
            ..Default::default()
        };
        let auth = AuthProfile { git_token: "supersecret".into(), jira_email: "me@x.com".into(), ..Default::default() };
        let v = global_view_of(&defaults, &auth);
        assert!(!v.to_string().contains("supersecret"), "plaintext leaked");
        assert_eq!(v["auth_profile"], "default");
        assert_eq!(v["wiring"]["git_host"]["provider"], "github");
        assert_eq!(v["wiring"]["git_host"]["base_url"], "https://api.github.com");
        assert_eq!(v["wiring"]["wiki"]["provider"], "");
        assert_eq!(v["secrets"]["git_token"], "**** (11 chars)");
        assert_eq!(v["secrets"]["jira_email"], "me@x.com");
        assert!(v["providers"]["git_host"].is_array());
    }

    #[test]
    fn apply_connector_secrets_only_touches_submitted_keys() {
        let mut auth = AuthProfile { git_token: "old".into(), jira_email: "keep@x.com".into(), ..Default::default() };
        let mut m = std::collections::BTreeMap::new();
        // blank token = keep; jira_email NOT submitted → untouched
        m.insert("git_token".to_string(), String::new());
        apply_connector_secrets(&mut auth, &m);
        assert_eq!(auth.git_token, "old");
        assert_eq!(auth.jira_email, "keep@x.com");
        // non-empty token = overwrite
        m.insert("git_token".to_string(), "new".to_string());
        apply_connector_secrets(&mut auth, &m);
        assert_eq!(auth.git_token, "new");
        assert_eq!(auth.jira_email, "keep@x.com");
        // identifier field present = set directly (even to empty)
        let mut e = std::collections::BTreeMap::new();
        e.insert("jira_email".to_string(), "a@b.com".to_string());
        apply_connector_secrets(&mut auth, &e);
        assert_eq!(auth.jira_email, "a@b.com");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test credentials:: 2>&1 | tail -20`
Expected: FAIL (compile error) — `cannot find function 'verify_kind' / 'global_view_of' / 'apply_connector_secrets'`.

- [ ] **Step 3: Extract `run_verify` and simplify `verify_capability`**

In `src/credentials.rs`, replace the body of `verify_capability` and add `run_verify` above it:

```rust
/// Build the capability's client from `pc`+`auth` and run `verify()`, mapping the
/// outcome to JSON (`{ok:true,message}` / `{ok:false,error}`). Unknown cap = `Err`.
fn run_verify(pc: &ProjectConfig, auth: &AuthProfile, cap: &str) -> Result<Value, String> {
    let insecure = false;
    let result = match cap {
        "issue_tracker" => crate::clients::issue_tracker(pc, auth, insecure).and_then(|c| c.verify()),
        "wiki" => crate::clients::doc_source(pc, auth, insecure).and_then(|c| c.verify()),
        "git_host" => crate::clients::git_host(pc, auth, insecure).and_then(|c| c.verify()),
        "design" => crate::clients::design_source(pc, auth, insecure).and_then(|c| c.verify()),
        "ci" => crate::clients::ci_provider(pc, auth, insecure).and_then(|c| c.verify()),
        "chat" => crate::clients::chat_notify(pc, auth, insecure).and_then(|c| c.verify()),
        other => return Err(format!("unknown capability '{other}'")),
    };
    Ok(match result {
        Ok(message) => json!({ "ok": true, "message": message }),
        Err(error) => json!({ "ok": false, "error": error }),
    })
}

/// Build the capability's client from a project's saved config + secrets and
/// `verify()` it. Verify failures are data (`ok:false`), not errors.
pub fn verify_capability(global: &GlobalConfig, name: &str, cap: &str) -> Result<Value, String> {
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get(&pc.auth_profile).cloned().unwrap_or_default();
    run_verify(&pc, &auth, cap)
}
```

- [ ] **Step 4: Add `verify_kind` + `integration_ref`**

In `src/credentials.rs`, add (near `integration_slot`):

```rust
/// Read-only sibling of `integration_slot`.
fn integration_ref<'a>(i: &'a Integrations, cap: &str) -> Option<&'a Option<Provider>> {
    Some(match cap {
        "issue_tracker" => &i.issue_tracker,
        "wiki" => &i.wiki,
        "git_host" => &i.git_host,
        "design" => &i.design,
        "ci" => &i.ci,
        "chat" => &i.chat,
        _ => return None,
    })
}

/// Which verify path a (capability, provider) takes from the GLOBAL page:
/// `"repo"` reads `self.repo` so it can only be checked from a project; everything
/// else is verifiable in-place. Confirmed against `src/clients/*::verify()`:
/// git_host/gitlab → `/user`, jenkins → `/me/api/json`, slack → local check — all
/// repo-free; only github_issues / github_actions / gitlab_ci parse a repo.
pub fn verify_kind(cap: &str, provider: &str) -> &'static str {
    match (cap, provider) {
        ("issue_tracker", "github_issues") => "repo",
        ("ci", "github_actions") | ("ci", "gitlab_ci") => "repo",
        _ => "now",
    }
}
```

- [ ] **Step 5: Add `global_view_of` + `global_view`**

In `src/credentials.rs`, add:

```rust
/// Masked, browser-safe view of global default wiring + the `default` auth profile.
fn global_view_of(defaults: &Integrations, auth: &AuthProfile) -> Value {
    let wire = |o: &Option<Provider>| match o {
        Some(p) => json!({ "provider": p.provider, "base_url": p.base_url }),
        None => json!({ "provider": "", "base_url": "" }),
    };
    json!({
        "auth_profile": "default",
        "providers": supported_providers(),
        "wiring": {
            "issue_tracker": wire(&defaults.issue_tracker),
            "wiki": wire(&defaults.wiki),
            "git_host": wire(&defaults.git_host),
            "design": wire(&defaults.design),
            "ci": wire(&defaults.ci),
            "chat": wire(&defaults.chat),
        },
        "secrets": {
            "jira_token": mask_secret(&auth.jira_token),
            "jira_email": auth.jira_email,
            "wiki_token": mask_secret(&auth.wiki_token),
            "wiki_email": auth.wiki_email,
            "figma_token": mask_secret(&auth.figma_token),
            "jenkins_user": auth.jenkins_user,
            "jenkins_token": mask_secret(&auth.jenkins_token),
            "git_token": mask_secret(&auth.git_token),
            "chat_webhook": mask_secret(&auth.chat_webhook),
        },
    })
}

/// The Connectors page read model (global default wiring + `default` secrets).
pub fn global_view() -> Result<Value, String> {
    let global = GlobalConfig::load_or_default()?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get("default").cloned().unwrap_or_default();
    Ok(global_view_of(&global.default_integrations, &auth))
}
```

- [ ] **Step 6: Add `ConnectorInput` + `apply_connector_secrets` + `apply_global`**

In `src/credentials.rs`, add:

```rust
#[derive(Deserialize, Default)]
struct ConnectorInput {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    base_url: String,
    /// Only the fields this connector owns are submitted (so other connectors'
    /// identifiers are never cleared). Token blank = keep; identifier present = set.
    #[serde(default)]
    secrets: BTreeMap<String, String>,
}

/// Apply only the submitted secret keys. Tokens overwrite when non-empty
/// (blank = keep); identifier fields (email/user) set directly when present.
fn apply_connector_secrets(auth: &mut AuthProfile, secrets: &BTreeMap<String, String>) {
    for (k, v) in secrets {
        let v = v.as_str();
        match k.as_str() {
            "jira_email" => auth.jira_email = v.to_string(),
            "wiki_email" => auth.wiki_email = v.to_string(),
            "jenkins_user" => auth.jenkins_user = v.to_string(),
            "jira_token" => if !v.is_empty() { auth.jira_token = v.to_string() },
            "wiki_token" => if !v.is_empty() { auth.wiki_token = v.to_string() },
            "figma_token" => if !v.is_empty() { auth.figma_token = v.to_string() },
            "jenkins_token" => if !v.is_empty() { auth.jenkins_token = v.to_string() },
            "git_token" => if !v.is_empty() { auth.git_token = v.to_string() },
            "chat_webhook" => if !v.is_empty() { auth.chat_webhook = v.to_string() },
            _ => {}
        }
    }
}

/// Save ONE connector globally: default wiring → `~/.palugada.yaml`
/// (`default_integrations.<cap>`, `(none)`/blank clears); tokens → the `default`
/// auth profile in `secrets.yaml`.
pub fn apply_global(cap: &str, body: &str) -> Result<Value, String> {
    let inp: ConnectorInput = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let mut global = GlobalConfig::load_or_default()?;
    {
        let slot = integration_slot(&mut global.default_integrations, cap)
            .ok_or_else(|| format!("unknown capability '{cap}'"))?;
        let prov = inp.provider.trim();
        if prov.is_empty() || prov == "(none)" {
            *slot = None;
        } else {
            *slot = Some(Provider {
                provider: prov.to_string(),
                base_url: inp.base_url.trim().to_string(),
                repo: String::new(),
            });
        }
    }
    global.save()?;
    if !inp.secrets.is_empty() {
        let mut secrets = Secrets::load_or_default()?;
        let auth = secrets.auth_profiles.entry("default".to_string()).or_default();
        apply_connector_secrets(auth, &inp.secrets);
        secrets.save()?;
    }
    Ok(json!({ "ok": true, "cap": cap }))
}
```

- [ ] **Step 7: Add `global_verify`**

In `src/credentials.rs`, add:

```rust
/// Verify a connector from the GLOBAL page. Repo-bound (cap,provider) pairs return
/// `needs_repo` without a network call; the rest build an ephemeral project config
/// from the global defaults and run the real `verify()`.
pub fn global_verify(cap: &str) -> Result<Value, String> {
    let global = GlobalConfig::load_or_default()?;
    let slot = integration_ref(&global.default_integrations, cap)
        .ok_or_else(|| format!("unknown capability '{cap}'"))?;
    let provider = slot.as_ref().map(|p| p.provider.as_str()).unwrap_or("");
    if provider.is_empty() {
        return Ok(json!({ "ok": false, "error": "no provider configured" }));
    }
    if verify_kind(cap, provider) == "repo" {
        return Ok(json!({ "ok": false, "needs_repo": true, "message": "verify from a project" }));
    }
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get("default").cloned().unwrap_or_default();
    let pc = ProjectConfig { integrations: global.default_integrations.clone(), ..Default::default() };
    run_verify(&pc, &auth, cap)
}
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test credentials:: 2>&1 | tail -20`
Expected: PASS — the 3 new tests plus all existing `credentials::tests::*` green.

- [ ] **Step 9: Build clean**

Run: `cargo build 2>&1 | tail -5`
Expected: 0 warnings. (If an unused-import warning for `BTreeMap` appears, it is already imported at the top of `credentials.rs` — do not re-add.)

- [ ] **Step 10: Commit**

```bash
git add src/credentials.rs
git commit -m "feat(credentials): global connectors view/apply/verify (default auth-profile)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Routes — three global `/api/connectors` endpoints (`web.rs`)

**Files:**
- Modify: `src/web.rs` (`Route` enum, `route()`, `api()` dispatch, `route_parses_paths` test)

**Interfaces:**
- Consumes: `credentials::{global_view, apply_global, global_verify}` from Task 2.
- Produces: `Route::{Connectors, SaveConnector(String), VerifyConnector(String)}`.

- [ ] **Step 1: Write the failing test additions**

Add to `src/web.rs` `route_parses_paths`:

```rust
        assert_eq!(route("GET", "/api/connectors"), Route::Connectors);
        assert_eq!(route("POST", "/api/connectors/git_host"), Route::SaveConnector("git_host".into()));
        assert_eq!(
            route("POST", "/api/connectors/git_host/verify"),
            Route::VerifyConnector("git_host".into())
        );
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test route_parses_paths 2>&1 | tail -20`
Expected: FAIL (compile error) — `no variant 'Connectors' / 'SaveConnector' / 'VerifyConnector'`.

- [ ] **Step 3: Add the `Route` variants**

In `src/web.rs`, add to the `enum Route` (before `NotFound`):

```rust
    Connectors,
    SaveConnector(String),
    VerifyConnector(String),
    NotFound,
```

- [ ] **Step 4: Add the route patterns**

In `src/web.rs` `route()`, add after the `("GET", ["api", "profiles"])` arm:

```rust
        ("GET", ["api", "connectors"]) => Route::Connectors,
        ("POST", ["api", "connectors", cap]) => Route::SaveConnector((*cap).to_string()),
        ("POST", ["api", "connectors", cap, "verify"]) => Route::VerifyConnector((*cap).to_string()),
```

- [ ] **Step 5: Add the API dispatch**

In `src/web.rs` `api()`, add before the final `_ => (501, ...)` arm:

```rust
        Route::Connectors => read(crate::credentials::global_view),
        Route::SaveConnector(cap) => write_op(|| crate::credentials::apply_global(&cap, body)),
        Route::VerifyConnector(cap) => read(|| crate::credentials::global_verify(&cap)),
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test web:: 2>&1 | tail -20`
Expected: PASS — `route_parses_paths` and all `web::tests::*` green.

- [ ] **Step 7: Build clean**

Run: `cargo build 2>&1 | tail -5`
Expected: 0 warnings.

- [ ] **Step 8: Commit**

```bash
git add src/web.rs
git commit -m "feat(web): global /api/connectors routes (view/save/verify)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Frontend — Connectors nav, view, and styles

**Files:**
- Modify: `src/web/index.html` (sidebar nav item)
- Modify: `src/web/style.css` (connector-card classes, append at end)
- Modify: `src/web/app.js` (`VIEWS` entry + `renderConnectors`/`connectorCard` and helpers)

**Interfaces:**
- Consumes: `GET /api/connectors`, `POST /api/connectors/{cap}`, `POST /api/connectors/{cap}/verify` from Task 3; existing JS helpers `api`, `toast`, `h`, `esc`, `viewHead`.
- Produces: a `connectors` view registered in `VIEWS`.

- [ ] **Step 1: Add the sidebar nav item**

In `src/web/index.html`, add after the `knowledge` nav item (inside `<nav class="sidebar">`):

```html
      <a class="nav-item" data-view="connectors"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M9 7H6a3 3 0 0 0 0 6h3M15 7h3a3 3 0 0 1 0 6h-3M8 10h8"/></svg>Connectors</a>
```

- [ ] **Step 2: Append the connector-card styles**

Append to `src/web/style.css` (reuses existing tokens; no palette change):

```css
/* ── connectors view ── */
.profile-chip{display:inline-flex;align-items:center;gap:8px;margin:0 0 var(--s5);
  font-family:var(--font-ui);font-weight:700;font-size:13.5px;color:var(--ink);
  background:var(--surface);border:var(--bw) solid var(--ink);border-radius:var(--r-pill);
  padding:5px 13px;box-shadow:var(--shadow-sm);}
.profile-chip .lbl{color:var(--ink-soft);}
.profile-chip .soon{font-family:var(--font-mono);font-size:11px;color:var(--faint);}
.cx{padding:var(--s5);}
.cx-head{display:flex;align-items:center;gap:var(--s3);flex-wrap:wrap;cursor:pointer;}
.cx-ic{width:40px;height:40px;flex-shrink:0;display:grid;place-items:center;border:var(--bw) solid var(--ink);
  border-radius:var(--r-sm);background:var(--surface-2);box-shadow:var(--shadow-sm);}
.cx-ic svg{width:22px;height:22px;stroke:var(--ink);}
.cx-title{font-family:var(--font-display);font-weight:400;font-size:23px;letter-spacing:.02em;line-height:1;color:var(--ink);}
.cx-cap{font-family:var(--font-mono);font-size:12px;color:var(--ink-soft);margin-top:3px;}
.cx-spacer{flex:1;}
.status{display:inline-flex;align-items:center;gap:6px;font-family:var(--font-display);font-size:14px;letter-spacing:.03em;
  border:var(--bw) solid var(--ink);border-radius:var(--r-pill);padding:1px 11px;color:var(--ink);transform:rotate(-1deg);white-space:nowrap;}
.status .dot{width:8px;height:8px;border-radius:50%;border:1px solid var(--ink);}
.status.ok{background:var(--conv-bg);color:var(--conv);}.status.ok .dot{background:var(--conv);}
.status.info{background:var(--rev-bg);color:var(--rev);}.status.info .dot{background:var(--rev);}
.status.off{background:transparent;border-style:dashed;color:var(--faint);}.status.off .dot{background:transparent;}
.cx-expand{margin-left:6px;background:none;border:none;box-shadow:none;color:var(--ink);padding:4px;cursor:pointer;line-height:0;}
.cx-expand:hover{background:none;transform:none;box-shadow:none;}
.cx-expand svg{width:20px;height:20px;stroke:var(--ink);transition:transform .15s;}
.cx.collapsed .cx-expand svg{transform:rotate(-90deg);}
.cx-powers{margin:10px 0 0;font-size:13px;color:var(--ink-soft);}
.cx-powers code{color:var(--ink);background:var(--surface-2);border:1.5px solid var(--ink);padding:0 5px;border-radius:5px;font-size:12px;}
.cx-body{margin-top:var(--s4);border-top:2px dashed var(--surface-2);padding-top:var(--s4);}
.cx.collapsed .cx-body,.cx.collapsed .cx-powers{display:none;}
.cx-fields{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:var(--s4);}
.cx-fields .full{grid-column:1 / -1;}
.key-wrap{display:flex;gap:var(--s2);align-items:stretch;}
.key-wrap input{flex:1;font-family:var(--font-mono);font-size:13.5px;}
.reveal{flex-shrink:0;font-family:var(--font-ui);font-weight:700;font-size:12px;padding:0 12px;background:var(--surface);color:var(--ink);}
.reveal:hover{background:var(--pow);}
.cx-actions{display:flex;align-items:center;gap:var(--s3);margin-top:var(--s4);flex-wrap:wrap;}
.vres{font-family:var(--font-mono);font-size:12.5px;font-weight:700;}
.vres.ok{color:var(--ok);}.vres.info{color:var(--rev);}.vres.muted{color:var(--ink-soft);}.vres.err{color:var(--err);}
.locked-note{font-size:12.5px;color:var(--faint);}
@media (max-width:880px){.cx-fields{grid-template-columns:1fr;}}
```

- [ ] **Step 3: Register the view**

In `src/web/app.js`, change the `VIEWS` map (line ~272) to add `connectors`:

```js
const VIEWS = { overview: renderOverview, projects: renderProjects, profiles: renderProfiles, knowledge: renderKnowledge, connectors: renderConnectors };
```

- [ ] **Step 4: Add the Connectors view + card**

In `src/web/app.js`, add (place near the other views, e.g. after `credentialsCard`):

```js
// ── connectors (global setup) ──────────────────────────────────────────────
const CX = [
  { cap: "git_host", title: "Git Host", powers: ["pr", "git"],
    icon: '<path d="M9 7H6a3 3 0 0 0 0 6h3M15 7h3a3 3 0 0 1 0 6h-3M8 10h8"/>' },
  { cap: "issue_tracker", title: "Issue Tracker", powers: ["issue"],
    icon: '<path d="M9 11l3 3 7-7"/><path d="M21 12a9 9 0 1 1-6.2-8.5"/>' },
  { cap: "wiki", title: "Docs & Wiki", powers: ["wiki", "prd"],
    icon: '<path d="M4 5a2 2 0 0 1 2-2h12v18H6a2 2 0 0 1-2-2z"/><path d="M8 7h7M8 11h7"/>' },
  { cap: "ci", title: "CI / Pipelines", powers: ["ci"],
    icon: '<path d="M5 12h4l2 5 3-10 2 5h3"/>' },
  { cap: "design", title: "Design", powers: ["design"],
    icon: '<circle cx="12" cy="12" r="3"/><path d="M12 3v3M12 18v3M3 12h3M18 12h3"/>' },
  { cap: "chat", title: "Chat & Notify", powers: ["notify"],
    icon: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>' },
];

// Mirrors credentials::verify_kind on the backend (kept here so provider changes
// re-classify live before save).
function verifyKind(cap, provider) {
  if (cap === "issue_tracker" && provider === "github_issues") return "repo";
  if (cap === "ci" && (provider === "github_actions" || provider === "gitlab_ci")) return "repo";
  return "now";
}

// Which base_url / identifier / token fields a (cap, provider) shows.
function cxFields(cap, provider) {
  const F = {
    "git_host": { base: { hint: "blank = api.github.com (GitHub) / gitlab.com (GitLab)" }, token: { k: "git_token", l: "API token (git_token)" } },
    "issue_tracker:jira": { base: { hint: "e.g. https://you.atlassian.net", req: true }, email: { k: "jira_email", l: "Account email" }, token: { k: "jira_token", l: "API token (jira_token)" } },
    "issue_tracker:github_issues": { inherits: "git_token" },
    "wiki:confluence": { base: { hint: "e.g. https://you.atlassian.net/wiki", req: true }, email: { k: "wiki_email", l: "Account email" }, token: { k: "wiki_token", l: "API token (wiki_token)" } },
    "wiki:notion": { token: { k: "wiki_token", l: "API token (wiki_token)" } },
    "design": { token: { k: "figma_token", l: "API token (figma_token)" } },
    "ci:jenkins": { base: { hint: "e.g. https://ci.you.com", req: true }, email: { k: "jenkins_user", l: "Username (jenkins_user)" }, token: { k: "jenkins_token", l: "API token (jenkins_token)" } },
    "ci:github_actions": { inherits: "git_token" },
    "ci:gitlab_ci": { inherits: "git_token" },
    "chat": { token: { k: "chat_webhook", l: "Webhook URL (chat_webhook)" } },
  };
  return F[cap + ":" + provider] || F[cap] || {};
}

function cxStatus(cap, provider, f, sec) {
  if (!provider) return ["off", "Not set"];
  if (f.inherits) return ["info", "Uses Git Host"];
  if (verifyKind(cap, provider) === "repo") return ["info", "Verify in project"];
  const k = f.token && f.token.k;
  const has = k && sec[k] && sec[k] !== "(unset)";
  return has ? ["ok", "Configured"] : ["off", "Key needed"];
}

async function renderConnectors() {
  view.innerHTML = viewHead("Setup", "Connectors & API Keys",
    "Set your API keys and default wiring once. Keys live globally in <code>~/.palugada/secrets.yaml</code> (chmod 600, never shown in full). Projects inherit this — they only set their own <code>repo</code>.");
  let d;
  try { d = await api("/api/connectors"); }
  catch (e) { toast(e.message, true); return; }
  view.appendChild(h(`<div class="profile-chip"><span class="lbl">Auth profile</span> <span class="id-chip">${esc(d.auth_profile || "default")}</span> <span class="soon">multi-profile soon</span></div>`));
  let configured = 0, repoReady = 0, notset = 0;
  CX.forEach(c => {
    const w = (d.wiring && d.wiring[c.cap]) || { provider: "" };
    if (!w.provider) { notset++; return; }
    if (verifyKind(c.cap, w.provider) === "repo") repoReady++;
    configured++;
  });
  view.appendChild(h(`<div class="stat-grid">
    <div class="stat"><div class="k">Connectors</div><div class="v">${CX.length}</div></div>
    <div class="stat"><div class="k">Configured</div><div class="v">${configured}</div></div>
    <div class="stat"><div class="k">Verify in project</div><div class="v">${repoReady}</div></div>
    <div class="stat"><div class="k">Not set</div><div class="v">${notset}</div></div>
  </div>`));
  CX.forEach(c => view.appendChild(connectorCard(c, d)));
  view.appendChild(h(`<div class="card" style="box-shadow:var(--shadow-sm)">
    <div class="card-head"><h3>How keys are stored</h3></div>
    <p class="card-note">Tokens are written to <code>~/.palugada/secrets.yaml</code> (<code>0600</code>) and never sent back to the browser — you only see <code>•••• (N chars)</code>. Blank keeps the existing value. Provider &amp; base URL save as global defaults; each project still sets its own repo.</p></div>`));
}

function connectorCard(c, d) {
  const w = (d.wiring && d.wiring[c.cap]) || { provider: "", base_url: "" };
  const sec = d.secrets || {};
  const provList = ["(none)", ...(((d.providers && d.providers[c.cap]) || []))];
  let provider = w.provider || "(none)";
  const card = h(`<div class="card cx collapsed" data-cap="${esc(c.cap)}">
    <div class="cx-head">
      <span class="cx-ic"><svg viewBox="0 0 24 24" fill="none" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round">${c.icon}</svg></span>
      <div><div class="cx-title">${esc(c.title)}</div><div class="cx-cap"></div></div>
      <span class="cx-spacer"></span>
      <span class="status"></span>
      <button class="cx-expand" type="button" aria-label="Expand"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><path d="M6 9l6 6 6-6"/></svg></button>
    </div>
    <p class="cx-powers">Powers ${c.powers.map(p => `<code>palugada ${esc(p)}</code>`).join(" ")}</p>
    <div class="cx-body"></div></div>`);
  const capEl = card.querySelector(".cx-cap");
  const statusEl = card.querySelector(".status");
  const body = card.querySelector(".cx-body");

  function paintHead() {
    const prov = provider === "(none)" ? "" : provider;
    capEl.textContent = c.cap + (prov ? " · " + prov : "");
    const [cls, text] = cxStatus(c.cap, prov, cxFields(c.cap, prov), sec);
    statusEl.className = "status " + cls;
    statusEl.innerHTML = `<span class="dot"></span>${esc(text)}`;
  }
  function paintBody() {
    const prov = provider === "(none)" ? "" : provider;
    const f = cxFields(c.cap, prov);
    const provOpts = provList.map(p => `<option${provider === p ? " selected" : ""}>${esc(p)}</option>`).join("");
    let html = `<div class="cx-fields"><div class="field"><label>Provider</label><select class="cx-prov">${provOpts}</select></div>`;
    if (f.base) html += `<div class="field"><label>Base URL${f.base.req ? " (required)" : ""}</label><input class="cx-base" value="${esc(w.base_url || "")}" placeholder="${esc(f.base.hint || "")}"></div>`;
    if (f.email) html += `<div class="field"><label>${esc(f.email.l)}</label><input class="cx-email" data-k="${esc(f.email.k)}" value="${esc(sec[f.email.k] || "")}"></div>`;
    if (f.inherits) html += `<div class="field full"><label>API key</label><div class="locked-note">↳ Inherited from <strong>Git Host</strong> (<code>${esc(f.inherits)}</code>). No separate key needed.</div></div>`;
    else if (f.token) {
      const masked = sec[f.token.k] || "(unset)"; const has = masked !== "(unset)";
      html += `<div class="field full"><label>${esc(f.token.l)}</label>
        <div class="key-wrap"><input class="cx-token" data-k="${esc(f.token.k)}" type="password" placeholder="${esc(has ? masked + " · blank = keep" : "Paste token…")}"><button class="reveal" type="button">Show</button></div></div>`;
    }
    html += `</div><div class="cx-actions"><button class="btn cx-save" type="button">Save ${esc(c.title)}</button>`;
    if (verifyKind(c.cap, prov) === "repo") html += `<span class="vres info">↳ needs a repo — verify from a project</span>`;
    else if (prov) html += `<button class="btn secondary cx-verify" type="button">Verify</button> <span class="vres"></span>`;
    html += `</div>`;
    body.innerHTML = html;
    body.querySelector(".cx-prov").onchange = e => { provider = e.target.value; paintHead(); paintBody(); };
    const rv = body.querySelector(".reveal");
    if (rv) rv.onclick = () => {
      const inp = body.querySelector(".cx-token");
      const show = inp.type === "password"; inp.type = show ? "text" : "password"; rv.textContent = show ? "Hide" : "Show";
    };
    body.querySelector(".cx-save").onclick = saveConnector;
    const vb = body.querySelector(".cx-verify");
    if (vb) vb.onclick = verifyConnector;
  }
  async function saveConnector() {
    const payload = { provider: provider, base_url: "", secrets: {} };
    const be = body.querySelector(".cx-base"); if (be) payload.base_url = be.value;
    const ee = body.querySelector(".cx-email"); if (ee) payload.secrets[ee.dataset.k] = ee.value;
    const te = body.querySelector(".cx-token"); if (te) payload.secrets[te.dataset.k] = te.value;
    try { await api(`/api/connectors/${c.cap}`, "POST", payload); toast("Saved " + c.title); renderConnectors(); }
    catch (e) { toast(e.message, true); }
  }
  async function verifyConnector() {
    const res = body.querySelector(".vres");
    res.textContent = "…"; res.className = "vres muted";
    try {
      const r = await api(`/api/connectors/${c.cap}/verify`, "POST", {});
      if (r.needs_repo) { res.textContent = "↳ " + (r.message || "verify from a project"); res.className = "vres info"; }
      else { res.textContent = (r.ok ? "✓ " : "✗ ") + (r.message || r.error || ""); res.className = "vres " + (r.ok ? "ok" : "err"); }
    } catch (e) { res.textContent = "✗ " + e.message; res.className = "vres err"; }
  }
  card.querySelector(".cx-head").onclick = e => { if (!e.target.closest(".cx-prov")) card.classList.toggle("collapsed"); };
  paintHead(); paintBody();
  return card;
}
```

- [ ] **Step 5: Syntax-check the JS**

Run: `node --check src/web/app.js && echo OK`
Expected: `OK` (no syntax errors).

- [ ] **Step 6: Build (embeds the assets) and run the full suite**

Run: `cargo build 2>&1 | tail -5 && cargo test 2>&1 | tail -8`
Expected: build 0 warnings; all tests pass (no count regression).

- [ ] **Step 7: Commit**

```bash
git add src/web/index.html src/web/style.css src/web/app.js
git commit -m "feat(web): Connectors view — global API keys + default wiring (accordion)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Integration verification (isolated HOME, live curl)

**Files:** none (verification only)

**Why isolated HOME:** the global routes read/write the REAL `~/.palugada.yaml` and `~/.palugada/secrets.yaml`. Point `HOME` at a throwaway dir so the e2e never touches the user's actual config or tokens.

- [ ] **Step 1: Start the web server against a scratch HOME**

```bash
SCRATCH=$(mktemp -d)
HOME="$SCRATCH" cargo run --quiet -- web --port 7799 &
WEB_PID=$!
sleep 2
```

- [ ] **Step 2: Read the empty global view**

```bash
curl -s localhost:7799/api/connectors | python3 -m json.tool
```
Expected: `auth_profile: "default"`, every `wiring.*.provider` is `""`, every `secrets.*` token is `"(unset)"`, `providers.git_host` is `["github","gitlab"]`.

- [ ] **Step 3: Save a connector (wiring + token), then prove masking**

```bash
curl -s -X POST localhost:7799/api/connectors/git_host \
  -H 'Content-Type: application/json' \
  -d '{"provider":"github","base_url":"","secrets":{"git_token":"ghp_FAKE123456"}}'
echo
curl -s localhost:7799/api/connectors | python3 -c 'import sys,json;d=json.load(sys.stdin);print("wiring:",d["wiring"]["git_host"]);print("git_token:",d["secrets"]["git_token"])'
```
Expected: save returns `{"ok":true,"cap":"git_host"}`; re-read shows `wiring git_host provider=github` and `git_token: **** (13 chars)` (never the plaintext).

- [ ] **Step 4: Prove blank = keep**

```bash
curl -s -X POST localhost:7799/api/connectors/git_host \
  -H 'Content-Type: application/json' \
  -d '{"provider":"github","base_url":"","secrets":{"git_token":""}}'
echo
curl -s localhost:7799/api/connectors | python3 -c 'import sys,json;print("git_token:",json.load(sys.stdin)["secrets"]["git_token"])'
```
Expected: `git_token: **** (13 chars)` still — the blank submit kept the stored value.

- [ ] **Step 5: Verify classification (repo-bound vs in-place)**

```bash
# repo-bound: github_issues → needs_repo, NO network call
curl -s -X POST localhost:7799/api/connectors/issue_tracker \
  -H 'Content-Type: application/json' -d '{"provider":"github_issues"}' >/dev/null
curl -s -X POST localhost:7799/api/connectors/issue_tracker/verify | python3 -m json.tool
# in-place: git_host(github) → runs real verify (fails auth with the fake token, which proves the path executed)
curl -s -X POST localhost:7799/api/connectors/git_host/verify | python3 -m json.tool
```
Expected: issue_tracker verify → `{"ok":false,"needs_repo":true,"message":"verify from a project"}`; git_host verify → `{"ok":false,"error":"..."}` (a real auth/network error from `/user`), confirming the in-place path ran.

- [ ] **Step 6: Confirm the page serves**

```bash
curl -s -o /dev/null -w "index:%{http_code} app.js:" localhost:7799/ ; curl -s -o /dev/null -w "%{http_code}\n" localhost:7799/app.js
grep -c "renderConnectors" <(curl -s localhost:7799/app.js)
```
Expected: `index:200 app.js:200` and the grep count ≥ 2 (the served app.js includes the new view).

- [ ] **Step 7: Stop the server and clean up**

```bash
kill "$WEB_PID" 2>/dev/null
rm -rf "$SCRATCH"
```

- [ ] **Step 8: Manual browser click-through (record result)**

Reinstall (`cargo install --path . --force`) or `cargo run -- web`, open the console, click **Connectors**, expand a card, change a provider (watch the fields + status re-render), toggle **Show** on a key, Save, and Verify a repo-free connector. Note the outcome in the PR/commit description. (Frontend has no unit tests by repo convention; this is the human gate.)

---

## Self-Review

**Spec coverage:**
- §1 config model (`default_integrations`, `merge_integrations`, `resolve_project`) → Task 1. ✓
- §2 backend (`global_view`, `apply_global`, `global_verify`) → Task 2. ✓
- §3 verify classification (`verify_kind`, repo vs in-place) → Task 2 (Steps 4,7) + Task 5 (Step 5). ✓
- §4 capability→fields→secret mapping (provider-aware) → Task 4 (`cxFields`). ✓
- §5 routes → Task 3. ✓
- §6 frontend (nav, `renderConnectors`, accordion, masking, reveal, security note) → Task 4. ✓
- §Testing → unit tests in Tasks 1–3, e2e + node-check in Tasks 4–5. ✓
- §Security (masking, blank=keep, loopback, verify-only network) → preserved in Task 2; proven in Task 5 Steps 3–5. ✓
- §7 known nuance (project editor unchanged) → respected; `verify_capability` kept on raw project config. ✓

**Placeholder scan:** none — every step has concrete code/commands and expected output.

**Type consistency:** `merge_integrations(&Integrations,&Integrations)->Integrations`, `verify_kind(&str,&str)->&'static str`, `global_view()->Result<Value,String>`, `apply_global(&str,&str)`, `global_verify(&str)`, `Route::{Connectors,SaveConnector(String),VerifyConnector(String)}`, and JS `verifyKind`/`cxFields`/`cxStatus`/`connectorCard` names match across tasks. The JS `verifyKind` deliberately mirrors the Rust `verify_kind` (noted inline). ✓
