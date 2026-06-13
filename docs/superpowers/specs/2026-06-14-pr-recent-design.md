# Design — `pr recent <file>` (GitHost extension)

> **Status:** Built · **Date:** 2026-06-14 · Group A sub-project 5 (final). Autonomous.

## Goal

Extend the `GitHost` trait with a read op that lists recent commits touching a
file from the host's API — the reverse-index foundation that complements
`brief bugfix`'s local `git log` with host data — and prove the trait extends
across GitHub **and** GitLab. New `palugada pr recent <file>` command.

## Scope decision: `pr create` deferred

`pr create` (opening a PR/MR) is a **mutating, outward-facing** action of low fit
for a read-oriented knowledge CLI, and was a "→ later" item in the PRD. It is
**deferred**, not built. The `Http::post_json` infrastructure (added with notify)
makes it a small future addition if a concrete need appears. This sub-project
ships only the read op.

## Design

- **Domain type** `CommitRef { sha, title, author, url }` in `clients/mod.rs`.
- **Trait** `GitHost` gains
  `fn recent_commits(&self, path: &str, limit: usize) -> Result<Vec<CommitRef>, String>`.
- **Repo is now needed** by GitHost (commits are per-repo), so
  `GitHub::new` and `GitLab::new` gain a `repo` ("owner/name" / "group/project")
  parameter; the `git_host` factory passes `p.repo`. `whoami`/`verify` ignore it.
- **GitHub** (`github.rs`): `GET {base}/repos/{o}/{r}/commits?path={path}&per_page={limit}`
  → each `{sha, commit.message, commit.author.name, html_url}`; `title` is the
  first line of the commit message (shared `title_line` helper). Repo parsed with
  `github_issues::parse_repo`.
- **GitLab** (`gitlab.rs`): `GET {base}/api/v4/projects/{enc}/repository/commits?path={path}&per_page={limit}`
  → each `{id, title, author_name, web_url}` (GitLab already supplies the title).
- **CLI:** `palugada pr recent <file>` prints `shortsha  title  (author)` + url.
  Empty path or empty repo/token → clear errors.

## Testing

- `title_line("feat: x\n\nbody")` → `"feat: x"`; `title_line("")` → `""`.
- Network calls not unit-tested (consistent with the connector suite).

## Files

`src/clients/mod.rs` (CommitRef + trait method + `title_line` + factory repo
args), `src/clients/github.rs` + `src/clients/gitlab.rs` (repo param +
recent_commits), `src/main.rs` (Pr command + cmd_pr), `README.md`.
