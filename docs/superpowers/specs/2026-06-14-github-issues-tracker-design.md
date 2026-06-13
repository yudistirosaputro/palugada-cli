# Design — GitHub Issues as a second IssueTracker

> **Status:** Approved for planning · **Date:** 2026-06-14
> **Scope:** Add a `github_issues` provider behind the existing `IssueTracker`
> trait so `palugada issue view <N>` works on a GitHub repo — the first proof of
> the provider-agnostic claim (PRD §4.4 / §7.4). Sub-project 1 of "Group A".

## 1. Problem

The connector layer is built on capability traits, but every trait has exactly
one implementation, so "provider-agnostic" is unproven. `IssueTracker` only has
Jira; the factory hard-errors on any other provider, and the README says
selecting `github_issues` is "a hard error". A team whose issues live on GitHub
can't use `palugada issue view`.

## 2. Goals

- A `GitHubIssues` provider implementing `IssueTracker`, selected by
  `provider: github_issues` in a project's `integrations.issue_tracker`.
- `palugada issue view <number>` fetches a GitHub issue; `config verify` /
  `doctor` validate the connection — both work through the existing generic
  call sites with no special-casing.
- Reuse the existing GitHub auth (`git_token`); no new secret.
- A small DRY improvement: share the GitHub HTTP headers between the existing
  `github.rs` (GitHost) and the new module.

## 3. Non-goals

- Other providers (Linear, Notion, GitHub Actions, Slack, PR ops) — separate
  sub-projects.
- Listing/searching/creating issues — only `get_issue` (view), matching the
  current `IssueCmd::View` surface.
- Per-key repo override (`owner/repo#42`) — the repo comes from config; revisit
  if needed.

## 4. Architecture

New module `src/clients/github_issues.rs` with a `GitHubIssues` struct
(`base_url`, `repo`, `token`, `http`) implementing `IssueTracker` — same shape as
`jira.rs` and `github.rs`. The factory `clients::issue_tracker()` gains a
`"github_issues"` arm building it from the provider's `base_url` + `repo` and the
auth profile's `git_token`. Because `cmd_issue`, `config verify`, and `doctor`
already call `issue_tracker(...)` / `.verify()` generically, no other code
changes.

**Shared headers (DRY):** extract the GitHub header set —
`User-Agent: palugada`, `X-GitHub-Api-Version: 2022-11-28`, and
`Authorization: Bearer <token>` when a token is present — into a
`github_headers(token: &str) -> Vec<(&'static str, String)>` helper in
`clients/mod.rs` (beside `atlassian_auth`), and use it from both `github.rs` and
`github_issues.rs`. `github.rs`'s inline `headers()` is replaced by a call to it.

## 5. Config

Add one optional field to `Provider` (`src/config.rs`):

```rust
#[serde(default)]
pub repo: String,   // "owner/name" — used by github_issues (and future github_actions)
```

Example project config:

```yaml
integrations:
  issue_tracker:
    provider: github_issues
    base_url: "https://api.github.com"   # or https://github.example.com/api/v3
    repo: "octocat/hello-world"
```

`base_url` defaults to `https://api.github.com` when empty (same as `github.rs`).

## 6. Endpoint + field mapping

`GET {base}/repos/{owner}/{repo}/issues/{number}` with the GitHub headers.
Map the response to the shared `Issue` struct:

| `Issue` field | GitHub source |
|---|---|
| `key` | `"{repo}#{number}"` (e.g. `octocat/hello-world#42`) |
| `summary` | `title` |
| `status` | `state` (`open` / `closed`) |
| `issue_type` | `pull_request` field present → `"Pull Request"`, else `"Issue"` |
| `assignee` | `assignee.login`, else `"Unassigned"` |
| `description` | `body` (may be null → empty) |

The issue number is taken as the command arg and URL-encoded via
`crate::http::encode_segment` (matching `jira.rs`).

## 7. verify()

`GET {base}/repos/{owner}/{repo}` → confirms the token authenticates **and** the
repo is reachable in one call. The response's `full_name` drives the success
message: `"GitHub Issues OK — {full_name} reachable"`. Empty `repo`/`token` →
specific error before any network call.

## 8. Error handling

- `repo` empty → `"github_issues: set integrations.issue_tracker.repo (owner/name)"`.
- `repo` without a single `/` → `"github_issues: repo must be 'owner/name', got '<x>'"`.
- `git_token` empty → `"git_token is empty in the auth profile"` (matches `github.rs`).
- HTTP 404/401/etc. surface through `Http::get_json` as today.

## 9. Testing

Pure unit tests in `clients/mod.rs` (the file already tests `atlassian_auth`;
network paths are not unit-tested here, consistent with `jira`/`github`):

- `parse_repo("owner/name")` returns `("owner", "name")`; `parse_repo("bad")` and
  `parse_repo("a/b/c")` return `Err`.
- `github_headers("tok")` contains `Authorization: Bearer tok`, a `User-Agent`,
  and `X-GitHub-Api-Version`; `github_headers("")` omits `Authorization`.

`parse_repo` lives in `github_issues.rs` and is tested in that file's `tests`
module; `github_headers` lives in `mod.rs` and is tested there. `cargo build`
confirms the factory arm wires.

## 10. Affected files

| File | Change |
|---|---|
| `src/clients/github_issues.rs` | new — `GitHubIssues` + `parse_repo` + `IssueTracker` impl |
| `src/clients/mod.rs` | `github_headers` helper; `github_issues` factory arm; `mod github_issues;` |
| `src/clients/github.rs` | use `github_headers` instead of inline `headers()` |
| `src/config.rs` | add `repo` to `Provider` |
| `README.md` | mark `github_issues` implemented; drop it from the "hard error" list |

## 11. Risks

- **GitHub issue vs PR:** the `/issues/{n}` endpoint also returns PRs (they share
  the number space). The `pull_request` field disambiguates `issue_type`; this is
  intended, not a bug.
- **Enterprise base URLs:** handled by `base_url` (trim trailing slash, default to
  public API) exactly as `github.rs` already does.
