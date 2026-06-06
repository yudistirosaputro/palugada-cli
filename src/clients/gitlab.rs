//! GitLab git host. `base_url` is the instance root, e.g.
//! `https://gitlab.example.com` (the `/api/v4` suffix is added here).

use super::{GitHost, GitUser};
use crate::http::Http;
use serde::Deserialize;

pub struct GitLab {
    base_url: String,
    token: String,
    http: Http,
}

impl GitLab {
    pub fn new(base_url: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://gitlab.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitLab {
            base_url: base,
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

    fn verify(&self) -> Result<String, String> {
        let u = self.whoami()?;
        Ok(format!("GitLab OK — authenticated as {} ({})", u.username, self.base_url))
    }
}
