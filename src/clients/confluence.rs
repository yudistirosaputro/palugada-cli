//! Confluence wiki/doc source. `base_url` is e.g.
//! `https://your-org.atlassian.net/wiki/rest/api/content` or self-hosted
//! `https://wiki.example.com/rest/api/content`.

use super::{DocSource, WikiPage};
use crate::http::Http;
use serde::Deserialize;

pub struct Confluence {
    base_url: String,
    email: String,
    token: String,
    http: Http,
}

impl Confluence {
    pub fn new(base_url: &str, email: &str, token: &str, insecure: bool) -> Self {
        Confluence {
            base_url: base_url.trim_end_matches('/').to_string(),
            email: email.to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        if self.token.is_empty() {
            vec![]
        } else {
            vec![("Authorization", super::atlassian_auth(&self.email, &self.token))]
        }
    }
}

#[derive(Deserialize)]
struct PageResp {
    id: String,
    title: Option<String>,
    body: Option<Body>,
}

#[derive(Deserialize)]
struct Body {
    storage: Option<Storage>,
}

#[derive(Deserialize)]
struct Storage {
    value: Option<String>,
}

impl DocSource for Confluence {
    fn get_page(&self, id: &str) -> Result<WikiPage, String> {
        if self.base_url.is_empty() {
            return Err("confluence base_url is empty in the project config (integrations.wiki.base_url)".into());
        }
        if self.token.is_empty() {
            return Err("wiki_token is empty in the auth profile".into());
        }
        let url = format!("{}/{}?expand=body.storage", self.base_url, crate::http::encode_segment(id));
        let r: PageResp = self.http.get_json(&url, &self.headers())?;
        Ok(WikiPage {
            id: r.id,
            title: r.title.unwrap_or_default(),
            body_html: r
                .body
                .and_then(|b| b.storage)
                .and_then(|s| s.value)
                .unwrap_or_default(),
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.base_url.is_empty() {
            return Err("confluence base_url is empty in the project config (integrations.wiki.base_url)".into());
        }
        if self.token.is_empty() {
            return Err("wiki_token is empty".into());
        }
        // A cheap authenticated call: list one page.
        let url = format!("{}?limit=1", self.base_url);
        let _: serde_json::Value = self.http.get_json(&url, &self.headers())?;
        Ok("Confluence OK — token accepted".to_string())
    }
}
