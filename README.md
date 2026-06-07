# palugada

Project-agnostic developer knowledge & connector CLI — one binary that gives
any project:

- **Connectors** to the tools your team already uses — Jira, Confluence,
  GitLab/GitHub, Figma, Jenkins — behind provider-agnostic traits, so the same
  command works regardless of vendor.
- **A knowledge layer** — stack conventions (`q`), task recipes (`for`), and
  keyword search (`s`) read from bundled profiles (android-mvvm starter).
- **A local code indexer** — `index` scans your repo into
  `<repo>/.palugada/index/`; `symbol` searches it.
- **Budgeted context packs** — `brief <flow>` assembles conventions + recipe +
  indexed facts into a token-budgeted pack for AI-agent work (bugfix, feature,
  refactor, review).
- **Offline scaffolding** — `palugada init` drops a per-project config and
  agent instruction files (Claude/Codex/Gemini/Cursor) into any repo in one
  command, no network needed.

## What you can do

| Goal | Commands |
|---|---|
| Wire a repo up for AI agents in one shot | `palugada init` |
| Ask "how do we do X here?" | `palugada q architecture`, `palugada s error` |
| Get a step-by-step recipe for a task | `palugada for feature` |
| Search your code's symbols | `palugada index`, then `palugada symbol LoginViewModel` |
| Build a context pack for a bugfix/feature | `palugada brief bugfix path/to/File.kt` |
| Pull a ticket / wiki page / design file | `palugada issue view`, `wiki page`, `design file` |
| Check CI / git identity | `palugada ci status <JOB>`, `palugada git whoami` |
| Verify every configured connection | `palugada config verify` |

## Build

Prerequisite: a stable Rust toolchain. If you don't have one yet:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # installs rustup + cargo
```

Then build, smoke-test, and optionally install on your PATH:

```bash
cd tools/palugada
cargo build --release            # compiles; binary at ./target/release/palugada
./target/release/palugada --help # sanity check

cargo install --path .           # optional: install `palugada` into ~/.cargo/bin
```

No async runtime — HTTP is synchronous via `ureq`. The first build downloads
crates, so it needs network access once.

## Quick start

Credentials are entered **once** and live outside any repo; each project only
references an auth-profile by name.

```bash
# 1. scaffold global config + secrets (chmod 0600)
palugada config init

# 2. put your tokens into ~/.palugada/secrets.yaml (see example below)

# 3. scaffold your repo: per-project config + agent files + registration
cd /Users/me/dev/my-app
palugada init                    # auto-detects the stack profile

# 4. test every configured connection
palugada config verify
```

Prefer manual setup? Instead of step 3, copy
[`examples/project.config.example.yaml`](examples/project.config.example.yaml)
to `<repo>/.palugada/config.yaml` and run
`palugada project add my-app /Users/me/dev/my-app`.

### What `palugada init` generates

```bash
palugada init [--repo .] [--name my-app] [--profile android-mvvm] \
              [--auth default] [--agents claude,codex,gemini,cursor] [--force]
