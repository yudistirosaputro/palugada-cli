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

/// The masked-secrets object shown in every credentials view: tokens are masked
/// (`•••• (N chars)` / `(unset)`), identifiers (emails/users) shown in clear.
fn masked_secrets(auth: &AuthProfile) -> Value {
    json!({
        "jira_token": mask_secret(&auth.jira_token),
        "jira_email": auth.jira_email,
        "wiki_token": mask_secret(&auth.wiki_token),
        "wiki_email": auth.wiki_email,
        "figma_token": mask_secret(&auth.figma_token),
        "jenkins_user": auth.jenkins_user,
        "jenkins_token": mask_secret(&auth.jenkins_token),
        "git_token": mask_secret(&auth.git_token),
        "chat_webhook": mask_secret(&auth.chat_webhook),
    })
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
        "secrets": masked_secrets(auth),
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

/// Read-only sibling of `integration_slot` (for consulting saved wiring).
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

/// The saved `Provider` for `cap`, if any.
fn saved_provider<'a>(i: &'a Integrations, cap: &str) -> Option<&'a Provider> {
    integration_ref(i, cap).and_then(|o| o.as_ref())
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

/// Connectors-page read model, shared by the global-default page and any project
/// target: provider wiring (incl. `repo`) + masked secrets for `auth_profile`.
fn connectors_view_of(integrations: &Integrations, auth: &AuthProfile, auth_profile: &str) -> Value {
    let wire = |o: &Option<Provider>| match o {
        Some(p) => json!({ "provider": p.provider, "base_url": p.base_url, "repo": p.repo }),
        None => json!({ "provider": "", "base_url": "", "repo": "" }),
    };
    json!({
        "auth_profile": auth_profile,
        "providers": supported_providers(),
        "wiring": {
            "issue_tracker": wire(&integrations.issue_tracker),
            "wiki": wire(&integrations.wiki),
            "git_host": wire(&integrations.git_host),
            "design": wire(&integrations.design),
            "ci": wire(&integrations.ci),
            "chat": wire(&integrations.chat),
        },
        "secrets": masked_secrets(auth),
    })
}

/// Masked, browser-safe view of global default wiring + the `default` auth profile.
fn global_view_of(defaults: &Integrations, auth: &AuthProfile) -> Value {
    connectors_view_of(defaults, auth, "default")
}

/// The Connectors page read model (global default wiring + `default` secrets).
pub fn global_view() -> Result<Value, String> {
    let global = GlobalConfig::load_or_default()?;
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get("default").cloned().unwrap_or_default();
    Ok(global_view_of(&global.default_integrations, &auth))
}

