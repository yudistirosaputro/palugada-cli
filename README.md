# palugada

Project-agnostic developer knowledge & connector CLI. This is the **base
slice**: configuration + a project registry + provider-agnostic connectors
(Jira, Confluence, GitLab/GitHub). The knowledge layer (profiles, `q`/`for`,
`brief`, the indexer) is layered on top of this 

> **Heads-up:** this code was written without a Rust toolchain available, so it
> has **not been compiled yet**. Run `cargo build` (below) on your machine; if
> the first build surfaces errors, paste them and they'll be fixed quickly.

## Build

```bash
cd tools/palugada
cargo build --release          # binary at target/release/palugada
cargo install --path .         # optional: put `palugada` on your PATH
```

Requires Rust (stable). No async runtime — HTTP is synchronous via `ureq`.

## Setup flow

Credentials are entered **once** and live outside any repo; each project only
references an auth-profile by name.

```bash
# 1. scaffold global config + secrets (chmod 0600)
palugada config init

# 2. put your tokens into the auth-profiles file
#    ~/.palugada/secrets.yaml   (see example below)

# 3. drop a per-project config into your repo: <repo>/.palugada/config.yaml
#    (see examples/project.config.example.yaml)

# 4. register the project and make it active
palugada project add ttsecuritas /Users/me/dev/ttsecuritas

# 5. test every configured connection
palugada config verify
```

## Commands (base)

| Command | What it does |
|---|---|
| `palugada config init` | create `~/.palugada.yaml` + `~/.palugada/secrets.yaml` |
| `palugada config show` | print config + **masked** credentials |
| `palugada config verify` | connectivity + auth check for the active project's providers |
| `palugada project add <name> <repo_path>` | register a project |
| `palugada project list` | list registered projects (`*` = active) |
| `palugada project use <name>` | set the active project |
| `palugada issue view <KEY>` | fetch an issue (Jira) |
| `palugada wiki page <ID>` | fetch a page (Confluence) |
| `palugada git whoami` | authenticated git-host user (GitLab/GitHub) |

Global flags: `--project <name>` (override active), `--insecure` (accept
self-signed TLS for corporate hosts).

## `~/.palugada/secrets.yaml` (example — never commit)

```yaml
auth_profiles:
  tuntun-corp:
    jira_token: "PASTE_JIRA_BEARER_TOKEN"
    wiki_token: "PASTE_CONFLUENCE_BEARER_TOKEN"
    git_token:  "PASTE_GITLAB_PAT"
```

## `<repo>/.palugada/config.yaml` (example)

See [`examples/project.config.example.yaml`](examples/project.config.example.yaml).
The provider for each integration is swappable (`jira`/`github_issues`,
`confluence`/`notion`, `gitlab`/`github`); only the providers in the table above
are implemented in this base slice.

## Layout

```
src/
├── main.rs            clap dispatch + command handlers
├── config.rs          GlobalConfig / Secrets / ProjectConfig + resolution
├── http.rs            ureq helper (Bearer/header auth, --insecure TLS)
└── clients/
    ├── mod.rs         capability traits (IssueTracker/DocSource/GitHost) + factories
    ├── jira.rs        IssueTracker (Jira REST v2)
    ├── confluence.rs  DocSource (Confluence storage body)
    ├── gitlab.rs      GitHost (GitLab /api/v4)
    └── github.rs      GitHost (GitHub /user)
knowledge/profiles/    bundled stack profiles (android-mvvm starter)
```

## Roadmap (next)

- Figma / CI (Jenkins/GH Actions) / chat (DingTalk/Slack) behind their traits.
- `palugada init` — instant, offline scaffolding of agent files
  (`CLAUDE.md`/`AGENTS.md`/`GEMINI.md`) per the bound profile.
- Knowledge layer: `q`, `for`, `brief <flow>`, and the generic indexer.
