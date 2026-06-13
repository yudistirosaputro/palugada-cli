//! GitHub git host. `base_url` is the API root, e.g. `https://api.github.com`
//! (or `https://github.example.com/api/v3` for GitHub Enterprise). `repo` is
//! "owner/name" — needed by `recent_commits` (not by `whoami`).

use super::{github_headers, github_issues::parse_repo, title_line, CommitRef, GitHost, GitUser};
use crate::http::Http;
use serde::Deserialize;

pub struct GitHub {
    base_url: String,
    repo: String,
    token: String,
    http: Http,
}

impl GitHub {
    pub fn new(base_url: &str, repo: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.github.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitHub {
            base_url: base,
            repo: repo.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }
}

#[derive(Deserialize)]
struct UserResp {
    login: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct CommitResp {
    sha: Option<String>,
    html_url: Option<String>,
    commit: CommitDetail,
}

#[derive(Deserialize)]
struct CommitDetail {
    message: Option<String>,
    author: Option<CommitAuthor>,
}

#[derive(Deserialize)]
struct CommitAuthor {
    name: Option<String>,
}

impl GitHost for GitHub {
    fn whoami(&self) -> Result<GitUser, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let url = format!("{}/user", self.base_url);
        let u: UserResp = self.http.get_json(&url, &github_headers(&self.token))?;
        Ok(GitUser {
            username: u.login.unwrap_or_default(),
            name: u.name.unwrap_or_default(),
            host: self.base_url.clone(),
        })
    }

    fn recent_commits(&self, path: &str, limit: usize) -> Result<Vec<CommitRef>, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let (owner, name) = parse_repo(&self.repo)?;
        let url = format!(
            "{}/repos/{}/{}/commits?path={}&per_page={}",
            self.base_url,
            crate::http::encode_segment(&owner),
            crate::http::encode_segment(&name),
            crate::http::encode_segment(path),
            limit,
        );
        let commits: Vec<CommitResp> = self.http.get_json(&url, &github_headers(&self.token))?;
        Ok(commits
            .into_iter()
            .map(|c| CommitRef {
                sha: c.sha.unwrap_or_default(),
                title: title_line(&c.commit.message.unwrap_or_default()),
                author: c.commit.author.and_then(|a| a.name).unwrap_or_default(),
                url: c.html_url.unwrap_or_default(),
            })
            .collect())
    }

    fn verify(&self) -> Result<String, String> {
        let u = self.whoami()?;
        Ok(format!("GitHub OK — authenticated as {} ({})", u.username, self.base_url))
    }
}