/// Verify a connector from the GLOBAL page against the CURRENT form values
/// (`body` = `ConnectorInput`), so a connector can be checked BEFORE it is saved.
/// A blank token falls back to the saved `default`-profile token. Repo-bound
/// providers return `needs_repo` (a global default has no repo).
pub fn global_verify(cap: &str, body: &str) -> Result<Value, String> {
    if !is_known_cap(cap) {
        return Err(format!("unknown capability '{cap}'"));
    }
    let inp: ConnectorInput = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    // Posted form wins; an empty/stale body falls back to the SAVED global default
    // so an already-configured connector still verifies.
    let posted = inp.provider.trim();
    let (provider, base_url) = if !posted.is_empty() && posted != "(none)" {
        (posted.to_string(), inp.base_url.trim().to_string())
    } else {
        let global = GlobalConfig::load_or_default()?;
        match saved_provider(&global.default_integrations, cap) {
            Some(p) => (p.provider.clone(), p.base_url.clone()),
            None => (String::new(), String::new()),
        }
    };
    if provider.is_empty() {
        return Ok(json!({ "ok": false, "error": "no provider configured" }));
    }
    if verify_kind(cap, &provider) == "repo" {
        return Ok(json!({ "ok": false, "needs_repo": true, "message": "verify from a project" }));
    }
    let secrets = Secrets::load_or_default()?;
    let base = secrets.auth_profiles.get("default").cloned().unwrap_or_default();
    match build_verify_config(base, cap, &provider, &base_url, "", &inp.secrets) {
        Some((pc, auth)) => run_verify(&pc, &auth, cap),
        None => Ok(json!({ "ok": false, "error": "no provider configured" })),
    }
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
    /// "owner/name" for repo-bound providers (github_issues / *_ci). Only
    /// meaningful on a project target; the global page leaves it blank.
    #[serde(default)]
    repo: String,
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

/// The six capability slots the console can wire.
fn is_known_cap(cap: &str) -> bool {
    matches!(cap, "issue_tracker" | "wiki" | "git_host" | "design" | "ci" | "chat")
}

/// An otherwise-empty `Integrations` with just `cap` wired — the shape needed to
/// `verify()` a single connector in isolation.
fn one_cap_integrations(cap: &str, provider: &str, base_url: &str, repo: &str) -> Integrations {
    let mut integrations = Integrations::default();
    if let Some(slot) = integration_slot(&mut integrations, cap) {
        *slot = Some(Provider {
            provider: provider.to_string(),
            base_url: base_url.to_string(),
            repo: repo.to_string(),
        });
    }
    integrations
}

/// A posted value wins; a blank one keeps `saved`. Callers pass `saved` only when
/// the provider is unchanged — switching providers (or the global page) passes
/// `None`, so a blank field then means "provider default", never the old value.
fn keep_or_set(posted: &str, saved: Option<&str>) -> String {
    let p = posted.trim();
    if p.is_empty() {
        saved.unwrap_or("").to_string()
    } else {
        p.to_string()
    }
}

/// Build the `(config, auth)` to verify ONE connector from already-resolved form
/// values. `base_auth` is the saved profile, so a blank token field keeps the
/// stored token (verify mirrors save's "blank = keep"). `None` when no provider.
fn build_verify_config(
    base_auth: AuthProfile,
    cap: &str,
    provider: &str,
    base_url: &str,
    repo: &str,
    secrets: &BTreeMap<String, String>,
) -> Option<(ProjectConfig, AuthProfile)> {
    if provider.is_empty() || provider == "(none)" {
        return None;
    }
    let mut auth = base_auth;
    apply_connector_secrets(&mut auth, secrets);
    let integrations = one_cap_integrations(cap, provider, base_url, repo);
    Some((ProjectConfig { integrations, ..Default::default() }, auth))
}

/// Apply ONE posted connector onto a project's config: set the slot (or clear on
/// `(none)`/blank). A blank `base_url`/`repo` KEEPS the stored value when the
/// provider is unchanged (the card renders no field for some of them), but a
/// provider switch drops them so a new provider never inherits a stale URL/repo.
/// Defaults an empty `auth_profile` to `default` so saved tokens are used at fetch.
fn apply_one_connector(pc: &mut ProjectConfig, cap: &str, inp: &ConnectorInput) {
    if let Some(slot) = integration_slot(&mut pc.integrations, cap) {
        let provider = inp.provider.trim();
        if provider.is_empty() || provider == "(none)" {
            *slot = None;
        } else {
            let same = slot.as_ref().is_some_and(|p| p.provider == provider);
            let saved_base = if same { slot.as_ref().map(|p| p.base_url.clone()) } else { None };
            let saved_repo = if same { slot.as_ref().map(|p| p.repo.clone()) } else { None };
            *slot = Some(Provider {
                provider: provider.to_string(),
                base_url: keep_or_set(&inp.base_url, saved_base.as_deref()),
                repo: keep_or_set(&inp.repo, saved_repo.as_deref()),
            });
        }
    }
    if pc.auth_profile.is_empty() {
        pc.auth_profile = "default".to_string();
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

/// Connectors-page read model for a registered PROJECT: same shape as the global
/// page, sourced from the project's own `config.yaml` + its bound auth profile.
pub fn project_connectors_view(global: &GlobalConfig, name: &str) -> Result<Value, String> {
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let secrets = Secrets::load_or_default()?;
    let profile = if pc.auth_profile.is_empty() { "default" } else { pc.auth_profile.as_str() };
    let auth = secrets.auth_profiles.get(profile).cloned().unwrap_or_default();
    Ok(connectors_view_of(&pc.integrations, &auth, profile))
}

/// Save ONE connector to a PROJECT's `config.yaml` (+ tokens → its bound auth
/// profile). Because the per-project wiring wins the merge, this makes what you
/// set in the console exactly what the CLI uses.
pub fn apply_project_connector(
    global: &GlobalConfig,
    name: &str,
    cap: &str,
    body: &str,
) -> Result<Value, String> {
    if !is_known_cap(cap) {
        return Err(format!("unknown capability '{cap}'"));
    }
    let inp: ConnectorInput = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let repo = entry.repo_path.clone();
    let mut pc = ProjectConfig::load_from(&repo)?;
    apply_one_connector(&mut pc, cap, &inp);
    pc.save_to(&repo)?;
    if !inp.secrets.is_empty() {
        let mut secrets = Secrets::load_or_default()?;
        let auth = secrets.auth_profiles.entry(pc.auth_profile.clone()).or_default();
        apply_connector_secrets(auth, &inp.secrets);
        secrets.save()?;
    }
    Ok(json!({ "ok": true, "cap": cap, "project": name }))
}

/// Verify ONE connector for a PROJECT against the CURRENT form values, before
/// save (bug-parity with `global_verify`). Repo-bound providers can be checked
/// here once a `repo` is supplied; a blank token falls back to the saved one.
pub fn project_verify(
    global: &GlobalConfig,
    name: &str,
    cap: &str,
    body: &str,
) -> Result<Value, String> {
    if !is_known_cap(cap) {
        return Err(format!("unknown capability '{cap}'"));
    }
    let inp: ConnectorInput = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let provider = inp.provider.trim();
    if provider.is_empty() || provider == "(none)" {
        return Ok(json!({ "ok": false, "error": "no provider configured" }));
    }
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    // Some providers render no base_url/repo field; for the SAME saved provider,
    // fall back to the stored values so a self-hosted connector isn't checked
    // against the public default (and falsely reported broken).
    let saved = saved_provider(&pc.integrations, cap).filter(|p| p.provider == provider);
    let base_url = keep_or_set(&inp.base_url, saved.map(|p| p.base_url.as_str()));
    let repo = keep_or_set(&inp.repo, saved.map(|p| p.repo.as_str()));
    if verify_kind(cap, provider) == "repo" && repo.is_empty() {
        return Ok(json!({ "ok": false, "needs_repo": true, "message": "set a repo to verify" }));
    }
    let profile = if pc.auth_profile.is_empty() { "default" } else { pc.auth_profile.as_str() };
    let secrets = Secrets::load_or_default()?;
    let base = secrets.auth_profiles.get(profile).cloned().unwrap_or_default();
    match build_verify_config(base, cap, provider, &base_url, &repo, &inp.secrets) {
        Some((pc, auth)) => run_verify(&pc, &auth, cap),
        None => Ok(json!({ "ok": false, "error": "no provider configured" })),
    }
}

/// Masked secrets for a named auth profile (an unknown name → all `(unset)`).
/// Lets the credentials card preview a profile's tokens when you switch the
/// `auth profile` field, instead of leaving stale values from the old profile.
pub fn auth_profile_secrets(name: &str) -> Result<Value, String> {
    let secrets = Secrets::load_or_default()?;
    let auth = secrets.auth_profiles.get(name).cloned().unwrap_or_default();
    Ok(masked_secrets(&auth))
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

    // ── bug 1: verify checks the CURRENT form state, saved or not ──────────

    #[test]
    fn build_verify_uses_given_provider_and_token() {
        // The just-typed provider+token must verify before any Save.
        let secrets = BTreeMap::from([("wiki_token".to_string(), "tok".to_string())]);
        let (pc, auth) = build_verify_config(AuthProfile::default(), "wiki", "notion", "", "", &secrets)
            .expect("non-empty provider → Some");
        assert_eq!(pc.integrations.wiki.as_ref().unwrap().provider, "notion");
        assert_eq!(auth.wiki_token, "tok");
    }

    #[test]
    fn build_verify_blank_token_falls_back_to_saved() {
        // A masked (blank) token field keeps the stored token for the check.
        let base = AuthProfile { wiki_token: "saved".into(), ..Default::default() };
        let secrets = BTreeMap::from([("wiki_token".to_string(), String::new())]);
        let (_pc, auth) = build_verify_config(base, "wiki", "notion", "", "", &secrets).unwrap();
        assert_eq!(auth.wiki_token, "saved");
    }

    #[test]
    fn build_verify_none_when_provider_blank() {
        assert!(build_verify_config(AuthProfile::default(), "wiki", "(none)", "", "", &BTreeMap::new()).is_none());
    }

    #[test]
    fn keep_or_set_prefers_posted_else_saved() {
        assert_eq!(keep_or_set("new", Some("old")), "new");
        assert_eq!(keep_or_set("", Some("old")), "old"); // blank keeps saved
        assert_eq!(keep_or_set("   ", Some("old")), "old"); // whitespace = blank
        assert_eq!(keep_or_set("", None), ""); // no saved (provider switch / global)
    }

    // ── bug 2 & 3: editing a connector rewrites the PROJECT config.yaml ─────

    #[test]
    fn apply_one_connector_keeps_repo_and_base_on_same_provider_resave() {
        // Re-saving the SAME provider with blank base_url/repo (fields the card
        // doesn't render) must NOT wipe the stored values — no silent data loss.
        let mut pc = ProjectConfig::default();
        pc.integrations.git_host = Some(Provider {
            provider: "github".into(),
            base_url: "https://gh.corp".into(),
            repo: "team/app".into(),
        });
        apply_one_connector(&mut pc, "git_host", &ConnectorInput { provider: "github".into(), ..Default::default() });
        let g = pc.integrations.git_host.as_ref().unwrap();
        assert_eq!(g.repo, "team/app");
        assert_eq!(g.base_url, "https://gh.corp");
    }

    #[test]
    fn apply_one_connector_switch_provider_drops_old_base_url() {
        // Switching providers must NOT inherit the old provider's base_url/repo.
        let mut pc = ProjectConfig::default();
        pc.integrations.wiki = Some(Provider {
            provider: "confluence".into(),
            base_url: "https://x.atlassian.net/wiki".into(),
            repo: String::new(),
        });
        apply_one_connector(&mut pc, "wiki", &ConnectorInput { provider: "notion".into(), ..Default::default() });
        let w = pc.integrations.wiki.as_ref().unwrap();
        assert_eq!(w.provider, "notion");
        assert_eq!(w.base_url, ""); // not the stale confluence URL
    }

    #[test]
    fn apply_one_connector_switches_project_provider() {
        // confluence → notion on the project itself, so the project (which wins
        // the merge) actually drives `palugada wiki page`.
        let mut pc = ProjectConfig::default();
        pc.integrations.wiki =
            Some(Provider { provider: "confluence".into(), ..Default::default() });
        apply_one_connector(&mut pc, "wiki", &ConnectorInput { provider: "notion".into(), ..Default::default() });
        assert_eq!(pc.integrations.wiki.as_ref().unwrap().provider, "notion");
        // empty auth_profile defaulted so saved tokens are actually used at fetch
        assert_eq!(pc.auth_profile, "default");
    }

    #[test]
    fn apply_one_connector_clears_on_none() {
        let mut pc = ProjectConfig::default();
        pc.integrations.wiki = Some(Provider { provider: "confluence".into(), ..Default::default() });
        apply_one_connector(&mut pc, "wiki", &ConnectorInput { provider: "(none)".into(), ..Default::default() });
        assert!(pc.integrations.wiki.is_none());
    }

    #[test]
    fn project_connector_save_roundtrips_to_config_yaml() {
        // The switch must persist to <repo>/.palugada/config.yaml.
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().to_str().unwrap();
        let mut pc = ProjectConfig::default();
        pc.integrations.wiki = Some(Provider { provider: "confluence".into(), ..Default::default() });
        pc.save_to(repo).unwrap();

        let mut loaded = ProjectConfig::load_from(repo).unwrap();
        apply_one_connector(&mut loaded, "wiki", &ConnectorInput { provider: "notion".into(), ..Default::default() });
        loaded.save_to(repo).unwrap();

        let reloaded = ProjectConfig::load_from(repo).unwrap();
        assert_eq!(reloaded.integrations.wiki.unwrap().provider, "notion");
    }

    #[test]
    fn masked_secrets_masks_tokens_keeps_identifiers() {
        let auth = AuthProfile {
            wiki_token: "supersecret".into(),
            wiki_email: "me@x.com".into(),
            ..Default::default()
        };
        let v = masked_secrets(&auth);
        assert!(!v.to_string().contains("supersecret"), "plaintext token leaked");
        assert_ne!(v["wiki_token"], "supersecret");
        assert_eq!(v["wiki_email"], "me@x.com");
        assert_eq!(v["git_token"], "(unset)");
    }

    #[test]
    fn connectors_view_of_includes_repo_and_profile() {
        let integ = Integrations {
            issue_tracker: Some(Provider {
                provider: "github_issues".into(),
                base_url: "https://api.github.com".into(),
                repo: "o/n".into(),
            }),
            ..Default::default()
        };
        let v = connectors_view_of(&integ, &AuthProfile::default(), "myprof");
        assert_eq!(v["auth_profile"], "myprof");
        assert_eq!(v["wiring"]["issue_tracker"]["provider"], "github_issues");
        assert_eq!(v["wiring"]["issue_tracker"]["repo"], "o/n");
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
