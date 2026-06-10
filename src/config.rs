//! Configuration model for palugada.
//!
//! Three files (see PRD §5):
//!   * `~/.palugada.yaml`         — global: defaults + project registry (no secrets)
//!   * `~/.palugada/secrets.yaml` — auth-profiles (tokens), chmod 0600
//!   * `<repo>/.palugada/config.yaml` — per-project: profile + provider wiring
//!
//! Tokens are referenced by auth-profile *name* from the per-project config,
//! so the project config can be committed without leaking credentials.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────
// ~/.palugada.yaml — global config
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default = "default_schema")]
    pub schema_version: String,
    #[serde(default)]
    pub engine: EngineSection,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub projects: Projects,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        GlobalConfig {
            schema_version: default_schema(),
            engine: EngineSection::default(),
            defaults: Defaults::default(),
            projects: Projects::default(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct EngineSection {
    /// Path to the `knowledge/` directory (the one containing `profiles/`).
    /// Auto-detected by `palugada config init` when run from the repo.
    #[serde(default)]
    pub knowledge_path: String,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Defaults {
    #[serde(default)]
    pub profile: String,
    #[serde(default = "default_stale")]
    pub stale_warning_days: u32,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Projects {
    /// Implicit target when `--project` is omitted.
    #[serde(default)]
    pub active: String,
    #[serde(default)]
    pub registered: BTreeMap<String, ProjectEntry>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ProjectEntry {
    /// Path to the project's code repo.
    #[serde(default)]
    pub repo_path: String,
    /// Path to the project's palugada workspace (default: <repo>/.palugada).
    #[serde(default)]
    pub workspace: String,
}

fn default_schema() -> String {
    "1.0".to_string()
}
fn default_stale() -> u32 {
    7
}

impl GlobalConfig {
    pub fn default_path() -> PathBuf {
        home_dir().join(".palugada.yaml")
    }

    /// Load `~/.palugada.yaml`, or return a default if it does not exist yet.
    pub fn load_or_default() -> Result<GlobalConfig, String> {
        let p = Self::default_path();
        if !p.exists() {
            return Ok(GlobalConfig::default());
        }
        let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        serde_yaml::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::default_path();
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        fs::write(&p, data).map_err(|e| format!("write {}: {e}", p.display()))?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────
// ~/.palugada/secrets.yaml — auth profiles (chmod 0600)
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Secrets {
    #[serde(default)]
    pub auth_profiles: BTreeMap<String, AuthProfile>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct AuthProfile {
    #[serde(default)]
    pub jira_token: String,
    #[serde(default)]
    pub wiki_token: String,
    #[serde(default)]
    pub figma_token: String,
    #[serde(default)]
    pub jenkins_user: String,
    #[serde(default)]
    pub jenkins_token: String,
    /// GitLab/GitHub personal access token for PR/MR + user APIs.
    #[serde(default)]
    pub git_token: String,
}

impl Secrets {
    pub fn dir() -> PathBuf {
        home_dir().join(".palugada")
    }
    pub fn default_path() -> PathBuf {
        Self::dir().join("secrets.yaml")
    }

    pub fn load_from_path(p: &Path) -> Result<Secrets, String> {
        if !p.exists() {
            return Ok(Secrets::default());
        }
        let data = fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))?;
        serde_yaml::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
    }

    pub fn load_or_default() -> Result<Secrets, String> {
        Self::load_from_path(&Self::default_path())
    }

    /// Write the secrets file, created 0600 so tokens are never world-readable.
    /// Uses an atomic write (write to sibling .tmp, then rename) to eliminate
    /// the brief world-readable window on re-save.
    pub fn save_to_path(&self, p: &Path) -> Result<(), String> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        let tmp = p.with_extension("tmp");
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|e| format!("open {}: {e}", tmp.display()))?;
        f.write_all(data.as_bytes())
            .map_err(|e| format!("write {}: {e}", tmp.display()))?;
        drop(f);
        fs::rename(&tmp, p).map_err(|e| format!("rename {}: {e}", tmp.display()))?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to_path(&Self::default_path())
    }
}

// ─────────────────────────────────────────────────────────────────────────
// <repo>/.palugada/config.yaml — per-project config
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub profile: String,
    /// Name of the auth-profile in ~/.palugada/secrets.yaml.
    #[serde(default)]
    pub auth_profile: String,
    #[serde(default)]
    pub integrations: Integrations,
}

#[derive(Debug, Serialize, Deserialize, Default)]
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
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Provider {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub base_url: String,
}

impl ProjectConfig {
    /// `<repo>/.palugada/config.yaml`
    pub fn config_path(repo_path: &str) -> PathBuf {
        expand_home(repo_path).join(".palugada").join("config.yaml")
    }

    pub fn load_from(repo_path: &str) -> Result<ProjectConfig, String> {
        let p = Self::config_path(repo_path);
        let data = fs::read_to_string(&p)
            .map_err(|e| format!("read {}: {e}\nRun `palugada init` in the project first.", p.display()))?;
        serde_yaml::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
    }

    pub fn save_to(&self, repo_path: &str) -> Result<(), String> {
        let p = Self::config_path(repo_path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        fs::write(&p, data).map_err(|e| format!("write {}: {e}", p.display()))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Resolution helpers
// ─────────────────────────────────────────────────────────────────────────

/// Resolve the target project name (PRD §5.4): explicit `--project` (must be
/// registered) → the registered project whose repo contains `cwd` → the
/// registry's `active`.
pub fn resolve_project_name(
    global: &GlobalConfig,
    project_override: Option<&str>,
    cwd: &Path,
) -> Result<String, String> {
    if let Some(name) = project_override.filter(|s| !s.is_empty()) {
        if !global.projects.registered.contains_key(name) {
            let known: Vec<&str> = global.projects.registered.keys().map(String::as_str).collect();
            return Err(format!(
                "project '{name}' is not registered — known projects: {}",
                if known.is_empty() { "(none)".to_string() } else { known.join(", ") }
            ));
        }
        return Ok(name.to_string());
    }
    let cwd_canon = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let best = global
        .projects
        .registered
        .iter()
        .filter(|(_, e)| !e.repo_path.is_empty())
        .filter_map(|(name, e)| {
            let p = std::fs::canonicalize(expand_home(&e.repo_path))
                .unwrap_or_else(|_| expand_home(&e.repo_path));
            cwd_canon.starts_with(&p).then(|| (name, p.components().count()))
        })
        .max_by_key(|(_, depth)| *depth);
    if let Some((name, _)) = best {
        return Ok(name.clone());
    }
    if !global.projects.active.is_empty() {
        let name = &global.projects.active;
        if global.projects.registered.contains_key(name) {
            return Ok(name.clone());
        }
        return Err(format!(
            "project '{name}' (active) is not in the registry — run `palugada project use <name>` to pick another or `palugada project add` to re-register"
        ));
    }
    Err("no active project — run `palugada project use <name>`, pass --project, or cd into a registered repo".into())
}

/// Resolve the target project's config plus the referenced auth-profile.
pub fn resolve_project(
    global: &GlobalConfig,
    secrets: &Secrets,
    project_override: Option<&str>,
) -> Result<(String, ProjectConfig, AuthProfile), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let name = resolve_project_name(global, project_override, &cwd)?;
    let entry = global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered — run `palugada project add {name} <repo_path>`"))?;

    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let auth = if pc.auth_profile.is_empty() {
        AuthProfile::default()
    } else {
        secrets.auth_profiles.get(&pc.auth_profile).cloned().ok_or_else(|| {
            let known: Vec<&str> = secrets.auth_profiles.keys().map(String::as_str).collect();
            format!(
                "auth profile '{}' (referenced by project '{name}') not found in {} — known profiles: {}",
                pc.auth_profile,
                Secrets::default_path().display(),
                if known.is_empty() { "(none)".to_string() } else { known.join(", ") }
            )
        })?
    };
    Ok((name, pc, auth))
}

/// Resolve the repo to operate on: explicit `--repo` → explicit `--project`
/// (must be registered) → cwd inside a registered repo → active project → cwd.
pub fn resolve_repo(
    global: &GlobalConfig,
    project_override: Option<&str>,
    repo_flag: Option<String>,
    cwd: &Path,
) -> Result<PathBuf, String> {
    if let Some(r) = repo_flag {
        if !r.is_empty() {
            return Ok(expand_home(&r));
        }
    }
    match resolve_project_name(global, project_override, cwd) {
        Ok(name) => {
            if let Some(e) = global.projects.registered.get(&name) {
                if !e.repo_path.is_empty() {
                    return Ok(expand_home(&e.repo_path));
                }
            }
            Ok(cwd.to_path_buf())
        }
        // explicit --project typo must surface; "no active project" falls back to cwd
        Err(e) if project_override.filter(|s| !s.is_empty()).is_some() => Err(e),
        Err(_) => Ok(cwd.to_path_buf()),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Small utilities
// ─────────────────────────────────────────────────────────────────────────

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

pub fn expand_home(s: &str) -> PathBuf {
    if s == "~" {
        home_dir()
    } else if let Some(stripped) = s.strip_prefix("~/") {
        home_dir().join(stripped)
    } else {
        Path::new(s).to_path_buf()
    }
}

/// Mask a secret for display. Reveals nothing but presence and length.
pub fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "(unset)".to_string()
    } else {
        format!("**** ({} chars)", s.chars().count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn global_with(name: &str, repo: &Path) -> GlobalConfig {
        let mut g = GlobalConfig::default();
        g.projects.registered.insert(
            name.to_string(),
            ProjectEntry { repo_path: repo.to_string_lossy().to_string(), workspace: String::new() },
        );
        g
    }

    #[test]
    fn resolve_project_name_prefers_cwd_over_active() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut g = global_with("aaa", a.path());
        g.projects.registered.insert(
            "bbb".into(),
            ProjectEntry { repo_path: b.path().to_string_lossy().to_string(), workspace: String::new() },
        );
        g.projects.active = "aaa".to_string();
        // cwd inside repo B → project bbb wins over active aaa
        let sub = b.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(resolve_project_name(&g, None, &sub).unwrap(), "bbb");
        // explicit --project always wins
        assert_eq!(resolve_project_name(&g, Some("aaa"), &sub).unwrap(), "aaa");
        // typo'd --project is a hard error naming known projects
        let err = resolve_project_name(&g, Some("nope"), &sub).unwrap_err();
        assert!(err.contains("nope") && err.contains("aaa"), "{err}");
        assert!(err.contains("bbb"), "{err}");
        // no cwd match → active
        let other = tempfile::tempdir().unwrap();
        assert_eq!(resolve_project_name(&g, None, other.path()).unwrap(), "aaa");
    }

    #[test]
    fn nested_repo_resolves_to_most_specific() {
        // "aaa-outer" < "zzz-inner" alphabetically; alphabetical (BTreeMap) order
        // would visit "aaa-outer" first and return it for a deep cwd — this test
        // proves the longest-match logic overrides key order.
        let outer = tempfile::tempdir().unwrap();
        let inner_path = outer.path().join("sub").join("inner");
        std::fs::create_dir_all(&inner_path).unwrap();

        let outer_canon = std::fs::canonicalize(outer.path()).unwrap();
        let inner_canon = std::fs::canonicalize(&inner_path).unwrap();

        let mut g = GlobalConfig::default();
        g.projects.registered.insert(
            "aaa-outer".into(),
            ProjectEntry { repo_path: outer_canon.to_string_lossy().to_string(), workspace: String::new() },
        );
        g.projects.registered.insert(
            "zzz-inner".into(),
            ProjectEntry { repo_path: inner_canon.to_string_lossy().to_string(), workspace: String::new() },
        );

        // cwd deep inside inner: must resolve to "zzz-inner", not "aaa-outer"
        let deep = inner_canon.join("src").join("module");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(resolve_project_name(&g, None, &deep).unwrap(), "zzz-inner");

        // cwd directly inside outer (but not inner): must resolve to "aaa-outer"
        let outer_sub = outer_canon.join("other");
        std::fs::create_dir_all(&outer_sub).unwrap();
        assert_eq!(resolve_project_name(&g, None, &outer_sub).unwrap(), "aaa-outer");
    }

    #[test]
    fn resolve_repo_returns_registered_path_when_cwd_inside() {
        let a = tempfile::tempdir().unwrap();
        let a_canon = std::fs::canonicalize(a.path()).unwrap();
        let g = global_with("myproj", &a_canon);
        let sub = a_canon.join("deep").join("dir");
        std::fs::create_dir_all(&sub).unwrap();
        let result = resolve_repo(&g, None, None, &sub).unwrap();
        assert_eq!(result, a_canon, "resolve_repo should return the registered canonicalized path");
    }

    #[test]
    fn resolve_repo_expands_tilde_and_errors_on_unknown_project() {
        let a = tempfile::tempdir().unwrap();
        let g = global_with("aaa", a.path());
        let cwd = tempfile::tempdir().unwrap();
        // unknown --project: hard error (old code silently fell back to cwd)
        assert!(resolve_repo(&g, Some("nope"), None, cwd.path()).is_err());
        // --repo flag wins and expands ~
        let r = resolve_repo(&g, None, Some("~/somewhere".into()), cwd.path()).unwrap();
        assert_eq!(r, home_dir().join("somewhere"));
        // fallback: plain cwd
        let r = resolve_repo(&g, None, None, cwd.path()).unwrap();
        assert_eq!(r, cwd.path().to_path_buf());
    }

    #[test]
    fn resolve_project_errors_on_unknown_auth_profile() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".palugada")).unwrap();
        std::fs::write(
            repo.path().join(".palugada").join("config.yaml"),
            "project: aaa\nprofile: generic\nauth_profile: typo\n",
        )
        .unwrap();
        let mut g = global_with("aaa", repo.path());
        g.projects.active = "aaa".to_string();
        let mut secrets = Secrets::default();
        secrets.auth_profiles.insert("default".into(), AuthProfile::default());
        let err = resolve_project(&g, &secrets, Some("aaa")).unwrap_err();
        assert!(err.contains("typo") && err.contains("default"), "{err}");
    }

    #[test]
    fn mask_secret_hides_everything_and_is_utf8_safe() {
        assert_eq!(mask_secret(""), "(unset)");
        let m = mask_secret("abcd1234");
        assert!(!m.contains("ab") && !m.contains("34"), "no leading/trailing chars: {m}");
        assert!(m.starts_with("****"), "should start with ****: {m}");
        assert!(m.contains("8 chars"), "should contain char count: {m}");
        // multi-byte secret must not panic (old code sliced bytes)
        let m = mask_secret("ключключключ");
        assert!(m.starts_with("****"), "{m}");
    }

    #[test]
    fn expand_home_handles_bare_tilde() {
        assert_eq!(expand_home("~"), home_dir());
        assert_eq!(expand_home("~/x"), home_dir().join("x"));
        assert_eq!(expand_home("/abs/path"), Path::new("/abs/path").to_path_buf());
        assert_eq!(expand_home("~foo"), Path::new("~foo").to_path_buf());
    }

    #[test]
    fn secrets_save_is_0600_and_round_trips() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().expect("tempdir creation failed");
        let p = dir.path().join("nested").join("secrets.yaml");
        let mut s = Secrets::default();
        s.auth_profiles.insert("default".into(), AuthProfile { jira_token: "t".into(), ..Default::default() });
        s.save_to_path(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode was {mode:o}");
        let loaded = Secrets::load_from_path(&p).unwrap();
        assert_eq!(loaded.auth_profiles["default"].jira_token, "t");
        // re-save over an existing file keeps 0600
        s.save_to_path(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
