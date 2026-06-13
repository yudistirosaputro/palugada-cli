//! GitHub Actions CI provider. `base_url` is the API root (default
//! `https://api.github.com`); `repo` is "owner/name"; auth reuses the GitHub PAT
//! (`git_token`). `job` is a workflow file name (`ci.yml`) or numeric id.

use super::{github_headers, github_issues::parse_repo, CiBuild, CiProvider};
use crate::http::Http;
use serde::Deserialize;

pub struct GitHubActions {
    base_url: String,
    repo: String,
    token: String,
    http: Http,
}

impl GitHubActions {
    pub fn new(base_url: &str, repo: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.github.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitHubActions {
            base_url: base,
            repo: repo.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }
}

/// Map a workflow run's (status, conclusion) to (result, building). A run is
/// "building" until its status is `completed`; once completed, the conclusion
/// (success/failure/cancelled/…) is the result.
fn run_state(status: &str, conclusion: Option<&str>) -> (String, bool) {
    if status == "completed" {
        (conclusion.unwrap_or("completed").to_string(), false)
    } else {
        (status.to_string(), true)
    }
}

#[derive(Deserialize)]
struct RunsResp {
    workflow_runs: Vec<Run>,
}

#[derive(Deserialize)]
struct Run {
    run_number: Option<u64>,
    status: Option<String>,
    conclusion: Option<String>,
}

#[derive(Deserialize)]
struct RepoResp {
    full_name: Option<String>,
}

impl CiProvider for GitHubActions {
    fn job_status(&self, job: &str) -> Result<CiBuild, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let (owner, name) = parse_repo(&self.repo)?;
        let url = format!(
            "{}/repos/{}/{}/actions/workflows/{}/runs?per_page=1",
            self.base_url,
            crate::http::encode_segment(&owner),
            crate::http::encode_segment(&name),
            crate::http::encode_segment(job),
        );
        let r: RunsResp = self.http.get_json(&url, &github_headers(&self.token))?;
        let run = r
            .workflow_runs
            .into_iter()
            .next()
            .ok_or_else(|| format!("no runs found for workflow '{job}' in {}", self.repo))?;
        let (result, building) =
            run_state(run.status.as_deref().unwrap_or(""), run.conclusion.as_deref());
        Ok(CiBuild {
            job: job.to_string(),
            number: run.run_number.unwrap_or(0),
            result,
            building,
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
            "GitHub Actions OK — {} reachable",
            r.full_name.unwrap_or_else(|| self.repo.clone())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::run_state;

    #[test]
    fn run_state_maps_completed_and_in_progress() {
        assert_eq!(run_state("completed", Some("success")), ("success".to_string(), false));
        assert_eq!(run_state("completed", Some("failure")), ("failure".to_string(), false));
        assert_eq!(run_state("in_progress", None), ("in_progress".to_string(), true));
        assert_eq!(run_state("queued", None), ("queued".to_string(), true));
    }
}
