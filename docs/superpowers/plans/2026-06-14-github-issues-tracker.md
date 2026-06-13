# GitHub Issues IssueTracker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `github_issues` provider behind the existing `IssueTracker` trait so `palugada issue view <N>` works on a GitHub repo.

**Architecture:** New `clients/github_issues.rs` (`GitHubIssues` + `parse_repo` + `IssueTracker` impl), a `github_issues` arm in the `issue_tracker()` factory, a `repo` field on `Provider`, and a shared `github_headers` helper in `mod.rs` reused by `github.rs`. Generic call sites (`cmd_issue`, `config verify`, `doctor`) need no change.

**Tech Stack:** Rust, `ureq` via `crate::http::Http`, `serde`/`serde_json`, no new dependencies (reuses `git_token`).

**Reference spec:** `docs/superpowers/specs/2026-06-14-github-issues-tracker-design.md`

**Patterns to mirror:** `src/clients/jira.rs` (IssueTracker impl), `src/clients/github.rs` (GitHub auth/base_url), factory in `src/clients/mod.rs`. `Http::get_json(url, &[(&str,String)])` and `crate::http::encode_segment` are the HTTP primitives.

**Test command:** `cargo test` · **Build:** `cargo build`

---

## Task 1: Shared `github_headers` helper

**Files:**
- Modify: `src/clients/mod.rs` (add helper + test)
- Modify: `src/clients/github.rs` (use it)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/clients/mod.rs` (which already tests `atlassian_auth`):

```rust
    #[test]
    fn github_headers_include_ua_version_and_optional_bearer() {
        let h = github_headers("tok123");
        assert!(h.iter().any(|(k, v)| *k == "Authorization" && v == "Bearer tok123"));
        assert!(h.iter().any(|(k, _)| *k == "User-Agent"));
        assert!(h.iter().any(|(k, _)| *k == "X-GitHub-Api-Version"));
        // no token → no Authorization header
        assert!(github_headers("").iter().all(|(k, _)| *k != "Authorization"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test github_headers_include`
Expected: FAIL — `github_headers` not defined.

- [ ] **Step 3: Add the helper**

In `src/clients/mod.rs`, add next to `atlassian_auth`:

```rust
/// Standard GitHub API request headers: the required User-Agent + API version,
/// plus a Bearer PAT when one is present. Shared by the GitHub GitHost and
/// IssueTracker providers.
pub fn github_headers(token: &str) -> Vec<(&str, String)> {
    let mut h = vec![
        ("User-Agent", "palugada".to_string()),
        ("X-GitHub-Api-Version", "2022-11-28".to_string()),
    ];
    if !token.is_empty() {
        h.push(("Authorization", format!("Bearer {token}")));
    }
    h
}
```

- [ ] **Step 4: Use it from `github.rs`**

In `src/clients/github.rs`, delete the inline `fn headers(&self)` method and replace its single call site. The `whoami` method calls `&self.headers()`; change that to `&super::github_headers(&self.token)`. Concretely:

- Remove:
```rust
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
```
- In `whoami`, change `let u: UserResp = self.http.get_json(&url, &self.headers())?;` to:
```rust
        let u: UserResp = self.http.get_json(&url, &super::github_headers(&self.token))?;
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: all pass — new `github_headers` test plus every existing test (github.rs still compiles and behaves identically).

- [ ] **Step 6: Commit**

```bash
git add src/clients/mod.rs src/clients/github.rs
git commit -m "refactor(clients): extract shared github_headers helper

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `repo` field on `Provider`

**Files:**
- Modify: `src/config.rs` (add field + test)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/config.rs`:

```rust
    #[test]
    fn provider_parses_repo_field() {
        let p: Provider = serde_yaml::from_str(
            "provider: github_issues\nbase_url: https://api.github.com\nrepo: octocat/hello\n",
        ).unwrap();
        assert_eq!(p.provider, "github_issues");
        assert_eq!(p.repo, "octocat/hello");
        // repo is optional — absent → empty
        let q: Provider = serde_yaml::from_str("provider: jira\n").unwrap();
        assert_eq!(q.repo, "");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test provider_parses_repo_field`
Expected: FAIL — `Provider` has no field `repo`.

- [ ] **Step 3: Add the field**

In `src/config.rs`, the `Provider` struct becomes:

```rust
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Provider {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub base_url: String,
    /// "owner/name" — used by github_issues (and future github_actions).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub repo: String,
}
```

- [ ] **Step 4: Run test**

Run: `cargo test provider_parses_repo_field`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add optional repo field to Provider

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `github_issues.rs` provider + factory wiring

**Files:**
- Create: `src/clients/github_issues.rs`
- Modify: `src/clients/mod.rs` (`mod github_issues;` + factory arm)

- [ ] **Step 1: Write the module (with its `parse_repo` test)**

Create `src/clients/github_issues.rs`:

```rust
//! GitHub Issues issue tracker. `base_url` is the API root (default
//! `https://api.github.com`); `repo` is "owner/name". Auth reuses the GitHub PAT
//! (`git_token`). The `/issues/{n}` endpoint also returns pull requests (shared
//! number space); the `pull_request` field disambiguates the type.

use super::{github_headers, Issue, IssueTracker};
use crate::http::Http;
use serde::Deserialize;

pub struct GitHubIssues {
    base_url: String,
    repo: String,
    token: String,
    http: Http,
}

impl GitHubIssues {
    pub fn new(base_url: &str, repo: &str, token: &str, insecure: bool) -> Self {
        let base = if base_url.is_empty() {
            "https://api.github.com".to_string()
        } else {
            base_url.trim_end_matches('/').to_string()
        };
        GitHubIssues {
            base_url: base,
            repo: repo.trim().to_string(),
            token: token.to_string(),
            http: Http::new(insecure),
        }
    }
}

/// Split "owner/name" into its two non-empty parts.
pub fn parse_repo(repo: &str) -> Result<(String, String), String> {
    if repo.is_empty() {
        return Err("github_issues: set integrations.issue_tracker.repo (owner/name)".into());
    }
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(format!("github_issues: repo must be 'owner/name', got '{repo}'"));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

#[derive(Deserialize)]
struct IssueResp {
    number: u64,
    title: Option<String>,
    state: Option<String>,
    body: Option<String>,
    assignee: Option<User>,
    pull_request: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct User {
    login: Option<String>,
}

#[derive(Deserialize)]
struct RepoResp {
    full_name: Option<String>,
}

impl IssueTracker for GitHubIssues {
    fn get_issue(&self, key: &str) -> Result<Issue, String> {
        if self.token.is_empty() {
            return Err("git_token is empty in the auth profile".into());
        }
        let (owner, name) = parse_repo(&self.repo)?;
        let url = format!(
            "{}/repos/{}/{}/issues/{}",
            self.base_url,
            crate::http::encode_segment(&owner),
            crate::http::encode_segment(&name),
            crate::http::encode_segment(key),
        );
        let r: IssueResp = self.http.get_json(&url, &github_headers(&self.token))?;
        Ok(Issue {
            key: format!("{}#{}", self.repo, r.number),
            summary: r.title.unwrap_or_default(),
            status: r.state.unwrap_or_default(),
            issue_type: if r.pull_request.is_some() {
                "Pull Request".to_string()
            } else {
                "Issue".to_string()
            },
            assignee: r.assignee.and_then(|a| a.login).unwrap_or_else(|| "Unassigned".to_string()),
            description: r.body.unwrap_or_default(),
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
            "GitHub Issues OK — {} reachable",
            r.full_name.unwrap_or_else(|| self.repo.clone())
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_repo;

    #[test]
    fn parse_repo_splits_owner_and_name() {
        assert_eq!(
            parse_repo("octocat/hello").unwrap(),
            ("octocat".to_string(), "hello".to_string())
        );
    }

    #[test]
    fn parse_repo_rejects_malformed() {
        assert!(parse_repo("bad").is_err());
        assert!(parse_repo("a/b/c").is_err());
        assert!(parse_repo("/x").is_err());
        assert!(parse_repo("x/").is_err());
        assert!(parse_repo("").is_err());
    }
}
```

- [ ] **Step 2: Register the module + factory arm in `mod.rs`**

In `src/clients/mod.rs`, add the module declaration alongside the others (`pub mod github;` etc.):

```rust
pub mod github_issues;
```

Then extend the `issue_tracker` factory. Replace:

```rust
    match p.provider.as_str() {
        "jira" => Ok(Box::new(jira::Jira::new(&p.base_url, &auth.jira_email, &auth.jira_token, insecure))),
        other => Err(format!("unsupported issue_tracker provider: '{other}' (supported: jira)")),
    }
```

with:

```rust
    match p.provider.as_str() {
        "jira" => Ok(Box::new(jira::Jira::new(&p.base_url, &auth.jira_email, &auth.jira_token, insecure))),
        "github_issues" => Ok(Box::new(github_issues::GitHubIssues::new(
            &p.base_url,
            &p.repo,
            &auth.git_token,
            insecure,
        ))),
        other => Err(format!(
            "unsupported issue_tracker provider: '{other}' (supported: jira, github_issues)"
        )),
    }
```

- [ ] **Step 3: Run tests + build**

Run: `cargo test && cargo build`
Expected: `parse_repo` tests pass; the full suite passes; build succeeds (factory arm wires `GitHubIssues`).

- [ ] **Step 4: Commit**

```bash
git add src/clients/github_issues.rs src/clients/mod.rs
git commit -m "feat(clients): GitHub Issues IssueTracker provider

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Docs + final verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Mark github_issues implemented in README**

In `README.md`, the line in the project-config section reads: "Each integration names a provider. Implemented today: issue tracker `jira`; …  Other providers (GitHub Issues, Notion, …) are roadmap only — selecting one is a hard error." Update it so issue tracker lists both providers and GitHub Issues is no longer in the roadmap-only list:

- Change `issue tracker \`jira\`` to `issue tracker \`jira\` or \`github_issues\``.
- Remove "GitHub Issues" from the parenthetical roadmap list, leaving e.g. "Other providers (Notion, …) are roadmap only".

- [ ] **Step 2: Final verification**

Run: `cargo test && cargo build --release`
Expected: all tests pass; release build succeeds.

Run: `./target/release/palugada issue --help`
Expected: prints help (no panic) — confirms the binary still builds and dispatches.

Run: `git grep -n "supported: jira, github_issues" src/clients/mod.rs`
Expected: one match — the factory now advertises both providers.

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: mark github_issues implemented in README

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-review notes

- **Spec coverage:** §4 arch (module + factory + shared headers) → Tasks 1 & 3; §5 config `repo` → Task 2; §6 endpoint/mapping → Task 3 `get_issue`; §7 verify → Task 3 `verify`; §8 errors → `parse_repo` + empty-token guards in Task 3; §9 tests → Task 1 (`github_headers`) + Task 2 (`provider_parses_repo_field`) + Task 3 (`parse_repo`); §10 README → Task 4.
- **Type consistency:** `github_headers(&str) -> Vec<(&str, String)>` matches `Http::get_json(headers: &[(&str, String)])`. `GitHubIssues::new(base_url, repo, token, insecure)` is called identically in the factory (Task 3 Step 2). `parse_repo` signature is identical in definition, callers, and tests.
- **No new deps:** reuses `git_token`, `serde_json` (already present) for the `pull_request` value.
- **Out of scope (spec §3):** list/create issues, other providers, per-key repo override.
