# Per-project Credentials & Integrations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Edit a project's integrations (provider/base_url/repo) and the bound auth-profile's tokens from `palugada web`, with masked write-only secrets and a per-integration Verify button.

**Architecture:** A new `src/credentials.rs` splits pure transforms (view/apply — unit-tested) from I/O wrappers (load config + secrets, save, verify via `clients::*`). Three endpoints feed a "Credentials & Integrations" editor on the existing per-project detail page; saving re-renders so the skill-flow map's tool-skill gating updates.

**Tech Stack:** Rust (serde, serde_yaml, tiny_http), vanilla JS.

Spec: `docs/superpowers/specs/2026-06-16-web-credentials-design.md`

---

## File structure

| File | Action |
|---|---|
| `src/credentials.rs` | Create — providers, view, apply, save, verify + tests |
| `src/main.rs` | `mod credentials;` |
| `src/web.rs` | 3 routes + handlers + route test |
| `src/web/app.js` | credentials editor in `renderProjectDetail` |
| `src/web/style.css` | verify badge / form rows |

---

## Task 1: `src/credentials.rs` pure core (TDD)

**Files:**
- Create: `src/credentials.rs`
- Modify: `src/main.rs` (`mod credentials;`)

- [ ] **Step 1: Create `src/credentials.rs` with pure transforms + tests**

