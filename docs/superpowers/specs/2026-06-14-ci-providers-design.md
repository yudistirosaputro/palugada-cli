# Design — GitHub Actions + GitLab CI (CiProvider)

> **Status:** Built · **Date:** 2026-06-14 · Group A sub-project 3. Autonomous.

## Goal

Add `github_actions` and `gitlab_ci` providers behind the existing `CiProvider`
trait so `palugada ci status <job>` works beyond Jenkins. Both reuse `git_token`
and the `repo` field — no new secret.

## Design

`CiProvider::job_status(job) -> CiBuild { job, number, result, building }` and
`verify()` are unchanged. Two new modules + two factory arms in `ci_provider()`.

### github_actions (`clients/github_actions.rs`)
- Config: `ci: { provider: github_actions, repo: owner/name }` (`base_url`
  defaults to `https://api.github.com`; Enterprise via base_url). Auth: `git_token`
  via the shared `github_headers`. Repo parsed with `github_issues::parse_repo`.
- `job` = a workflow file name (`ci.yml`) or numeric id.
  `GET {base}/repos/{o}/{r}/actions/workflows/{job}/runs?per_page=1` → newest run.
- Mapping via a pure `run_state(status, conclusion)`:
  `building = status != "completed"`; `result =` conclusion when completed, else
  the status (e.g. `in_progress`). `number = run_number`.
- `verify`: `GET {base}/repos/{o}/{r}` → `"GitHub Actions OK — {full_name} reachable"`.

### gitlab_ci (`clients/gitlab_ci.rs`)
- Config: `ci: { provider: gitlab_ci, base_url: https://gitlab.com, repo: group/project }`.
  Auth: `git_token` via `PRIVATE-TOKEN` (matches `gitlab.rs`). The project path is
  URL-encoded whole (`encode_segment("group/project")` → `group%2Fproject`).
- `job` = a git ref/branch (optional). `GET {base}/api/v4/projects/{enc}/pipelines?per_page=1`
  (plus `&ref={job}` when non-empty) → newest pipeline.
- Mapping via a pure `pipeline_building(status)`: true for
  `running|pending|created|preparing|waiting_for_resource|scheduled`. `result =
  status`; `number = id`.
- `verify`: `GET {base}/api/v4/projects/{enc}` → `"GitLab CI OK — {path_with_namespace} reachable"`.

## Non-goals

- Triggering builds, listing jobs within a pipeline, logs — only latest status.

## Testing

- `github_actions::run_state("completed", Some("success"))` → `("success", false)`;
  `("in_progress", None)` → `("in_progress", true)`.
- `gitlab_ci::pipeline_building("running")` true; `("success")` false.
- Empty `repo`/`token` → error before network (tested via the parse/guards).

## Files

`src/clients/github_actions.rs`, `src/clients/gitlab_ci.rs` (new);
`src/clients/mod.rs` (two factory arms + `mod` decls); `README.md`.
