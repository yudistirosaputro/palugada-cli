//! Jenkins CI provider. Basic auth (`username:api_token`). `base_url` is the
//! Jenkins root, e.g. `https://jenkins.example.com`.

use super::{CiBuild, CiProvider};
use crate::http::Http;
use base64::Engine as _;
use serde::Deserialize;

pub struct Jenkins {
    base_url: String,
    user: String,
    token: String,
    http: Http,
}

impl Jenkins {
    pub fn new(base_url: &str, user: &str, token: &str, insecure: bool) -> Self {
        Jenkins {
            base_url: base_url.trim_end_matches('/').to_string(),
            user: user.to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        if self.user.is_empty() && self.token.is_empty() {
            vec![]
        } else {
            let creds = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", self.user, self.token));
            vec![("Authorization", format!("Basic {creds}"))]
        }
    }
}

#[derive(Deserialize)]
struct BuildResp {
    number: Option<u64>,
    result: Option<String>,
    building: Option<bool>,
}

#[derive(Deserialize)]
struct MeResp {
    #[serde(rename = "fullName")]
    full_name: Option<String>,
}

/// Jenkins path for a possibly-foldered job: "a/b" → "a/job/b" (each segment
/// percent-encoded). The caller wraps it as /job/<this>/...
fn job_path(job: &str) -> String {
    job.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| crate::http::encode_segment(s))
        .collect::<Vec<_>>()
        .join("/job/")
}

impl CiProvider for Jenkins {
    fn job_status(&self, job: &str) -> Result<CiBuild, String> {
        if self.base_url.is_empty() {
            return Err("jenkins base_url is empty in the project config".into());
        }
        let url = format!("{}/job/{}/lastBuild/api/json", self.base_url, job_path(job));
        let r: BuildResp = self.http.get_json(&url, &self.headers())?;
        let building = r.building.unwrap_or(false);
        Ok(CiBuild {
            job: job.to_string(),
            number: r.number.unwrap_or(0),
            result: r
                .result
                .unwrap_or_else(|| if building { "BUILDING".to_string() } else { "UNKNOWN".to_string() }),
            building,
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.base_url.is_empty() {
            return Err("jenkins base_url is empty".into());
        }
        let url = format!("{}/me/api/json", self.base_url);
        let me: MeResp = self.http.get_json(&url, &self.headers())?;
        Ok(format!(
            "Jenkins OK — authenticated as {}",
            me.full_name.unwrap_or_else(|| "?".to_string())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_path_handles_folders_and_encoding() {
        assert_eq!(job_path("app"), "app");
        assert_eq!(job_path("folder/app"), "folder/job/app");
        assert_eq!(job_path("team a/app"), "team%20a/job/app");
    }
}
