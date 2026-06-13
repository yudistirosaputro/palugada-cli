//! GitLab git host. `base_url` is the instance root, e.g.
//! `https://gitlab.example.com` (the `/api/v4` suffix is added here).

use super::{CommitRef, GitHost, GitUser};
use crate::http::Http;
use serde::Deserialize;

pub struct GitLab {
    base_url: String,
    project: String,
    token: String,
    http: Http,
}

impl GitLab {
    pub fn new(base_url: &str, repo: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://gitlab.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitLab {
            base_url: base,
            project: repo.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        if self.token.is_empty() {
            vec![]
        } else {
            // GitLab personal access tokens use the PRIVATE-TOKEN header.
            vec![("PRIVATE-TOKEN", self.token.clone())]
        }
    }
}

#[derive(Deserialize)]
struct UserResp {
    username: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct CommitResp {
    id: Option<String>,
    title: Option<String>,
    author_name: Option<String>,
    web_url: Option<String>,
}

impl GitHost for GitLab {
    fn whoami(&self) -> Result<GitUser, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let url = format!("{}/api/v4/user", self.base_url);
        let u: UserResp = self.http.get_json(&url, &self.headers())?;
        Ok(GitUser {
            username: u.username.unwrap_or_default(),
            name: u.name.unwrap_or_default(),
            host: self.base_url.clone(),
        })
    }

    fn recent_commits(&self, path: &str, limit: usize) -> Result<Vec<CommitRef>, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        if self.project.is_empty() {
            return Err("gitlab: set integrations.git_host.repo (group/project)".into());
        }
        let url = format!(
            "{}/api/v4/projects/{}/repository/commits?path={}&per_page={}",
            self.base_url,
            crate::http::encode_segment(&self.project),
            crate::http::encode_segment(path),
            limit,
        );
        let commits: Vec<CommitResp> = self.http.get_json(&url, &self.headers())?;
        Ok(commits
            .into_iter()
            .map(|c| CommitRef {
                sha: c.id.unwrap_or_default(),
                title: c.title.unwrap_or_default(),
                author: c.author_name.unwrap_or_default(),
                url: c.web_url.unwrap_or_default(),
            })
            .collect())
    }

    fn verify(&self) -> Result<String, String> {
        let u = self.whoami()?;
        Ok(format!("GitLab OK — authenticated as {} ({})", u.username, self.base_url))
    }
}
