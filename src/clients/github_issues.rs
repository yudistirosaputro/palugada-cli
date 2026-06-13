//! GitHub Issues issue tracker. `base_url` is the API root (default
//! `https://api.github.com`); `repo` is "owner/name". Auth reuses the GitHub PAT
//! (`git_token`). The `/issues/{n}` endpoint also returns pull requests (shared
//! number space); the `pull_request` field disambiguates the type.

use super::{github_headers, Issue, IssueTracker};
use crate::http::Http;
use serde::Deserialize;

pub struct GitHubIssues {
    base_url: String,
    repo: String,
    token: String,
    http: Http,
}

impl GitHubIssues {
    pub fn new(base_url: &str, repo: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.github.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitHubIssues {
            base_url: base,
            repo: repo.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }
}

/// Split "owner/name" into its two non-empty parts.
pub fn parse_repo(repo: &str) -> Result<(String, String), String> {
    if repo.is_empty() {
        return Err("github_issues: set integrations.issue_tracker.repo (owner/name)".into());
    }
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("github_issues: repo must be 'owner/name', got '{repo}'"));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[derive(Deserialize)]
struct IssueResp {
    number: u64,
    title: Option<String>,
    state: Option<String>,
    body: Option<String>,
    assignee: Option<User>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct User {
    login: Option<String>,
}

#[derive(Deserialize)]
struct RepoResp {
    full_name: Option<String>,
}

impl IssueTracker for GitHubIssues {
    fn get_issue(&self, key: &str) -> Result<Issue, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let (owner, name) = parse_repo(&self.repo)?;
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.base_url,
            crate::http::encode_segment(&owner),
            crate::http::encode_segment(&name),
            crate::http::encode_segment(key),
        );
        let r: IssueResp = self.http.get_json(&url, &github_headers(&self.token))?;
        Ok(Issue {
            key: format!("{}#{}", self.repo, r.number),
            summary: r.title.unwrap_or_default(),
            status: r.state.unwrap_or_default(),
            issue_type: if r.pull_request.is_some() {
                "Pull Request".to_string()
            } else {
                "Issue".to_string()
            },
            assignee: r.assignee.and_then(|a| a.login).unwrap_or_else(|| "Unassigned".to_string()),
            description: r.body.unwrap_or_default(),
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let (owner, name) = parse_repo(&self.repo)?;
        let url = format!(
            "{}/repos/{}/{}",
            self.base_url,
            crate::http::encode_segment(&owner),
            crate::http::encode_segment(&name),
        );
        let r: RepoResp = self.http.get_json(&url, &github_headers(&self.token))?;
        Ok(format!(
            "GitHub Issues OK — {} reachable",
            r.full_name.unwrap_or_else(|| self.repo.clone())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_repo;

    #[test]
    fn parse_repo_splits_owner_and_name() {
        assert_eq!(
            parse_repo("octocat/hello").unwrap(),
            ("octocat".to_string(), "hello".to_string())
        );
    }

    #[test]
    fn parse_repo_rejects_malformed() {
        assert!(parse_repo("bad").is_err());
        assert!(parse_repo("a/b/c").is_err());
        assert!(parse_repo("/x").is_err());
        assert!(parse_repo("x/").is_err());
        assert!(parse_repo("").is_err());
    }
}