```rust
//! Per-project credential & integration editing for the web console. Pure
//! transforms (view/apply) are split from the I/O wrappers so they can be
//! unit-tested without touching the real config/secrets files.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{json, Value};

use crate::config::{mask_secret, AuthProfile, GlobalConfig, Integrations, ProjectConfig, Provider, Secrets};

/// Capability → allowed provider names. Mirrors the `clients::*` factories.
pub fn supported_providers() -> Value {
    json!({
        "issue_tracker": ["jira", "github_issues"],
        "wiki": ["confluence", "notion"],
        "git_host": ["github", "gitlab"],
        "design": ["figma"],
        "ci": ["jenkins", "github_actions", "gitlab_ci"],
        "chat": ["slack"],
    })
}

fn provider_json(o: &Option<Provider>) -> Value {
    match o {
        Some(p) => json!({ "provider": p.provider, "base_url": p.base_url, "repo": p.repo }),
        None => Value::Null,
    }
}

/// Masked, browser-safe view of a project's config + bound auth profile.
fn config_view(name: &str, pc: &ProjectConfig, auth: &AuthProfile) -> Value {
    let i = &pc.integrations;
    json!({
        "project": name,
        "profile": pc.profile,
        "auth_profile": pc.auth_profile,
        "integrations": {
            "issue_tracker": provider_json(&i.issue_tracker),
            "wiki": provider_json(&i.wiki),
            "git_host": provider_json(&i.git_host),
            "design": provider_json(&i.design),
            "ci": provider_json(&i.ci),
            "chat": provider_json(&i.chat),
        },
        "providers": supported_providers(),
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

#[derive(Deserialize, Default)]
struct ProviderInput {
    #[serde(default)]
    provider: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    repo: String,
}

#[derive(Deserialize, Default)]
struct SecretInput {
    #[serde(default)]
    jira_token: String,
    #[serde(default)]
    jira_email: String,
    #[serde(default)]
    wiki_token: String,
    #[serde(default)]
    wiki_email: String,
    #[serde(default)]
    figma_token: String,
    #[serde(default)]
    jenkins_user: String,
    #[serde(default)]
    jenkins_token: String,
    #[serde(default)]
    git_token: String,
    #[serde(default)]
    chat_webhook: String,
}

#[derive(Deserialize, Default)]
struct ConfigPayload {
    #[serde(default)]
    auth_profile: Option<String>,
    #[serde(default)]
    integrations: BTreeMap<String, ProviderInput>,
    #[serde(default)]
    secrets: Option<SecretInput>,
}

fn integration_slot<'a>(i: &'a mut Integrations, cap: &str) -> Option<&'a mut Option<Provider>> {
    Some(match cap {
        "issue_tracker" => &mut i.issue_tracker,
        "wiki" => &mut i.wiki,
        "git_host" => &mut i.git_host,
        "design" => &mut i.design,
        "ci" => &mut i.ci,
        "chat" => &mut i.chat,
        _ => return None,
    })
}

/// Set `auth_profile` + each integration on `pc` (provider `(none)`/blank clears;
/// a capability absent from the payload is left as-is).
fn apply_integrations(pc: &mut ProjectConfig, auth_profile: &str, payload: &ConfigPayload) {
    pc.auth_profile = auth_profile.to_string();
    for (cap, inp) in &payload.integrations {
        let Some(slot) = integration_slot(&mut pc.integrations, cap) else { continue };
        let prov = inp.provider.trim();
        if prov.is_empty() || prov == "(none)" {
            *slot = None;
        } else {
            *slot = Some(Provider {
                provider: prov.to_string(),
                base_url: inp.base_url.trim().to_string(),
                repo: inp.repo.trim().to_string(),
            });
        }
    }
}

/// Overwrite non-secret identifiers directly; overwrite secret tokens only when
/// the submitted value is non-empty (blank = leave unchanged).
fn apply_secrets(auth: &mut AuthProfile, s: Option<&SecretInput>) {
    let Some(s) = s else { return };
    auth.jira_email = s.jira_email.clone();
    auth.wiki_email = s.wiki_email.clone();
    auth.jenkins_user = s.jenkins_user.clone();
    let set = |dst: &mut String, v: &str| {
        if !v.is_empty() {
            *dst = v.to_string();
        }
    };
    set(&mut auth.jira_token, &s.jira_token);
    set(&mut auth.wiki_token, &s.wiki_token);
    set(&mut auth.figma_token, &s.figma_token);
    set(&mut auth.jenkins_token, &s.jenkins_token);
    set(&mut auth.git_token, &s.git_token);
    set(&mut auth.chat_webhook, &s.chat_webhook);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_view_masks_tokens() {
        let pc = ProjectConfig { profile: "p".into(), auth_profile: "default".into(), ..Default::default() };
        let auth = AuthProfile { jira_token: "supersecret".into(), jira_email: "me@x.com".into(), ..Default::default() };
        let v = config_view("app", &pc, &auth);
        assert!(!v.to_string().contains("supersecret"), "plaintext token leaked");
        assert_ne!(v["secrets"]["jira_token"], "supersecret");
        assert_eq!(v["secrets"]["jira_email"], "me@x.com");
        assert_eq!(v["secrets"]["git_token"], "(unset)");
    }

    #[test]
    fn apply_integrations_set_and_clear() {
        let mut pc = ProjectConfig::default();
        let mut payload = ConfigPayload::default();
        payload.integrations.insert("git_host".into(), ProviderInput {
            provider: "github".into(), base_url: "https://api.github.com".into(), repo: "o/n".into(),
        });
        apply_integrations(&mut pc, "default", &payload);
        let g = pc.integrations.git_host.as_ref().unwrap();
        assert_eq!(g.provider, "github");
        assert_eq!(g.repo, "o/n");
        assert_eq!(pc.auth_profile, "default");

        let mut clear = ConfigPayload::default();
        clear.integrations.insert("git_host".into(), ProviderInput { provider: "(none)".into(), ..Default::default() });
        apply_integrations(&mut pc, "default", &clear);
        assert!(pc.integrations.git_host.is_none());
    }

    #[test]
    fn apply_secrets_empty_unchanged_nonempty_overwrites() {
        let mut auth = AuthProfile { git_token: "old".into(), ..Default::default() };
        let mut s = SecretInput::default();
        apply_secrets(&mut auth, Some(&s));
        assert_eq!(auth.git_token, "old");
        s.git_token = "new".into();
        s.jira_email = "a@b.com".into();
        apply_secrets(&mut auth, Some(&s));
        assert_eq!(auth.git_token, "new");
        assert_eq!(auth.jira_email, "a@b.com");
    }

    #[test]
    fn supported_providers_lists_all_capabilities() {
        let p = supported_providers();
        for cap in ["issue_tracker", "wiki", "git_host", "design", "ci", "chat"] {
            assert!(p[cap].is_array(), "missing {cap}");
        }
        assert_eq!(p["git_host"], json!(["github", "gitlab"]));
    }
}
```

