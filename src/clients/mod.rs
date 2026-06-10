//! Provider-agnostic connector layer.
//!
//! Each integration is a capability **trait**; concrete providers implement it.
//! A per-project config (`integrations:` block) selects the provider + base
//! URL, and the matching auth-profile supplies the token. This is what lets the
//! same `palugada issue view X` work whether the project is on Jira or GitHub.

pub mod confluence;
pub mod figma;
pub mod github;
pub mod gitlab;
pub mod jenkins;
pub mod jira;

use crate::config::{AuthProfile, ProjectConfig};

// ── Shared domain types ─────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Issue {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub assignee: String,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct WikiPage {
    pub id: String,
    pub title: String,
    pub body_html: String,
}

#[derive(Debug, Default)]
pub struct GitUser {
    pub username: String,
    pub name: String,
    pub host: String,
}

#[derive(Debug, Default)]
pub struct DesignFile {
    pub key: String,
    pub name: String,
    pub last_modified: String,
    pub version: String,
}

#[derive(Debug, Default)]
pub struct CiBuild {
    pub job: String,
    pub number: u64,
    pub result: String,
    pub building: bool,
}

// ── Capability traits ───────────────────────────────────────────────────

pub trait IssueTracker {
    fn get_issue(&self, key: &str) -> Result<Issue, String>;
    /// Lightweight connectivity + auth check. Returns a human-readable status.
    fn verify(&self) -> Result<String, String>;
}

pub trait DocSource {
    fn get_page(&self, id: &str) -> Result<WikiPage, String>;
    fn verify(&self) -> Result<String, String>;
}

pub trait GitHost {
    fn whoami(&self) -> Result<GitUser, String>;
    fn verify(&self) -> Result<String, String>;
}

pub trait DesignSource {
    fn get_file(&self, key: &str) -> Result<DesignFile, String>;
    fn verify(&self) -> Result<String, String>;
}

pub trait CiProvider {
    fn job_status(&self, job: &str) -> Result<CiBuild, String>;
    fn verify(&self) -> Result<String, String>;
}

// ── Factories: build a connector from project config + auth profile ───────

/// Authorization header value for Atlassian-style APIs: `email` present →
/// Basic base64(email:token) (Atlassian Cloud API tokens), else Bearer
/// (server / Data-Center PATs).
pub fn atlassian_auth(email: &str, token: &str) -> String {
    use base64::Engine as _;
    if email.is_empty() {
        format!("Bearer {token}")
    } else {
        let creds =
            base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
        format!("Basic {creds}")
    }
}

pub fn issue_tracker(
    pc: &ProjectConfig,
    auth: &AuthProfile,
    insecure: bool,
) -> Result<Box<dyn IssueTracker>, String> {
    let p = pc
        .integrations
        .issue_tracker
        .as_ref()
        .ok_or("no issue_tracker configured for this project")?;
    match p.provider.as_str() {
        "jira" => Ok(Box::new(jira::Jira::new(&p.base_url, &auth.jira_email, &auth.jira_token, insecure))),
        other => Err(format!("unsupported issue_tracker provider: '{other}' (supported: jira)")),
    }
}

pub fn doc_source(
    pc: &ProjectConfig,
    auth: &AuthProfile,
    insecure: bool,
) -> Result<Box<dyn DocSource>, String> {
    let p = pc
        .integrations
        .wiki
        .as_ref()
        .ok_or("no wiki configured for this project")?;
    match p.provider.as_str() {
        "confluence" => Ok(Box::new(confluence::Confluence::new(
            &p.base_url,
            &auth.wiki_email,
            &auth.wiki_token,
            insecure,
        ))),
        other => Err(format!("unsupported wiki provider: '{other}' (supported: confluence)")),
    }
}

pub fn git_host(
    pc: &ProjectConfig,
    auth: &AuthProfile,
    insecure: bool,
) -> Result<Box<dyn GitHost>, String> {
    let p = pc
        .integrations
        .git_host
        .as_ref()
        .ok_or("no git_host configured for this project")?;
    match p.provider.as_str() {
        "gitlab" => Ok(Box::new(gitlab::GitLab::new(&p.base_url, &auth.git_token, insecure))),
        "github" => Ok(Box::new(github::GitHub::new(&p.base_url, &auth.git_token, insecure))),
        other => Err(format!("unsupported git_host provider: '{other}' (supported: gitlab, github)")),
    }
}

pub fn design_source(
    pc: &ProjectConfig,
    auth: &AuthProfile,
    insecure: bool,
) -> Result<Box<dyn DesignSource>, String> {
    let p = pc
        .integrations
        .design
        .as_ref()
        .ok_or("no design configured for this project")?;
    match p.provider.as_str() {
        "figma" => Ok(Box::new(figma::Figma::new(&p.base_url, &auth.figma_token, insecure))),
        other => Err(format!("unsupported design provider: '{other}' (supported: figma)")),
    }
}

pub fn ci_provider(
    pc: &ProjectConfig,
    auth: &AuthProfile,
    insecure: bool,
) -> Result<Box<dyn CiProvider>, String> {
    let p = pc
        .integrations
        .ci
        .as_ref()
        .ok_or("no ci configured for this project")?;
    match p.provider.as_str() {
        "jenkins" => Ok(Box::new(jenkins::Jenkins::new(
            &p.base_url,
            &auth.jenkins_user,
            &auth.jenkins_token,
            insecure,
        ))),
        other => Err(format!("unsupported ci provider: '{other}' (supported: jenkins)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlassian_auth_picks_basic_when_email_present() {
        assert_eq!(atlassian_auth("", "tok123"), "Bearer tok123");
        // base64("me@x.co:tok123") = bWVAeC5jbzp0b2sxMjM=
        assert_eq!(atlassian_auth("me@x.co", "tok123"), "Basic bWVAeC5jbzp0b2sxMjM=");
    }
}
