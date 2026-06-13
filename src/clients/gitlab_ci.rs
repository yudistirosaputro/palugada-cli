//! GitLab CI provider. `base_url` is the instance root (default
//! `https://gitlab.com`); `repo` is the project path "group/project"; auth
//! reuses the GitLab PAT (`git_token`) via the PRIVATE-TOKEN header. `job` is an
//! optional git ref/branch — empty means the project's newest pipeline.

use super::{CiBuild, CiProvider};
use crate::http::Http;
use serde::Deserialize;

pub struct GitLabCi {
    base_url: String,
    project: String,
    token: String,
    http: Http,
}

impl GitLabCi {
    pub fn new(base_url: &str, project: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://gitlab.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitLabCi {
            base_url: base,
            project: project.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }

    fn headers(&self) -> Vec<(&str, String)> {
        vec![("PRIVATE-TOKEN", self.token.clone())]
    }
}

/// A GitLab pipeline is "building" while it is queued or running.
fn pipeline_building(status: &str) -> bool {
    matches!(
        status,
        "running" | "pending" | "created" | "preparing" | "waiting_for_resource" | "scheduled"
    )
}

#[derive(Deserialize)]
struct Pipeline {
    id: Option<u64>,
    status: Option<String>,
}

#[derive(Deserialize)]
struct ProjectResp {
    path_with_namespace: Option<String>,
}

impl CiProvider for GitLabCi {
    fn job_status(&self, job: &str) -> Result<CiBuild, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        if self.project.is_empty() {
            return Err("gitlab_ci: set integrations.ci.repo (group/project)".into());
        }
        let mut url = format!(
            "{}/api/v4/projects/{}/pipelines?per_page=1",
            self.base_url,
            crate::http::encode_segment(&self.project),
        );
        if !job.is_empty() {
            url.push_str(&format!("&ref={}", crate::http::encode_segment(job)));
        }
        let pipelines: Vec<Pipeline> = self.http.get_json(&url, &self.headers())?;
        let p = pipelines.into_iter().next().ok_or_else(|| {
            let scope = if job.is_empty() { self.project.clone() } else { format!("{} @ {job}", self.project) };
            format!("no pipelines found for {scope}")
        })?;
        let status = p.status.unwrap_or_default();
        Ok(CiBuild {
            job: if job.is_empty() { self.project.clone() } else { job.to_string() },
            number: p.id.unwrap_or(0),
            building: pipeline_building(&status),
            result: status,
        })
    }

    fn verify(&self) -> Result<String, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        if self.project.is_empty() {
            return Err("gitlab_ci: set integrations.ci.repo (group/project)".into());
        }
        let url = format!(
            "{}/api/v4/projects/{}",
            self.base_url,
            crate::http::encode_segment(&self.project),
        );
        let r: ProjectResp = self.http.get_json(&url, &self.headers())?;
        Ok(format!(
            "GitLab CI OK — {} reachable",
            r.path_with_namespace.unwrap_or_else(|| self.project.clone())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::pipeline_building;

    #[test]
    fn pipeline_building_tracks_active_statuses() {
        assert!(pipeline_building("running"));
        assert!(pipeline_building("pending"));
        assert!(!pipeline_building("success"));
        assert!(!pipeline_building("failed"));
        assert!(!pipeline_building("canceled"));
    }
}