```

| Target | Files written |
|---|---|
| (always) | `<repo>/.palugada/config.yaml` + registration in `~/.palugada.yaml` |
| `claude` (default) | `CLAUDE.md` + `.claude/skills/{bugfix,feature,refactor,review}/SKILL.md` |
| `codex` | `AGENTS.md` |
| `gemini` | `GEMINI.md` |
| `cursor` | `.cursor/rules/palugada.mdc` |

The stack profile is auto-detected (Gradle files → `android-mvvm`,
`package.json` → `web-react`); existing files are skipped unless `--force`.
Everything is offline — tokens stay in `~/.palugada/secrets.yaml`.

## Commands

| Command | What it does |
|---|---|
| `palugada init` | scaffold a repo: config + agent files + registration (offline) |
| `palugada config init` | create `~/.palugada.yaml` + `~/.palugada/secrets.yaml` |
| `palugada config show` | print config + **masked** credentials |
| `palugada config verify` | connectivity + auth check for the active project's providers |
| `palugada project add <name> <repo_path>` | register a project |
| `palugada project list` | list registered projects (`*` = active) |
| `palugada project use <name>` | set the active project |
| `palugada q <topic>[.N]` | read a convention from the active profile (`-b` outline, `--list`) |
| `palugada for <task>` | read a recipe from the active profile (`--list`) |
| `palugada s <kw>` | search conventions + recipes by keyword |
| `palugada index` | scan the project's code → `<repo>/.palugada/index/` (local, per-dev) |
| `palugada symbol <query>` | search indexed symbols by name |
| `palugada brief <flow> [target]` | one budgeted context pack for a flow (`--budget`, `--json`) |
| `palugada issue view <KEY>` | fetch an issue (Jira) |
| `palugada wiki page <ID>` | fetch a page (Confluence) |
| `palugada git whoami` | authenticated git-host user (GitLab/GitHub) |
| `palugada design file <KEY>` | a design file's metadata (Figma) |
| `palugada ci status <JOB>` | last build status of a CI job (Jenkins) |

Global flags: `--project <name>` (override active), `--insecure` (accept
self-signed TLS for corporate hosts).

## Using the knowledge layer

`q` / `for` / `s` read the bundled profile under `knowledge/profiles/`. The CLI
finds that directory via (in order) the `PALUGADA_KNOWLEDGE` env var,
`engine.knowledge_path` in `~/.palugada.yaml` (auto-recorded by `palugada config
init` when run from the repo), or by walking up from the binary. The active
profile resolves from the project's config → `defaults.profile` → the sole
bundled profile.

```bash
palugada q --list                 # what topics does this profile cover?
palugada q architecture           # full convention; `q architecture.2` = one section
palugada for feature              # recipe: how to build a feature here
palugada s viewmodel              # keyword search across conventions + recipes
```

`brief <flow>` runs the step list declared under `flows:` in the profile and
packs the result within `--budget` tokens. Example:

```bash
palugada index                          # once, to populate facts
palugada brief bugfix path/to/File.kt   # recent commits + symbols + errorhandling/testing
palugada brief feature TICKET-123 --budget 1500 --json
```

## `~/.palugada/secrets.yaml` (example — never commit)

```yaml
auth_profiles:
  default:
    jira_token:    "PASTE_JIRA_BEARER_TOKEN"
    wiki_token:    "PASTE_CONFLUENCE_BEARER_TOKEN"
    git_token:     "PASTE_GIT_PAT"
    figma_token:   "PASTE_FIGMA_TOKEN"
    jenkins_user:  "your-username"
    jenkins_token: "PASTE_JENKINS_API_TOKEN"
```

## `<repo>/.palugada/config.yaml` (example)

See [`examples/project.config.example.yaml`](examples/project.config.example.yaml).
The provider for each integration is swappable (`jira`/`github_issues`,
`confluence`/`notion`, `gitlab`/`github`); only the providers in the table above
are implemented today.

## Layout

```
src/
├── main.rs            clap dispatch + command handlers
├── config.rs          GlobalConfig / Secrets / ProjectConfig + resolution
├── http.rs            ureq helper (Bearer/header auth, --insecure TLS)
├── scaffold.rs        `palugada init` — offline agent-file + config scaffolding
├── knowledge.rs       `q` / `for` / `s` — read conventions/recipes from a profile
├── indexer.rs         `index` / `symbol` — scan code → <repo>/.palugada/index/
├── brief.rs           `brief` — budgeted flow context packs
└── clients/
    ├── mod.rs         capability traits (IssueTracker/DocSource/GitHost/DesignSource/CiProvider) + factories
    ├── jira.rs        IssueTracker (Jira REST v2)
    ├── confluence.rs  DocSource (Confluence storage body)
    ├── gitlab.rs      GitHost (GitLab /api/v4)
    ├── github.rs      GitHost (GitHub /user)
    ├── figma.rs       DesignSource (Figma files + /me)
    └── jenkins.rs     CiProvider (Jenkins job status)
knowledge/profiles/    bundled stack profiles (android-mvvm starter)
```

## Roadmap (next)

- Flesh out the remaining `brief` flow steps: `prd.context` (issue/wiki
  tie-in), `module.info`, and `diff.scan` (for `review`).
- Typed fact aliases over the index (`viewmodel` / `service` / `route` …) and
  richer extractors (tree-sitter where regex is too coarse).
- More providers as demand dictates (GitHub Actions / GitLab CI for `ci`,
  Notion for `wiki`, GitHub Issues / Linear for `issue_tracker`).

There is **no `sync`**: the index is local to each developer — `palugada index`
regenerates it from the local checkout; nothing is pulled from a shared corpus.

Done so far: connectors (Jira / Confluence / Figma / Jenkins / GitLab / GitHub),
`palugada init` (offline multi-agent scaffolding), knowledge reads
(`q` / `for` / `s`), the project indexer (`index` + `symbol`), and flow context
packs (`brief` — the `bugfix` flow is fully wired end-to-end).
