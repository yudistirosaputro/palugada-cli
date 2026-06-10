//! Jira issue tracker (REST API v2). `base_url` is e.g.
//! `https://your-org.atlassian.net/rest/api/2` or a self-hosted equivalent.

use super::{Issue, IssueTracker};
use crate::http::Http;
use serde::Deserialize;

pub struct Jira {
    base_url: String,
    email: String,
    token: String,
    http: Http,
}

impl Jira {
    pub fn new(base_url: &str, email: &str, token: &str, insecure: bool) -> Self {
        Jira {
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
struct IssueResp {
    key: String,
    fields: Fields,
}

#[derive(Deserialize)]
struct Fields {
    summary: Option<String>,
    status: Option<Named>,
    issuetype: Option<Named>,
    description: Option<String>,
    assignee: Option<Assignee>,
}

#[derive(Deserialize)]
struct Named {
    name: Option<String>,
}

#[derive(Deserialize)]
struct Assignee {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct Myself {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    name: Option<String>,
}

impl IssueTracker for Jira {
    fn get_issue(&self, key: &str) -> Result<Issue, String> {
        if self.base_url.is_empty() {
            return Err("jira base_url is empty in the project config (integrations.issue_tracker.base_url)".into());
        }
        if self.token.is_empty() {
            return Err("jira_token is empty in the auth profile".into());
        }
        let url = format!("{}/issue/{}", self.base_url, crate::http::encode_segment(key));
        let r: IssueResp = self.http.get_json(&url, &self.headers())?;
        Ok(Issue {
            key: r.key,
            summary: r.fields.summary.unwrap_or_default(),
            status: r.fields.status.and_then(|s| s.name).unwrap_or_default(),
            issue_type: r.fields.issuetype.and_then(|s| s.name).unwrap_or_default(),
            assignee: r
                .fields
                .assignee
                .and_then(|a| a.display_name)
                .unwrap_or_else(|| "Unassigned".to_string()),
            description: r.fields.description.unwrap_or_default(),
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.base_url.is_empty() {
            return Err("jira base_url is empty in the project config (integrations.issue_tracker.base_url)".into());
        }
        if self.token.is_empty() {
            return Err("jira_token is empty".into());
        }
        let url = format!("{}/myself", self.base_url);
        let me: Myself = self.http.get_json(&url, &self.headers())?;
        let who = me.display_name.or(me.name).unwrap_or_else(|| "?".to_string());
        Ok(format!("Jira OK — authenticated as {who}"))
    }
}