- [ ] **Step 2: Register the module**

In `src/main.rs`, add `mod credentials;` (next to `mod config;`).

- [ ] **Step 3: Run to verify (the I/O wrappers in Task 2 are not yet referenced)**

Run: `cargo test credentials:: 2>&1 | tail -10`
Expected: PASS (4 tests). Unused-warning on `config_view`/`apply_*` is acceptable until Task 2 wires them.

- [ ] **Step 4: Commit**

```bash
git add src/credentials.rs src/main.rs
git commit -m "feat(credentials): pure config view + apply transforms (tested)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `credentials.rs` I/O wrappers (read / save / verify)

**Files:**
- Modify: `src/credentials.rs` (add public wrappers above the `#[cfg(test)]` module)

- [ ] **Step 1: Add the wrappers**

```rust
/// Masked view of a registered project's config + bound auth profile.
pub fn project_config_json(global: &GlobalConfig, name: &str) -> Result<Value, String> {
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get(&pc.auth_profile).cloned().unwrap_or_default();
    Ok(config_view(name, &pc, &auth))
}

/// Apply a config payload: integrations + auth-profile name → `config.yaml`;
/// tokens (non-empty only) → `secrets.yaml`.
pub fn save_project_config(global: &GlobalConfig, name: &str, body: &str) -> Result<Value, String> {
    let payload: ConfigPayload = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let repo = entry.repo_path.clone();
    let mut pc = ProjectConfig::load_from(&repo)?;
    let ap = payload
        .auth_profile
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(if pc.auth_profile.is_empty() { "default" } else { pc.auth_profile.as_str() })
        .to_string();
    apply_integrations(&mut pc, &ap, &payload);
    pc.save_to(&repo)?;
    if payload.secrets.is_some() {
        let mut secrets = Secrets::load_or_default()?;
        let auth = secrets.auth_profiles.entry(ap.clone()).or_default();
        apply_secrets(auth, payload.secrets.as_ref());
        secrets.save()?;
    }
    Ok(json!({ "ok": true, "auth_profile": ap }))
}

/// Build the capability's client from saved config + secrets and `verify()` it.
/// Verify failures are data (`ok:false`), not errors; only an unknown capability
/// or unloadable project is an `Err`.
pub fn verify_capability(global: &GlobalConfig, name: &str, cap: &str) -> Result<Value, String> {
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get(&pc.auth_profile).cloned().unwrap_or_default();
    let insecure = false;
    let result = match cap {
        "issue_tracker" => crate::clients::issue_tracker(&pc, &auth, insecure).and_then(|c| c.verify()),
        "wiki" => crate::clients::doc_source(&pc, &auth, insecure).and_then(|c| c.verify()),
        "git_host" => crate::clients::git_host(&pc, &auth, insecure).and_then(|c| c.verify()),
        "design" => crate::clients::design_source(&pc, &auth, insecure).and_then(|c| c.verify()),
        "ci" => crate::clients::ci_provider(&pc, &auth, insecure).and_then(|c| c.verify()),
        "chat" => crate::clients::chat_notify(&pc, &auth, insecure).and_then(|c| c.verify()),
        other => return Err(format!("unknown capability '{other}'")),
    };
    Ok(match result {
        Ok(message) => json!({ "ok": true, "message": message }),
        Err(error) => json!({ "ok": false, "error": error }),
    })
}
```

- [ ] **Step 2: Build + run credentials tests**

