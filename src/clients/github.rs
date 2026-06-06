//! GitHub git host. `base_url` is the API root, e.g. `https://api.github.com`
//! (or `https://github.example.com/api/v3` for GitHub Enterprise).

use super::{GitHost, GitUser};
use crate::http::Http;
use serde::Deserialize;

pub struct GitHub {
    base_url: String,
    token: String,
    http: Http,
}

impl GitHub {
    pub fn new(base_url: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.github.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitHub {
            base_url: base,
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        // GitHub requires a User-Agent; auth is a Bearer PAT.
        let mut h = vec![
            ("User-Agent", "palugada".to_string()),
            ("X-GitHub-Api-Version", "2022-11-28".to_string()),
        ];
        if !self.token.is_empty() {
            h.push(("Authorization", format!("Bearer {}", self.token)));
        }
        h
    }
}

#[derive(Deserialize)]
struct UserResp {
    login: Option<String>,
    name: Option<String>,
}

impl GitHost for GitHub {
    fn whoami(&self) -> Result<GitUser, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let url = format!("{}/user", self.base_url);
        let u: UserResp = self.http.get_json(&url, &self.headers())?;
        Ok(GitUser {
            username: u.login.unwrap_or_default(),
            name: u.name.unwrap_or_default(),
            host: self.base_url.clone(),
        })
    }

    fn verify(&self) -> Result<String, String> {
        let u = self.whoami()?;
        Ok(format!("GitHub OK — authenticated as {} ({})", u.username, self.base_url))
    }
}
