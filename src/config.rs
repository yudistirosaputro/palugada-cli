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
    pub fn save_to_path(&self, p: &Path) -> Result<(), String> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(p)
            .map_err(|e| format!("open {}: {e}", p.display()))?;
        f.write_all(data.as_bytes())
            .map_err(|e| format!("write {}: {e}", p.display()))?;
        fs::set_permissions(p, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod {}: {e}", p.display()))?;
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

    #[test]
    fn mask_secret_hides_everything_and_is_utf8_safe() {
        assert_eq!(mask_secret(""), "(unset)");
        let m = mask_secret("abcd1234");
        assert!(!m.contains("ab") && !m.contains("34"), "no leading/trailing chars: {m}");
        // multi-byte secret must not panic (old code sliced bytes)
        let m = mask_secret("ключключключ");
        assert!(m.starts_with("****"), "{m}");
    }

    #[test]
    fn expand_home_handles_bare_tilde() {
        assert_eq!(expand_home("~"), home_dir());
        assert_eq!(expand_home("~/x"), home_dir().join("x"));
        assert_eq!(expand_home("/abs/path"), Path::new("/abs/path").to_path_buf());
    }

    #[test]
    fn secrets_save_is_0600_and_round_trips() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
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