Run: `cargo build 2>&1 | tail -3 && cargo test credentials:: 2>&1 | tail -6`
Expected: build OK; 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/credentials.rs
git commit -m "feat(credentials): read/save/verify I/O wrappers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: web routes + handlers

**Files:**
- Modify: `src/web.rs` (Route enum, `route()`, `api()`, route test)

- [ ] **Step 1: Add the failing route test**

In `src/web.rs` `route_parses_paths`, after the skillmap asserts add:

```rust
        assert_eq!(route("GET", "/api/project/app/config"), Route::ProjectConfig("app".into()));
        assert_eq!(route("POST", "/api/project/app/config"), Route::SaveProjectConfig("app".into()));
        assert_eq!(
            route("POST", "/api/project/app/verify/git_host"),
            Route::VerifyCapability("app".into(), "git_host".into())
        );
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test route_parses_paths 2>&1 | tail -8`
Expected: FAIL — `no variant ... ProjectConfig`.

- [ ] **Step 3: Add Route variants**

In `enum Route` (after the cycle-B `SetRecipeBody(String, String),`):

```rust
    ProjectConfig(String),
    SaveProjectConfig(String),
    VerifyCapability(String, String),
```

- [ ] **Step 4: Add route matches**

In `route()`, before the final `_ => Route::NotFound,`:

```rust
        ("GET", ["api", "project", name, "config"]) => Route::ProjectConfig((*name).to_string()),
        ("POST", ["api", "project", name, "config"]) => Route::SaveProjectConfig((*name).to_string()),
        ("POST", ["api", "project", name, "verify", cap]) => {
            Route::VerifyCapability((*name).to_string(), (*cap).to_string())
        }
```

- [ ] **Step 5: Add dispatch handlers**

In `api()`, before the final `_ => (501, ...)`:

```rust
        Route::ProjectConfig(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::project_config_json(&global, &name)
        }),
        Route::SaveProjectConfig(name) => write_op(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::save_project_config(&global, &name, body)
        }),
        Route::VerifyCapability(name, cap) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            crate::credentials::verify_capability(&global, &name, &cap)
        }),
```

- [ ] **Step 6: Build + route test**

Run: `cargo test route_parses_paths 2>&1 | tail -6 && cargo build 2>&1 | tail -3`
Expected: route test PASS; build OK.

- [ ] **Step 7: Commit**

