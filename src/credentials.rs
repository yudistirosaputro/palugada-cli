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
#[allow(dead_code)]
pub fn verify_kind(cap: &str, provider: &str) -> &'static str {
    match (cap, provider) {
        ("issue_tracker", "github_issues") => "repo",
        ("ci", "github_actions") | ("ci", "gitlab_ci") => "repo",
        _ => "now",
    }
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
#[allow(dead_code)]
pub fn global_view() -> Result<Value, String> {
    let global = GlobalConfig::load_or_default()?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get("default").cloned().unwrap_or_default();
    Ok(global_view_of(&global.default_integrations, &auth))
}

/// Verify a connector from the GLOBAL page. Repo-bound (cap,provider) pairs return
/// `needs_repo` without a network call; the rest build an ephemeral project config
/// from the global defaults and run the real `verify()`.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
}
