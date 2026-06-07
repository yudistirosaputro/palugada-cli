//! Figma design source. Auth via the `X-Figma-Token` header; the API root is
//! `https://api.figma.com` (a per-project `base_url` is optional).

use super::{DesignFile, DesignSource};
use crate::http::Http;
use serde::Deserialize;

pub struct Figma {
    base_url: String,
    token: String,
    http: Http,
}

impl Figma {
    pub fn new(base_url: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.figma.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        Figma {
            base_url: base,
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        if self.token.is_empty() {
            vec![]
        } else {
            vec![("X-Figma-Token", self.token.clone())]
        }
    }
}

#[derive(Deserialize)]
struct FileResp {
    name: Option<String>,
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct MeResp {
    email: Option<String>,
    handle: Option<String>,
}

impl DesignSource for Figma {
    fn get_file(&self, key: &str) -> Result<DesignFile, String> {
        if self.token.is_empty() {
            return Err("figma_token is empty in the auth profile".into());
        }
        let url = format!("{}/v1/files/{}", self.base_url, key);
        let r: FileResp = self.http.get_json(&url, &self.headers())?;
        Ok(DesignFile {
            key: key.to_string(),
            name: r.name.unwrap_or_default(),
            last_modified: r.last_modified.unwrap_or_default(),
            version: r.version.unwrap_or_default(),
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.token.is_empty() {
            return Err("figma_token is empty".into());
        }
        let url = format!("{}/v1/me", self.base_url);
        let me: MeResp = self.http.get_json(&url, &self.headers())?;
        let who = me.email.or(me.handle).unwrap_or_else(|| "?".to_string());
        Ok(format!("Figma OK — authenticated as {who}"))
    }
}