```bash
git add src/web.rs
git commit -m "feat(web): project config GET/POST + verify routes

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: web UI — credentials editor

**Files:**
- Modify: `src/web/app.js` (credentials card + wire into `renderProjectDetail`)
- Modify: `src/web/style.css`

- [ ] **Step 1: Add `credentialsCard` + wire into `renderProjectDetail`**

In `src/web/app.js`, add `credentialsCard` (after `renderProjectDetail`):

```js
function credentialsCard(name, cfg) {
  const card = h(`<div class="card"><h3>Credentials &amp; Integrations</h3>
    <label>auth profile</label><input id="cd-auth" value="${esc(cfg.auth_profile || "default")}">
    <div class="muted">Shared by all projects using this auth-profile name.</div></div>`);
  const CAPS = [
    ["issue_tracker", "Issue tracker", true],
    ["wiki", "Wiki", false],
    ["git_host", "Git host", true],
    ["design", "Design", false],
    ["ci", "CI", true],
    ["chat", "Chat", false],
  ];
  const intWrap = h(`<div></div>`);
  CAPS.forEach(([cap, label, hasRepo]) => {
    const cur = cfg.integrations[cap] || {};
    const opts = ["(none)", ...(cfg.providers[cap] || [])]
      .map(p => `<option${(cur.provider || "(none)") === p ? " selected" : ""}>${esc(p)}</option>`).join("");
    const row = h(`<div class="cd-int" data-cap="${cap}">
      <div class="row"><strong style="min-width:110px">${esc(label)}</strong>
        <select class="cd-prov">${opts}</select>
        <span class="spacer"></span><a class="link cd-verify">Verify</a> <span class="cd-vres"></span></div>
      <input class="cd-base" placeholder="base_url" value="${esc(cur.base_url || "")}">
      ${hasRepo ? `<input class="cd-repo" placeholder="repo (owner/name)" value="${esc(cur.repo || "")}">` : ""}
    </div>`);
    row.querySelector(".cd-verify").onclick = async () => {
      const res = row.querySelector(".cd-vres");
      res.textContent = "…"; res.className = "cd-vres muted";
      try {
        const r = await api(`/api/project/${encodeURIComponent(name)}/verify/${cap}`, "POST", {});
        res.textContent = r.ok ? ("✓ " + (r.message || "ok")) : ("✗ " + (r.error || "failed"));
        res.className = "cd-vres " + (r.ok ? "ok-pill" : "warn-pill");
      } catch (e) { res.textContent = "✗ " + e.message; res.className = "cd-vres warn-pill"; }
    };
    intWrap.appendChild(row);
  });
  card.appendChild(intWrap);

  const sec = cfg.secrets || {};
  const tok = (k, label) => `<label>${label}</label><input class="cd-sec" data-k="${k}" type="password" placeholder="${esc(sec[k] || "(unset)")} — blank = keep">`;
  const txt = (k, label) => `<label>${label}</label><input class="cd-txt" data-k="${k}" value="${esc(sec[k] || "")}">`;
  card.appendChild(h(`<div style="margin-top:10px;border-top:1px solid #2b313c;padding-top:8px"><strong>Tokens</strong>
    ${tok("jira_token", "jira_token")}${txt("jira_email", "jira_email")}
    ${tok("wiki_token", "wiki_token")}${txt("wiki_email", "wiki_email")}
    ${tok("figma_token", "figma_token")}
    ${txt("jenkins_user", "jenkins_user")}${tok("jenkins_token", "jenkins_token")}
    ${tok("git_token", "git_token")}${tok("chat_webhook", "chat_webhook")}
    <div class="muted">Blank token = unchanged. Stored in ~/.palugada/secrets.yaml (0600); never echoed back in full.</div></div>`));

  const save = h(`<div class="row" style="margin-top:10px"><span class="spacer"></span><button id="cd-save">Save credentials</button></div>`);
  card.appendChild(save);
  save.querySelector("#cd-save").onclick = async () => {
    const integrations = {};
    intWrap.querySelectorAll(".cd-int").forEach(row => {
      const repoEl = row.querySelector(".cd-repo");
      integrations[row.dataset.cap] = {
        provider: row.querySelector(".cd-prov").value,
        base_url: row.querySelector(".cd-base").value,
        repo: repoEl ? repoEl.value : "",
      };
    });
    const secrets = {};
    card.querySelectorAll(".cd-sec, .cd-txt").forEach(i => { secrets[i.dataset.k] = i.value; });
    try {
      await api(`/api/project/${encodeURIComponent(name)}/config`, "POST",
        { auth_profile: card.querySelector("#cd-auth").value, integrations, secrets });
      toast("saved credentials");
      renderProjectDetail(name);
    } catch (e) { toast(e.message, true); }
  };
  return card;
}
```

Then, inside `renderProjectDetail`, after the `profile` card is appended and before warnings/skills, insert:

```js
  try {
    const cfg = await api("/api/project/" + encodeURIComponent(name) + "/config");
    view.appendChild(credentialsCard(name, cfg));
  } catch (e) { toast(e.message, true); }
