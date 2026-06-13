# palugada

[![CI](https://github.com/yudistirosaputro/palugada-cli/actions/workflows/ci.yml/badge.svg)](https://github.com/yudistirosaputro/palugada-cli/actions/workflows/ci.yml)

Project-agnostic developer knowledge & connector CLI — one binary that gives
any project:

- **Connectors** to the tools your team already uses — Jira, Confluence,
  GitLab/GitHub, Figma, Jenkins — behind provider-agnostic traits, so the same
  command works regardless of vendor.
- **A knowledge layer** — stack conventions (`q`), task recipes (`for`), and
  keyword search (`s`) read from bundled profiles (android-mvvm starter).
- **A local code indexer** — `index` scans your repo into
  `<repo>/.palugada/index/`; `symbol` searches it. Extraction is per
  fact-family: structural **tree-sitter** queries (Kotlin today) with regex for
  the long tail.
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

## Install

### Quick install (prebuilt — no clone, no Rust toolchain)

```bash
curl -fsSL https://raw.githubusercontent.com/yudistirosaputro/palugada-cli/main/install.sh | sh
```

This downloads the right prebuilt archive for your OS/arch from the
[Releases](https://github.com/yudistirosaputro/palugada-cli/releases) page,
installs the binary to `~/.local/bin`, and keeps the bundled `knowledge/`
profiles next to it so `q` / `for` / `s` / `brief` work immediately.

### Package managers

```bash
# npm (any OS with Node) — installs the right native binary automatically
npm install -g palugada-cli        # or run ad-hoc: npx palugada-cli q --list

# Homebrew (macOS / Linux)
brew install yudistirosaputro/tap/palugada

# Scoop (Windows)
scoop bucket add palugada https://github.com/yudistirosaputro/scoop-bucket
scoop install palugada
```

All three bundle the `knowledge/` profiles and wire them up automatically, so
`q` / `for` / `s` / `brief` work right after install. Publishing each channel is
opt-in — see [docs/PUBLISHING.md](docs/PUBLISHING.md).

### Manual download

Grab an archive for your platform from the
[latest release](https://github.com/yudistirosaputro/palugada-cli/releases/latest)
and extract it — the binary and its `knowledge/` dir ship together:

```bash
# example: macOS Apple Silicon
mkdir -p ~/.local/share/palugada
curl -fsSL -o p.tar.gz \
  https://github.com/yudistirosaputro/palugada-cli/releases/latest/download/palugada-aarch64-apple-darwin.tar.gz
tar xzf p.tar.gz -C ~/.local/share/palugada
ln -sf ~/.local/share/palugada/palugada ~/.local/bin/palugada   # if ~/.local/bin is on PATH
palugada --help
```

Archives are published for Linux x86_64, macOS arm64, macOS x86_64, and Windows
x86_64. **Keep the binary next to its `knowledge/` dir** — it locates the bundled
profiles by walking up from its own path (symlinks onto `PATH` are resolved). If
you move the bare binary elsewhere, point `PALUGADA_KNOWLEDGE` at the bundled
`knowledge/` directory.

## Build (from source)

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
| `palugada project remove <name>` | unregister a project (files on disk untouched) |
| `palugada profile list/validate/new` | list, lint, or scaffold a stack profile |
| `palugada q <topic>[.N]` | read a convention from the active profile (`-b` outline, `--list`) |
| `palugada for <task>` | read a recipe from the active profile (`--list`) |
| `palugada s <kw>` | search conventions + recipes by keyword |
| `palugada index` | scan the project's code → `<repo>/.palugada/index/` (local, per-dev) |
| `palugada symbol <query>` | search indexed symbols by name |
| `palugada fact <family> [name]` | look up indexed facts of a profile-declared family (e.g. `fact viewmodel Login`) |
| `palugada brief <flow> [target]` | one budgeted context pack for a flow (`--budget`, `--json`) |
| `palugada issue view <KEY>` | fetch an issue (Jira) |
| `palugada wiki page <ID>` | fetch a page (Confluence) |
| `palugada git whoami` | authenticated git-host user (GitLab/GitHub) |
| `palugada pr recent <file>` | recent commits touching a file, from the git host (needs `repo`) |
| `palugada design file <KEY>` | a design file's metadata (Figma) |
| `palugada ci status <JOB>` | last build status of a CI job (Jenkins) |
| `palugada notify <msg>` | send a message to the project's chat (Slack webhook) |
| `palugada prd fetch/list/cat/search` | personal corpus of fetched tickets in `~/.palugada/personal/` |
| `palugada exec <verb> [k=v…]` | run a profile/project-declared shell verb (`--list`, `--json`) |
| `palugada doctor` | check tool + connector readiness (`--json`); non-zero exit on failure |

Global flags: `--project <name>` (override active), `--insecure` (accept
self-signed TLS for corporate hosts), `--version`. Every invocation needs a home
directory — `HOME`, or `%USERPROFILE%` on Windows — to locate `~/.palugada.yaml`
and `~/.palugada/secrets.yaml`.

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

## Running tasks & diagnostics (`exec`, `doctor`)

`exec` runs named **verbs** — shell command sequences declared under `exec:` in
the active profile and/or `<repo>/.palugada/config.yaml`. The project's map
overrides the profile's per verb. `{key}` placeholders are filled from `k=v` args.

```bash
palugada exec --list                       # verbs available in this repo
palugada exec build                        # run the `build` verb
palugada exec install apk=app/out.apk      # fill {apk} from the k=v arg
palugada exec test --json                  # capture a JSON outcome instead of streaming
```

Each verb may set `timeout_secs` (default 600; `0` = unlimited). palugada exits
with the child's exit code (a timeout exits 124 and kills the whole process
group), so agents and CI can branch on it.

`doctor` checks repo readiness: it runs the `doctor` verb (tool checks) and, when
a project + connectors resolve, verifies each connector. It exits non-zero if any
check fails; `--json` emits `{ok, checks[]}`.

```bash
palugada doctor
palugada doctor --json
```

## `~/.palugada/secrets.yaml` (example — never commit)

```yaml
auth_profiles:
  default:
    # Atlassian Cloud: set the *_email fields to use Basic auth (email + API
    # token). Leave them empty for Server/Data Center, which uses a Bearer PAT.
    jira_email:    "you@example.com"
    jira_token:    "PASTE_JIRA_API_TOKEN_OR_PAT"
    wiki_email:    "you@example.com"
    wiki_token:    "PASTE_CONFLUENCE_API_TOKEN_OR_PAT"
    git_token:     "PASTE_GIT_PAT"
    figma_token:   "PASTE_FIGMA_TOKEN"
    jenkins_user:  "your-username"
    jenkins_token: "PASTE_JENKINS_API_TOKEN"
    chat_webhook:  "https://hooks.slack.com/services/PASTE/WEBHOOK/URL"
```

## `<repo>/.palugada/config.yaml` (example)

See [`examples/project.config.example.yaml`](examples/project.config.example.yaml).
Each integration names a provider. Implemented today: issue tracker `jira` or
`github_issues` (set `repo: owner/name`); wiki `confluence` or `notion`; git host `gitlab`
or `github`; design `figma`; CI `jenkins`, `github_actions`, or `gitlab_ci`; chat
`slack`. Other providers (Notion, Linear, …) are roadmap only — selecting one is
a hard error.

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
├── exec.rs            `exec` / `doctor` — profile/project shell verbs + JSON outcome
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

- Wiki tie-in for `prd.context` (a ticket's linked Confluence/Notion spec) — the
  `feature` flow currently packs the issue summary + description only.
- More tree-sitter grammars (Swift, TS, Go, Python) and typed fact aliases
  (`viewmodel` / `service` …) layered over the generic `fact` command.
- More providers as demand dictates (Linear for `issue_tracker`, Teams/DingTalk
  for `notify`).

There is **no `sync`**: the index is local to each developer — `palugada index`
regenerates it from the local checkout; nothing is pulled from a shared corpus.

Done so far: connectors (Jira / Confluence / Figma / Jenkins / GitLab / GitHub),
`palugada init` (offline multi-agent scaffolding), knowledge reads
(`q` / `for` / `s`), the project indexer (`index` + `symbol` + `fact`), and flow
context packs (`brief` — all four flows wired: bugfix, feature, refactor, review,
with a priority-fill token budget).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for the local build/test workflow, coding
conventions, and how to add a connector or knowledge profile. Maintainers: see
[docs/PUBLISHING.md](docs/PUBLISHING.md) for releases.

## License

[MIT](LICENSE) © 2026 Yudistiro Saputro.
