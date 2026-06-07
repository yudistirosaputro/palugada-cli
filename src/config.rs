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
use std::os::unix::fs::PermissionsExt;
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
    /// Chat webhook (DingTalk/Slack/Teams).
    #[serde(default)]
    pub chat_webhook: String,
}

impl Secrets {
    pub fn dir() -> PathBuf {
        home_dir().join(".palugada")
    }
    pub fn default_path() -> PathBuf {
        Self::dir().join("secrets.yaml")
    }

    pub fn load_or_default() -> Result<Secrets, String> {
        let p = Self::default_path();
        if !p.exists() {
            return Ok(Secrets::default());
        }
        let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
        serde_yaml::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
    }

    /// Write the secrets file with 0600 permissions (creates ~/.palugada/).
    pub fn save(&self) -> Result<(), String> {
        let dir = Self::dir();
        fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        let p = Self::default_path();
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        fs::write(&p, data).map_err(|e| format!("write {}: {e}", p.display()))?;
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod {}: {e}", p.display()))?;
        Ok(())
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
    #[serde(default)]
    pub chat: Option<Provider>,
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

/// Resolve the target project: explicit `--project`, else the registry's
/// `active`. Returns its per-project config plus the referenced auth-profile.
pub fn resolve_project(
    global: &GlobalConfig,
    secrets: &Secrets,
    project_override: Option<&str>,
) -> Result<(String, ProjectConfig, AuthProfile), String> {
    let name = project_override
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| global.projects.active.clone());

    if name.is_empty() {
        return Err("no active project — run `palugada project use <name>` or pass --project".into());
    }
    let entry = global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered — run `palugada project add {name} <repo_path>`"))?;

    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let auth = secrets
        .auth_profiles
        .get(&pc.auth_profile)
        .cloned()
        .unwrap_or_default();
    Ok((name, pc, auth))
}

// ─────────────────────────────────────────────────────────────────────────
// Small utilities
// ─────────────────────────────────────────────────────────────────────────

pub fn home_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
}

pub fn expand_home(s: &str) -> PathBuf {
    if let Some(stripped) = s.strip_prefix("~/") {
        home_dir().join(stripped)
    } else {
        Path::new(s).to_path_buf()
    }
}

/// Mask a secret for display, keeping the first/last 2 chars.
pub fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "(unset)".to_string()
    } else if s.len() <= 4 {
        "*".repeat(s.len())
    } else {
        format!("{}{}{}", &s[..2], "*".repeat(s.len() - 4), &s[s.len() - 2..])
    }
}