```

- [ ] **Step 2: Add styling**

Append to `src/web/style.css`:

```css
.cd-int { border: 1px dashed #313845; border-radius: 6px; padding: 8px; margin: 6px 0; }
.cd-vres { font-size: 12px; }
```

- [ ] **Step 3: JS syntax check + build**

Run: `node --check src/web/app.js && echo OK && cargo build 2>&1 | tail -2`
Expected: `OK`; build succeeds.

- [ ] **Step 4: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "feat(web): per-project credentials & integrations editor + verify

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Verification

**Files:** none.

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | tail -6`
Expected: all pass (prior + 4 credentials + route asserts).

- [ ] **Step 2: Live HTTP e2e (masked read + save + skill-map reflects)**

```bash
./target/debug/palugada web --port 7799 >/tmp/pw.log 2>&1 &
SRV=$!; sleep 1.5
echo "=== GET config (tokens masked) ==="
curl -s localhost:7799/api/project/status-saver/config | python3 -m json.tool | sed -n '1,40p'
echo "=== save git_host=github + repo, no token ==="
curl -s -X POST localhost:7799/api/project/status-saver/config -H 'Content-Type: application/json' \
  -d '{"integrations":{"git_host":{"provider":"github","base_url":"https://api.github.com","repo":"owner/name"}}}'
echo; echo "=== skillmap now shows palugada-git enabled ==="
curl -s localhost:7799/api/project/status-saver/skillmap | python3 -c 'import json,sys;m=json.load(sys.stdin);print([s for s in m["skills"] if s["name"]=="palugada-git"])'
kill $SRV 2>/dev/null
```
Expected: config JSON shows masked tokens (no plaintext); save returns `{"ok":true,...}`; `palugada-git` now `enabled:true`.

- [ ] **Step 3: Revert the e2e edit (status-saver is a real project)**

Re-issue the save with `git_host: {provider:"(none)"}` (or restore the prior value) so the live e2e doesn't leave `status-saver` reconfigured. Confirm via a fresh `GET .../config`.

```bash
./target/debug/palugada web --port 7799 >/tmp/pw.log 2>&1 &
SRV=$!; sleep 1.5
curl -s -X POST localhost:7799/api/project/status-saver/config -H 'Content-Type: application/json' \
  -d '{"integrations":{"git_host":{"provider":"(none)"}}}' >/dev/null
curl -s localhost:7799/api/project/status-saver/config | python3 -c 'import json,sys;print("git_host:", json.load(sys.stdin)["integrations"]["git_host"])'
kill $SRV 2>/dev/null
```
Expected: `git_host: None`. (If `status-saver` had a real git_host before, restore that instead of clearing — check the GET output from Step 2 first and re-set it.)

- [ ] **Step 4: Manual browser check + dev tree clean**

Open `palugada web` → a project → Credentials card: set a provider, click Verify (expect `✓`/`✗`), Save, confirm the skill map updates. Then:
Run: `git status --porcelain` (expect empty; the dev repo's tracked files unchanged — edits went to `~/.palugada/secrets.yaml` + the project's own `.palugada/config.yaml`, not this repo).

---

## Self-Review

**Spec coverage:**
- `supported_providers` / masked `config_view` / `apply_integrations` / `apply_secrets` → Task 1. ✓
- Read/save/verify I/O wrappers → Task 2. ✓
- 3 routes + handlers → Task 3. ✓
- Credentials editor on the detail page + Verify + Save→re-render → Task 4. ✓
- Masked write-only secrets + 0600 → Task 1 (`config_view` masks, `apply_secrets` non-empty-only) + `Secrets::save`. ✓
- Verify error-boxed (`ok:false`) → Task 2. ✓
- Tests (masked-read, apply set/clear, empty/non-empty token, providers) + route + live e2e → Tasks 1, 3, 5. ✓

**Placeholder scan:** No TBD/TODO; every code step shows complete code.

**Type consistency:** `project_config_json`/`save_project_config`/`verify_capability(global, name[, cap/body])` signatures match the `web.rs` handlers. Route variants `ProjectConfig`/`SaveProjectConfig`/`VerifyCapability` consistent across enum, `route()`, `api()`, tests. JSON keys (`integrations`, `providers`, `secrets`, `auth_profile`, `ok`/`message`/`error`) match between `config_view`/`verify_capability` and the JS (`credentialsCard`). `integration_slot` capability strings match `CAPS` in JS and `supported_providers` keys.
