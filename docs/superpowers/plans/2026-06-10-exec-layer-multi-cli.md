# Palugada v2 — Exec Layer + Multi-CLI Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make palugada the substrate for a complete AI-agent loop (plan → code → execute → test → bugfix → review) in any project from any AI CLI, with android-cli as the Android execution backend — plus all confirmed review-finding fixes and a test suite.

**Architecture:** Three pillars in one cold-start Rust binary: KNOW (q/for/s/index/symbol/brief — fixed + six flows), CONNECT (existing trait clients + Atlassian Cloud auth), EXEC (new `src/exec.rs`: profile/project-declared verb→command templates run via `sh -c`, exit code propagated, `--json` outcome). Scaffolded agent files become managed marker blocks generated from a data-driven target table.

**Tech Stack:** Rust 2021, clap 4, ureq 2, serde_yaml, regex, walkdir; dev-deps: tempfile 3. Spec: `docs/superpowers/specs/2026-06-10-exec-layer-multi-cli-design.md`.

**Conventions for every task:** run commands from the repo root `/Users/septiandwisaputro/Documents/project/tools/palugada`. `cargo test` must be green before every commit. Unit tests live in `#[cfg(test)] mod tests` at the bottom of the file they test.

---

## File structure (what's created/modified)

| Path | Role |
|---|---|
| `src/config.rs` (modify) | + `CmdSpec`/`VerbSpec`, `AuthProfile.{jira_email,wiki_email}`, `ProjectConfig.exec`, `resolve_project_name`, `resolve_repo` (moved here), 0600 atomic secrets, util fixes |
| `src/exec.rs` (create) | exec verb merge, `{k}` substitution, runner w/ timeout + tail, outcome JSON |
| `src/http.rs` (modify) | overall timeout, URL in errors, `encode_segment` |
| `src/clients/mod.rs` (modify) | + `atlassian_auth` helper |
| `src/clients/{jira,confluence}.rs` (modify) | email param, Basic-or-Bearer, base_url guard, encoded ids |
| `src/clients/jenkins.rs` (modify) | folder-aware `job_path` |
| `src/clients/{figma}.rs` (modify) | encoded key |
| `src/indexer.rs` (modify) | family-id validation, clear index dir |
| `src/knowledge.rs` (modify) | fence-aware `sections`, `topics_matching_tags` |
| `src/brief.rs` (rewrite) | budget-correct `select_packs`, steps: issue.context / diff.scan / exec.hints, `--diff`, `ConnectorCtx` |
| `src/scaffold.rs` (rewrite) | managed marker blocks, target table, six flows, namespaced skills, `sync` |
| `src/profiles.rs` (create) | `profile list` / `profile validate` |
| `src/main.rs` (modify) | new commands: Exec/Doctor/Skills/Profile, ProjectCmd::Remove, resolution rewiring, HOME guard, --insecure warning |
| `knowledge/profiles/android-mvvm/*` (modify) | six flows + `exec:`; + conventions errorhandling/testing/style; + recipe refactor |
| `knowledge/profiles/web-react/*` (create) | full second profile |
| `knowledge/profiles/generic/*` (create) | stack-agnostic fallback profile |
| `tests/cli.rs` (create) | end-to-end binary tests |
| `README.md`, `PRD-unified-palugada.md` (modify) | document the new surface |

**Shared signatures defined by this plan (use exactly these):**

```rust
// config.rs
pub fn resolve_project_name(global: &GlobalConfig, project_override: Option<&str>, cwd: &Path) -> Result<String, String>;
pub fn resolve_repo(global: &GlobalConfig, project_override: Option<&str>, repo_flag: Option<String>, cwd: &Path) -> Result<PathBuf, String>;
pub enum CmdSpec { One(String), Many(Vec<String>) }
pub enum VerbSpec { Simple(String), Full { cmd: CmdSpec, timeout_secs: u64 } }   // untagged
impl VerbSpec { pub fn commands(&self) -> Vec<String>; pub fn timeout_secs(&self) -> u64; }
// clients/mod.rs
pub fn atlassian_auth(email: &str, token: &str) -> String;
// http.rs
pub fn encode_segment(s: &str) -> String;
// exec.rs
pub fn merged_verbs(kn: Option<&Path>, profile: &str, repo: &Path) -> Result<BTreeMap<String, (VerbSpec, &'static str)>, String>;
pub fn parse_kv_args(args: &[String]) -> Result<BTreeMap<String, String>, String>;
pub fn substitute(template: &str, args: &BTreeMap<String, String>) -> Result<String, String>;
pub fn run_verb(verbs: &BTreeMap<String, (VerbSpec, &'static str)>, repo: &Path, req: &ExecRequest) -> Result<ExecOutcome, String>;
pub fn run_one_captured(cmd_str: &str, repo: &Path, timeout: Duration, out_buf: &mut String) -> Result<i32, String>;
pub fn tail_lines(s: &str, n: usize) -> String;
// knowledge.rs
pub fn topics_matching_tags(kn: &Path, profile: &str, keys: &BTreeSet<String>) -> Vec<(String, String)>;
// brief.rs
pub struct ConnectorCtx { pub pc: Option<ProjectConfig>, pub auth: Option<AuthProfile>, pub insecure: bool }
pub const KNOWN_STEP_KINDS: &[&str];
// scaffold.rs
pub fn upsert_managed(existing: &str, block: &str) -> String;
pub fn sync(repo: String, agents_csv: String) -> Result<(), String>;
// profiles.rs
pub fn list(kn: &Path) -> Result<(), String>;
pub fn validate(kn: &Path, id: &str) -> Result<(), String>;
```

---

### Task 0: Branch + dev-dependency

**Files:** Modify: `Cargo.toml`

- [ ] **Step 1: Create a working branch**

```bash
git checkout -b feat/exec-layer-multi-cli
```

- [ ] **Step 2: Add tempfile dev-dependency** — append to `Cargo.toml` after the `[dependencies]` block:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Verify it builds**

Run: `cargo build`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add tempfile dev-dependency for tests"
```

---

### Task 1: Config utility fixes (mask_secret, expand_home, 0600 secrets, HOME guard)

**Files:**
- Modify: `src/config.rs`
- Modify: `src/main.rs:218` (run), `src/main.rs:411` (config init message)

- [ ] **Step 1: Write failing unit tests** — append to the bottom of `src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_secret_hides_everything_and_is_utf8_safe() {
        assert_eq!(mask_secret(""), "(unset)");
        let m = mask_secret("abcd1234");
        assert!(!m.contains("ab") && !m.contains("34"), "no leading/trailing chars: {m}");
        // multi-byte secret must not panic (old code sliced bytes)
        let m = mask_secret("ключключключ");
        assert!(m.starts_with("****"), "{m}");
    }

    #[test]
    fn expand_home_handles_bare_tilde() {
        assert_eq!(expand_home("~"), home_dir());
        assert_eq!(expand_home("~/x"), home_dir().join("x"));
        assert_eq!(expand_home("/abs/path"), Path::new("/abs/path").to_path_buf());
    }

    #[test]
    fn secrets_save_is_0600_and_round_trips() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nested").join("secrets.yaml");
        let mut s = Secrets::default();
        s.auth_profiles.insert("default".into(), AuthProfile { jira_token: "t".into(), ..Default::default() });
        s.save_to_path(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode was {mode:o}");
        let loaded = Secrets::load_from_path(&p).unwrap();
        assert_eq!(loaded.auth_profiles["default"].jira_token, "t");
        // re-save over an existing file keeps 0600
        s.save_to_path(&p).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib config::tests -- --nocapture` (note: until `main.rs` has `mod` items unchanged this is `cargo test config::tests`)
Expected: FAIL — `mask_secret` panics or keeps chars; `expand_home("~")` returns `"~"`; `save_to_path`/`load_from_path` don't exist (compile error first; that counts as the failing state).

- [ ] **Step 3: Implement** — in `src/config.rs`:

Replace `mask_secret` (lines 278-287) with:

```rust
/// Mask a secret for display. Reveals nothing but presence and length.
pub fn mask_secret(s: &str) -> String {
    if s.is_empty() {
        "(unset)".to_string()
    } else {
        format!("**** ({} chars)", s.chars().count())
    }
}
```

Replace `expand_home` (lines 270-276) with:

```rust
pub fn expand_home(s: &str) -> PathBuf {
    if s == "~" {
        home_dir()
    } else if let Some(stripped) = s.strip_prefix("~/") {
        home_dir().join(stripped)
    } else {
        Path::new(s).to_path_buf()
    }
}
```

Replace `Secrets::load_or_default` and `Secrets::save` (lines 144-163) with:

```rust
    pub fn load_from_path(p: &Path) -> Result<Secrets, String> {
        if !p.exists() {
            return Ok(Secrets::default());
        }
        let data = fs::read_to_string(p).map_err(|e| format!("read {}: {e}", p.display()))?;
        serde_yaml::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
    }

    pub fn load_or_default() -> Result<Secrets, String> {
        Self::load_from_path(&Self::default_path())
    }

    /// Write the secrets file, created 0600 so tokens are never world-readable.
    pub fn save_to_path(&self, p: &Path) -> Result<(), String> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        if let Some(dir) = p.parent() {
            fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        let data = serde_yaml::to_string(self).map_err(|e| e.to_string())?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(p)
            .map_err(|e| format!("open {}: {e}", p.display()))?;
        f.write_all(data.as_bytes())
            .map_err(|e| format!("write {}: {e}", p.display()))?;
        fs::set_permissions(p, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("chmod {}: {e}", p.display()))?;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        self.save_to_path(&Self::default_path())
    }
```

In `src/main.rs` `run()` (line 218), add a HOME guard as the first statement of the function body:

```rust
fn run(cli: Cli) -> Result<(), String> {
    if std::env::var("HOME").map(|h| h.is_empty()).unwrap_or(true) {
        return Err("HOME is not set — palugada needs it to locate ~/.palugada.yaml and ~/.palugada/secrets.yaml".into());
    }
    let project = cli.project.as_deref();
    ...
```

In `src/main.rs` `cmd_config` `ConfigCmd::Init` (lines 411-415), fix the misleading chmod message:

```rust
            println!(
                "Wrote {} and {} (secrets chmod 0600).",
                GlobalConfig::default_path().display(),
                Secrets::default_path().display()
            );
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS (3 new tests).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "fix: harden config utils — full secret masking, bare-tilde expansion, atomic 0600 secrets, HOME guard"
```

---

### Task 2: Resolution fixes (cwd-first, hard errors on typos, auth-profile errors, project add/remove)

**Files:**
- Modify: `src/config.rs` (resolve_project_name, resolve_project, resolve_repo)
- Modify: `src/main.rs` (delete local resolve_repo at lines 348-372; rewire resolve_profile lines 304-328; ProjectCmd Add/Remove lines 479-524)

- [ ] **Step 1: Write failing unit tests** — add inside `src/config.rs` `mod tests`:

```rust
    fn global_with(name: &str, repo: &Path) -> GlobalConfig {
        let mut g = GlobalConfig::default();
        g.projects.registered.insert(
            name.to_string(),
            ProjectEntry { repo_path: repo.to_string_lossy().to_string(), workspace: String::new() },
        );
        g
    }

    #[test]
    fn resolve_project_name_prefers_cwd_over_active() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let mut g = global_with("aaa", a.path());
        g.projects.registered.insert(
            "bbb".into(),
            ProjectEntry { repo_path: b.path().to_string_lossy().to_string(), workspace: String::new() },
        );
        g.projects.active = "aaa".to_string();
        // cwd inside repo B → project bbb wins over active aaa
        let sub = b.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(resolve_project_name(&g, None, &sub).unwrap(), "bbb");
        // explicit --project always wins
        assert_eq!(resolve_project_name(&g, Some("aaa"), &sub).unwrap(), "aaa");
        // typo'd --project is a hard error naming known projects
        let err = resolve_project_name(&g, Some("nope"), &sub).unwrap_err();
        assert!(err.contains("nope") && err.contains("aaa"), "{err}");
        // no cwd match → active
        let other = tempfile::tempdir().unwrap();
        assert_eq!(resolve_project_name(&g, None, other.path()).unwrap(), "aaa");
    }

    #[test]
    fn resolve_repo_expands_tilde_and_errors_on_unknown_project() {
        let a = tempfile::tempdir().unwrap();
        let g = global_with("aaa", a.path());
        let cwd = tempfile::tempdir().unwrap();
        // unknown --project: hard error (old code silently fell back to cwd)
        assert!(resolve_repo(&g, Some("nope"), None, cwd.path()).is_err());
        // --repo flag wins and expands ~
        let r = resolve_repo(&g, None, Some("~/somewhere".into()), cwd.path()).unwrap();
        assert_eq!(r, home_dir().join("somewhere"));
        // fallback: plain cwd
        let r = resolve_repo(&g, None, None, cwd.path()).unwrap();
        assert_eq!(r, cwd.path().to_path_buf());
    }

    #[test]
    fn resolve_project_errors_on_unknown_auth_profile() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join(".palugada")).unwrap();
        std::fs::write(
            repo.path().join(".palugada").join("config.yaml"),
            "project: aaa\nprofile: generic\nauth_profile: typo\n",
        )
        .unwrap();
        let mut g = global_with("aaa", repo.path());
        g.projects.active = "aaa".to_string();
        let mut secrets = Secrets::default();
        secrets.auth_profiles.insert("default".into(), AuthProfile::default());
        let err = resolve_project(&g, &secrets, Some("aaa")).unwrap_err();
        assert!(err.contains("typo") && err.contains("default"), "{err}");
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test`
Expected: compile error — `resolve_project_name` / `resolve_repo` not in config.rs. That is the failing state.

- [ ] **Step 3: Implement in `src/config.rs`** — replace the whole `resolve_project` function (lines 234-260) with:

```rust
/// Resolve the target project name (PRD §5.4): explicit `--project` (must be
/// registered) → the registered project whose repo contains `cwd` → the
/// registry's `active`.
pub fn resolve_project_name(
    global: &GlobalConfig,
    project_override: Option<&str>,
    cwd: &Path,
) -> Result<String, String> {
    if let Some(name) = project_override.filter(|s| !s.is_empty()) {
        if !global.projects.registered.contains_key(name) {
            let known: Vec<&str> = global.projects.registered.keys().map(String::as_str).collect();
            return Err(format!(
                "project '{name}' is not registered — known projects: {}",
                if known.is_empty() { "(none)".to_string() } else { known.join(", ") }
            ));
        }
        return Ok(name.to_string());
    }
    for (name, entry) in &global.projects.registered {
        if !entry.repo_path.is_empty() && cwd.starts_with(expand_home(&entry.repo_path)) {
            return Ok(name.clone());
        }
    }
    if !global.projects.active.is_empty() {
        return Ok(global.projects.active.clone());
    }
    Err("no active project — run `palugada project use <name>`, pass --project, or cd into a registered repo".into())
}

/// Resolve the target project's config plus the referenced auth-profile.
pub fn resolve_project(
    global: &GlobalConfig,
    secrets: &Secrets,
    project_override: Option<&str>,
) -> Result<(String, ProjectConfig, AuthProfile), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let name = resolve_project_name(global, project_override, &cwd)?;
    let entry = global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered — run `palugada project add {name} <repo_path>`"))?;

    let pc = ProjectConfig::load_from(&entry.repo_path)?;
    let auth = if pc.auth_profile.is_empty() {
        AuthProfile::default()
    } else {
        secrets.auth_profiles.get(&pc.auth_profile).cloned().ok_or_else(|| {
            let known: Vec<&str> = secrets.auth_profiles.keys().map(String::as_str).collect();
            format!(
                "auth profile '{}' (referenced by project '{name}') not found in {} — known profiles: {}",
                pc.auth_profile,
                Secrets::default_path().display(),
                if known.is_empty() { "(none)".to_string() } else { known.join(", ") }
            )
        })?
    };
    Ok((name, pc, auth))
}

/// Resolve the repo to operate on: explicit `--repo` → explicit `--project`
/// (must be registered) → cwd inside a registered repo → active project → cwd.
pub fn resolve_repo(
    global: &GlobalConfig,
    project_override: Option<&str>,
    repo_flag: Option<String>,
    cwd: &Path,
) -> Result<PathBuf, String> {
    if let Some(r) = repo_flag {
        if !r.is_empty() {
            return Ok(expand_home(&r));
        }
    }
    match resolve_project_name(global, project_override, cwd) {
        Ok(name) => {
            if let Some(e) = global.projects.registered.get(&name) {
                if !e.repo_path.is_empty() {
                    return Ok(expand_home(&e.repo_path));
                }
            }
            Ok(cwd.to_path_buf())
        }
        // explicit --project typo must surface; "no active project" falls back to cwd
        Err(e) if project_override.is_some() => Err(e),
        Err(_) => Ok(cwd.to_path_buf()),
    }
}
```

- [ ] **Step 4: Rewire `src/main.rs`:**

(a) Delete the local `resolve_repo` (lines 347-372). Update its three call sites to:

```rust
// in cmd_index:
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, repo, &cwd)?;
    indexer::run(&repo_path, &kn, &prof)
// in cmd_symbol:
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, None, &cwd)?;
    indexer::symbol_search(&repo_path, &query)
// in cmd_brief (full rewrite comes in Task 9; for now):
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    brief::run(&kn, &repo, &prof, &brief::BriefOptions { flow, target, budget, json })
```

(`config::resolve_repo` returns `PathBuf`; `indexer::run`/`symbol_search`/`brief::run` already take `&Path`.) Add `use std::path::Path;` only if the compiler asks. The import line at `src/main.rs:16-18` becomes:

```rust
use config::{mask_secret, resolve_project, GlobalConfig, ProjectEntry, Secrets};
```

(unchanged) plus prefix calls with `config::` where shown.

(b) Replace `resolve_profile` (lines 304-328) with — this stops swallowing project-config parse errors and drops the needless secrets load that broke offline `q`/`for`/`s`:

```rust
/// Resolve which profile to read: explicit flag → the resolved project's
/// profile (cwd-aware; parse errors surface) → global default → sole profile.
fn resolve_profile(
    global: &GlobalConfig,
    project: Option<&str>,
    profile_flag: Option<&str>,
    kn: &std::path::Path,
) -> Result<String, String> {
    if let Some(p) = profile_flag {
        if !p.is_empty() {
            return Ok(p.to_string());
        }
    }
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let name = if project.is_some() {
        Some(config::resolve_project_name(global, project, &cwd)?)
    } else {
        config::resolve_project_name(global, None, &cwd).ok()
    };
    if let Some(name) = name {
        if let Some(entry) = global.projects.registered.get(&name) {
            let pc = config::ProjectConfig::load_from(&entry.repo_path)?;
            if !pc.profile.is_empty() {
                return Ok(pc.profile);
            }
        }
    }
    if !global.defaults.profile.is_empty() {
        return Ok(global.defaults.profile.clone());
    }
    if let Some(only) = knowledge::only_profile(kn) {
        return Ok(only);
    }
    Err("no profile resolved — pass --profile <id>, set defaults.profile in ~/.palugada.yaml, or run `palugada init` in a project".to_string())
}
```

(c) In `ProjectCmd` enum (lines 170-178) add:

```rust
    /// Remove a project from the registry (files on disk are untouched).
    Remove { name: String },
```

(d) In `cmd_project`, replace the `Add` arm with path validation + collision warning, and add `Remove`:

```rust
        ProjectCmd::Add { name, repo_path } => {
            let mut global = GlobalConfig::load_or_default()?;
            let repo = std::fs::canonicalize(config::expand_home(&repo_path))
                .map_err(|e| format!("repo path not found ({repo_path}): {e}"))?;
            if !repo.is_dir() {
                return Err(format!("not a directory: {}", repo.display()));
            }
            let repo = repo.to_string_lossy().to_string();
            if let Some(existing) = global.projects.registered.get(&name) {
                if existing.repo_path != repo {
                    eprintln!(
                        "warning: project '{name}' was registered at {} — overwriting with {repo}",
                        existing.repo_path
                    );
                }
            }
            let workspace = format!("{repo}/.palugada");
            global
                .projects
                .registered
                .insert(name.clone(), ProjectEntry { repo_path: repo.clone(), workspace });
            let became_active = global.projects.active.is_empty();
            if became_active {
                global.projects.active = name.clone();
            }
            global.save()?;
            println!("Registered '{name}' -> {repo}");
            if became_active {
                println!("(set as the active project)");
            }
            Ok(())
        }
        ProjectCmd::Remove { name } => {
            let mut global = GlobalConfig::load_or_default()?;
            if global.projects.registered.remove(&name).is_none() {
                return Err(format!("project '{name}' is not registered"));
            }
            if global.projects.active == name {
                global.projects.active.clear();
            }
            global.save()?;
            println!("Removed '{name}'.");
            Ok(())
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: PASS (all, incl. the 3 new resolution tests).

- [ ] **Step 6: Commit**

```bash
git add src/config.rs src/main.rs
git commit -m "fix: cwd-first project resolution, hard errors on unknown --project/auth-profile, validated project add/remove"
```

---

### Task 3: Connector hardening (Atlassian Cloud auth, URL encoding, HTTP errors/timeout, --insecure warning)

**Files:**
- Modify: `src/config.rs` (AuthProfile), `src/clients/mod.rs`, `src/clients/jira.rs`, `src/clients/confluence.rs`, `src/clients/jenkins.rs`, `src/clients/figma.rs`, `src/http.rs`, `src/main.rs` (config show + insecure warning + factory args)

- [ ] **Step 1: Write failing unit tests**

Append to `src/clients/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlassian_auth_picks_basic_when_email_present() {
        assert_eq!(atlassian_auth("", "tok123"), "Bearer tok123");
        // base64("me@x.co:tok123") = bWVAeC5jbzp0b2sxMjM=
        assert_eq!(atlassian_auth("me@x.co", "tok123"), "Basic bWVAeC5jbzp0b2sxMjM=");
    }
}
```

Append to `src/http.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_segment_percent_encodes_reserved_chars() {
        assert_eq!(encode_segment("PROJ-123"), "PROJ-123");
        assert_eq!(encode_segment("a b/c?d"), "a%20b%2Fc%3Fd");
        assert_eq!(encode_segment("naïve"), "na%C3%AFve");
    }
}
```

Append to `src/clients/jenkins.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_path_handles_folders_and_encoding() {
        assert_eq!(job_path("app"), "app");
        assert_eq!(job_path("folder/app"), "folder/job/app");
        assert_eq!(job_path("team a/app"), "team%20a/job/app");
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test`
Expected: compile errors — `atlassian_auth`, `encode_segment`, `job_path` don't exist.

- [ ] **Step 3: Implement**

(a) `src/config.rs` — add two fields to `AuthProfile` (after `wiki_token`):

```rust
    /// Atlassian Cloud: account email; when set, Jira/Confluence use
    /// `Basic base64(email:token)` instead of `Bearer token`.
    #[serde(default)]
    pub jira_email: String,
    #[serde(default)]
    pub wiki_email: String,
```

(b) `src/clients/mod.rs` — add above the factories:

```rust
/// Authorization header value for Atlassian-style APIs: `email` present →
/// Basic base64(email:token) (Atlassian Cloud API tokens), else Bearer
/// (server / Data-Center PATs).
pub fn atlassian_auth(email: &str, token: &str) -> String {
    use base64::Engine as _;
    if email.is_empty() {
        format!("Bearer {token}")
    } else {
        let creds =
            base64::engine::general_purpose::STANDARD.encode(format!("{email}:{token}"));
        format!("Basic {creds}")
    }
}
```

and change the two Atlassian factories to pass emails:

```rust
        "jira" => Ok(Box::new(jira::Jira::new(&p.base_url, &auth.jira_email, &auth.jira_token, insecure))),
...
        "confluence" => Ok(Box::new(confluence::Confluence::new(
            &p.base_url,
            &auth.wiki_email,
            &auth.wiki_token,
            insecure,
        ))),
```

(c) `src/http.rs` — add `.timeout(Duration::from_secs(90))` to BOTH `AgentBuilder` chains (overall per-request cap; a trickling server can otherwise stall forever), change both `.map_err(describe_ureq)` to `.map_err(|e| describe_ureq(url, e))`, replace `describe_ureq` with:

```rust
/// Turn a ureq error into a readable message, surfacing URL + HTTP status.
fn describe_ureq(url: &str, e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(code, resp) => {
            let body = resp.into_string().unwrap_or_default();
            let snippet: String = body.chars().take(300).collect();
            format!("GET {url} -> HTTP {code}: {snippet}")
        }
        ureq::Error::Transport(t) => format!("GET {url}: transport error: {t}"),
    }
}
```

and add at the bottom (above `NoVerify`):

```rust
/// Percent-encode a single URL path segment (RFC 3986 unreserved kept).
pub fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
```

(d) `src/clients/jira.rs` — struct gains `email: String`; `new(base_url, email, token, insecure)` sets it; `headers()` becomes:

```rust
    fn headers(&self) -> Vec<(&str, String)> {
        if self.token.is_empty() {
            vec![]
        } else {
            vec![("Authorization", super::atlassian_auth(&self.email, &self.token))]
        }
    }
```

`get_issue` and `verify` each gain a guard as their first lines and encode the key:

```rust
        if self.base_url.is_empty() {
            return Err("jira base_url is empty in the project config (integrations.issue_tracker.base_url)".into());
        }
...
        let url = format!("{}/issue/{}", self.base_url, crate::http::encode_segment(key));
```

(e) `src/clients/confluence.rs` — same pattern: `email` field, `new(base_url, email, token, insecure)`, `atlassian_auth` in headers, base_url guard in `get_page`/`verify`, and `crate::http::encode_segment(id)` in the page URL.

(f) `src/clients/jenkins.rs` — add a module-level fn and use it in `job_status`:

```rust
/// Jenkins path for a possibly-foldered job: "a/b" → "a/job/b" (each segment
/// percent-encoded). The caller wraps it as /job/<this>/...
fn job_path(job: &str) -> String {
    job.split('/')
        .filter(|s| !s.is_empty())
        .map(|s| crate::http::encode_segment(s))
        .collect::<Vec<_>>()
        .join("/job/")
}
...
        let url = format!("{}/job/{}/lastBuild/api/json", self.base_url, job_path(job));
```

(g) `src/clients/figma.rs` — `get_file` URL becomes:

```rust
        let url = format!("{}/v1/files/{}", self.base_url, crate::http::encode_segment(key));
```

(h) `src/main.rs` — in `run()` right after the HOME guard, warn on `--insecure`:

```rust
    if cli.insecure {
        eprintln!("warning: --insecure accepts ANY TLS certificate for every host this run");
    }
```

and in `cmd_config` `ConfigCmd::Show`, print the new fields with the others:

```rust
                println!("    jira_email:    {}", if a.jira_email.is_empty() { "(unset)".into() } else { a.jira_email.clone() });
                println!("    wiki_email:    {}", if a.wiki_email.is_empty() { "(unset)".into() } else { a.wiki_email.clone() });
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: PASS (3 new test fns).

- [ ] **Step 5: Commit**

```bash
git add src/config.rs src/clients src/http.rs src/main.rs
git commit -m "fix: Atlassian Cloud Basic auth, URL-encoded identifiers, folder-aware Jenkins jobs, request timeout, --insecure warning"
```

---

### Task 4: Indexer fixes (family-id validation, clean re-index)

**Files:**
- Modify: `src/indexer.rs`

- [ ] **Step 1: Write failing tests** — append to `src/indexer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Build a throwaway knowledge dir with one profile + extractors.yaml.
    fn fixture(extractors_yaml: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(&prof).unwrap();
        fs::write(prof.join("extractors.yaml"), extractors_yaml).unwrap();
        let repo = tempfile::tempdir().unwrap();
        (kn, repo)
    }

    #[test]
    fn rejects_path_traversal_family_id() {
        let (kn, repo) = fixture(
            "families:\n  - id: \"../evil\"\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        );
        let err = run(repo.path(), kn.path(), "p").unwrap_err();
        assert!(err.contains("../evil"), "{err}");
    }

    #[test]
    fn reindex_clears_stale_family_files() {
        let (kn, repo) = fixture(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)ViewModel'\n",
        );
        fs::write(repo.path().join("A.kt"), "class LoginViewModel {}").unwrap();
        run(repo.path(), kn.path(), "p").unwrap();
        let idx = repo.path().join(".palugada").join("index");
        assert!(idx.join("viewmodel.json").exists());
        // family disappears from the code → its file must disappear from the index
        fs::write(repo.path().join("A.kt"), "class Login {}").unwrap();
        run(repo.path(), kn.path(), "p").unwrap();
        assert!(!idx.join("viewmodel.json").exists(), "stale viewmodel.json survived re-index");
        assert!(idx.join("symbols.json").exists());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test indexer`
Expected: `rejects_path_traversal_family_id` FAILS (no validation today) and `reindex_clears_stale_family_files` FAILS (stale file survives).

- [ ] **Step 3: Implement** — in `src/indexer.rs` `run()`:

(a) inside the `for f in &cfg.families` compile loop, validate the id first:

```rust
    for f in &cfg.families {
        let id_ok = !f.id.is_empty()
            && f.id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !id_ok {
            return Err(format!(
                "family id '{}' is invalid — use only [a-z0-9_-] (ids become index file names)",
                f.id
            ));
        }
        let re = Regex::new(&f.regex)
...
```

(b) replace the index-dir creation (lines 138-139) with a clean rebuild:

```rust
    let out = repo.join(".palugada").join("index");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("clear {}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/indexer.rs
git commit -m "fix: validate extractor family ids (path-traversal) and clear index dir on re-index"
```

---

### Task 5: Knowledge fixes (fence-aware sections, tag lookup for diff.scan)

**Files:**
- Modify: `src/knowledge.rs`

- [ ] **Step 1: Write failing tests** — append to `src/knowledge.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_ignores_headers_inside_code_fences() {
        let body = "## One\ntext\n```sh\n## not a header\n```\nmore\n## Two\nend\n";
        let secs = sections(body);
        assert_eq!(secs.len(), 2, "{:?}", secs.iter().map(|s| &s.title).collect::<Vec<_>>());
        assert_eq!(secs[0].title, "One");
        assert!(secs[0].body.contains("## not a header"));
        assert_eq!(secs[1].title, "Two");
    }

    #[test]
    fn topics_matching_tags_filters_by_intersection() {
        let kn = tempfile::tempdir().unwrap();
        let conv = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&conv).unwrap();
        std::fs::write(
            conv.join("_index.json"),
            r#"{"topics":[
                {"id":"style","description":"kotlin style","tags":["kt","style"]},
                {"id":"css","description":"css rules","tags":["css"]}
            ]}"#,
        )
        .unwrap();
        let mut keys = std::collections::BTreeSet::new();
        keys.insert("kt".to_string());
        let hits = topics_matching_tags(kn.path(), "p", &keys);
        assert_eq!(hits, vec![("style".to_string(), "kotlin style".to_string())]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test knowledge`
Expected: `sections_ignores_headers_inside_code_fences` FAILS (3 sections found); `topics_matching_tags` doesn't compile.

- [ ] **Step 3: Implement**

(a) Replace `sections` (lines 336-355) with:

```rust
/// Split a markdown body into `## ` sections (anchors stripped from titles).
/// Lines inside ``` fences are body text, never headers.
fn sections(body: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut cur: Option<Section> = None;
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            if let Some(rest) = line.strip_prefix("## ") {
                if let Some(s) = cur.take() {
                    out.push(s);
                }
                let title = rest.split("{#").next().unwrap_or(rest).trim().to_string();
                cur = Some(Section { title, body: String::new() });
                continue;
            }
        }
        if let Some(s) = cur.as_mut() {
            s.body.push_str(line);
            s.body.push('\n');
        }
    }
    if let Some(s) = cur.take() {
        out.push(s);
    }
    out
}
```

(b) Add after `search`:

```rust
/// Convention topics whose tags intersect `keys` (lowercased file extensions
/// or family ids). Used by `brief`'s diff.scan to map changed files to rules.
pub fn topics_matching_tags(
    kn: &Path,
    profile: &str,
    keys: &std::collections::BTreeSet<String>,
) -> Vec<(String, String)> {
    let Ok(idx) = read_conv_index(kn, profile) else {
        return Vec::new();
    };
    idx.topics
        .iter()
        .filter(|t| t.tags.iter().any(|tag| keys.contains(&tag.to_lowercase())))
        .map(|t| (t.id.clone(), t.description.clone()))
        .collect()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/knowledge.rs
git commit -m "fix: fence-aware section splitting; add tag-based topic lookup for diff.scan"
```

---

### Task 6: Exec config types (CmdSpec / VerbSpec / ProjectConfig.exec)

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing tests** — add inside `src/config.rs` `mod tests`:

```rust
    #[test]
    fn verbspec_parses_string_table_and_list_forms() {
        let yaml = r#"
build: "./gradlew assembleDebug"
test: { cmd: "./gradlew test", timeout_secs: 900 }
doctor: { cmd: ["android -V", "adb version"] }
"#;
        let m: std::collections::BTreeMap<String, VerbSpec> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(m["build"].commands(), vec!["./gradlew assembleDebug"]);
        assert_eq!(m["build"].timeout_secs(), 600);
        assert_eq!(m["test"].timeout_secs(), 900);
        assert_eq!(m["doctor"].commands(), vec!["android -V", "adb version"]);
    }

    #[test]
    fn project_config_round_trips_exec_map() {
        let mut pc = ProjectConfig::default();
        pc.exec.insert("build".into(), VerbSpec::Simple("make".into()));
        let yaml = serde_yaml::to_string(&pc).unwrap();
        let back: ProjectConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.exec["build"].commands(), vec!["make"]);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test config`
Expected: compile error — `VerbSpec` missing.

- [ ] **Step 3: Implement** — in `src/config.rs`, add below the `Provider` struct:

```rust
// ─────────────────────────────────────────────────────────────────────────
// Exec verbs — `exec:` maps in profile.yaml and per-project config
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum CmdSpec {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum VerbSpec {
    Simple(String),
    Full {
        cmd: CmdSpec,
        #[serde(default = "default_exec_timeout")]
        timeout_secs: u64,
    },
}

pub fn default_exec_timeout() -> u64 {
    600
}

impl VerbSpec {
    /// The shell commands to run, in order (stop on first failure).
    pub fn commands(&self) -> Vec<String> {
        match self {
            VerbSpec::Simple(s) => vec![s.clone()],
            VerbSpec::Full { cmd: CmdSpec::One(s), .. } => vec![s.clone()],
            VerbSpec::Full { cmd: CmdSpec::Many(v), .. } => v.clone(),
        }
    }

    pub fn timeout_secs(&self) -> u64 {
        match self {
            VerbSpec::Simple(_) => default_exec_timeout(),
            VerbSpec::Full { timeout_secs, .. } => *timeout_secs,
        }
    }
}
```

and add to `ProjectConfig` (after `integrations`):

```rust
    /// Per-repo exec verbs; override/extend the profile's `exec:` map.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exec: BTreeMap<String, VerbSpec>,
```

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat: exec verb config types (CmdSpec/VerbSpec) on profile and project config"
```

---

### Task 7: `src/exec.rs` + `palugada exec` command

**Files:**
- Create: `src/exec.rs`
- Modify: `src/main.rs` (mod, Commands::Exec, cmd_exec, resolve_profile_best_effort)

- [ ] **Step 1: Create `src/exec.rs` with unit tests included** (the complete file):

```rust
//! `palugada exec <verb>` — the execution toolbelt.
//!
//! Verbs are DATA: `exec:` maps in the bound profile's `profile.yaml` and in
//! the repo's `.palugada/config.yaml` (project wins per-verb). The same verbs
//! (build/test/run/...) thus mean the right thing in every repo — android-cli
//! and gradle on Android, npm on web — and any AI CLI can branch on the exit
//! code or the `--json` outcome.

use crate::config::{ProjectConfig, VerbSpec};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Deserialize, Default)]
struct ProfileExec {
    #[serde(default)]
    exec: BTreeMap<String, VerbSpec>,
}

/// Merge exec verbs: profile first, project `.palugada/config.yaml` overrides
/// per-verb. `kn`/`profile` may be absent — project-only verbs still work.
pub fn merged_verbs(
    kn: Option<&Path>,
    profile: &str,
    repo: &Path,
) -> Result<BTreeMap<String, (VerbSpec, &'static str)>, String> {
    let mut out: BTreeMap<String, (VerbSpec, &'static str)> = BTreeMap::new();
    if let Some(kn) = kn {
        if !profile.is_empty() {
            let pf_path = kn.join("profiles").join(profile).join("profile.yaml");
            if let Ok(raw) = fs::read_to_string(&pf_path) {
                let pf: ProfileExec = serde_yaml::from_str(&raw)
                    .map_err(|e| format!("parse {}: {e}", pf_path.display()))?;
                for (k, v) in pf.exec {
                    out.insert(k, (v, "profile"));
                }
            }
        }
    }
    let repo_str = repo.to_string_lossy();
    if let Ok(pc) = ProjectConfig::load_from(&repo_str) {
        for (k, v) in pc.exec {
            out.insert(k, (v, "project"));
        }
    }
    Ok(out)
}

/// Parse `k=v` CLI args into a substitution map. Keys are [a-z0-9_-].
pub fn parse_kv_args(args: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut map = BTreeMap::new();
    for a in args {
        let (k, v) = a
            .split_once('=')
            .ok_or_else(|| format!("expected key=value, got '{a}'"))?;
        let ok = !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !ok {
            return Err(format!("invalid placeholder key '{k}' — use [a-z0-9_-]"));
        }
        map.insert(k.to_string(), v.to_string());
    }
    Ok(map)
}

/// Substitute `{key}` placeholders. Every placeholder must have a value;
/// otherwise error listing exactly what to pass.
pub fn substitute(template: &str, args: &BTreeMap<String, String>) -> Result<String, String> {
    let re = regex::Regex::new(r"\{([a-z0-9_-]+)\}").unwrap();
    let mut missing: Vec<String> = Vec::new();
    let out = re
        .replace_all(template, |caps: &regex::Captures| {
            let key = &caps[1];
            match args.get(key) {
                Some(v) => v.clone(),
                None => {
                    missing.push(key.to_string());
                    String::new()
                }
            }
        })
        .to_string();
    if !missing.is_empty() {
        missing.dedup();
        return Err(format!(
            "command `{template}` needs value(s) for: {} — pass them as `palugada exec <verb> {}`",
            missing.join(", "),
            missing
                .iter()
                .map(|m| format!("{m}=<value>"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    Ok(out)
}

#[derive(Serialize)]
pub struct ExecOutcome {
    pub verb: String,
    pub command: String,
    pub exit_code: i32,
    pub duration_ms: u128,
    pub tail: String,
}

pub struct ExecRequest<'a> {
    pub verb: &'a str,
    pub args: &'a BTreeMap<String, String>,
    /// JSON mode captures output (for `tail`); text mode streams it live.
    pub json: bool,
}

/// Run a verb's command(s) sequentially, stopping at the first failure.
/// Returns the outcome; the CALLER decides the process exit code.
pub fn run_verb(
    verbs: &BTreeMap<String, (VerbSpec, &'static str)>,
    repo: &Path,
    req: &ExecRequest,
) -> Result<ExecOutcome, String> {
    let (spec, _src) = verbs.get(req.verb).ok_or_else(|| {
        let have: Vec<&str> = verbs.keys().map(String::as_str).collect();
        format!(
            "no exec verb '{}' — available: {} (define it under `exec:` in .palugada/config.yaml or the profile)",
            req.verb,
            if have.is_empty() { "(none)".to_string() } else { have.join(", ") }
        )
    })?;
    let timeout = Duration::from_secs(spec.timeout_secs());
    let mut all_out = String::new();
    let mut ran: Vec<String> = Vec::new();
    let started = Instant::now();
    let mut exit_code = 0i32;
    for raw in spec.commands() {
        let cmd_str = substitute(&raw, req.args)?;
        if !req.json {
            eprintln!("$ {cmd_str}");
        }
        ran.push(cmd_str.clone());
        let code = run_one(&cmd_str, repo, timeout, req.json, &mut all_out)?;
        if code != 0 {
            exit_code = code;
            break;
        }
    }
    Ok(ExecOutcome {
        verb: req.verb.to_string(),
        command: ran.join(" && "),
        exit_code,
        duration_ms: started.elapsed().as_millis(),
        tail: tail_lines(&all_out, 40),
    })
}

/// Run one shell command with output captured into `out_buf` (used by doctor).
pub fn run_one_captured(
    cmd_str: &str,
    repo: &Path,
    timeout: Duration,
    out_buf: &mut String,
) -> Result<i32, String> {
    run_one(cmd_str, repo, timeout, true, out_buf)
}

fn run_one(
    cmd_str: &str,
    repo: &Path,
    timeout: Duration,
    capture: bool,
    out_buf: &mut String,
) -> Result<i32, String> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(cmd_str).current_dir(repo);
    if capture {
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn `{cmd_str}`: {e}"))?;
    let mut readers = Vec::new();
    if capture {
        if let Some(mut p) = child.stdout.take() {
            readers.push(std::thread::spawn(move || {
                let mut b = String::new();
                let _ = p.read_to_string(&mut b);
                b
            }));
        }
        if let Some(mut p) = child.stderr.take() {
            readers.push(std::thread::spawn(move || {
                let mut b = String::new();
                let _ = p.read_to_string(&mut b);
                b
            }));
        }
    }
    let start = Instant::now();
    let status = loop {
        match child.try_wait().map_err(|e| format!("wait `{cmd_str}`: {e}"))? {
            Some(s) => break Some(s),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    };
    for r in readers {
        if let Ok(s) = r.join() {
            out_buf.push_str(&s);
        }
    }
    match status {
        Some(s) => Ok(s.code().unwrap_or(1)),
        None => {
            out_buf.push_str(&format!("\n(timed out after {}s — killed)\n", timeout.as_secs()));
            Ok(124)
        }
    }
}

/// Last `n` lines of `s`.
pub fn tail_lines(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let start = lines.len().saturating_sub(n);
    lines[start..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verbs(yaml: &str) -> BTreeMap<String, (VerbSpec, &'static str)> {
        let m: BTreeMap<String, VerbSpec> = serde_yaml::from_str(yaml).unwrap();
        m.into_iter().map(|(k, v)| (k, (v, "project"))).collect()
    }

    #[test]
    fn substitute_fills_and_reports_missing() {
        let mut args = BTreeMap::new();
        args.insert("apk".to_string(), "out/app.apk".to_string());
        assert_eq!(
            substitute("android run --apks={apk}", &args).unwrap(),
            "android run --apks=out/app.apk"
        );
        let err = substitute("run {apk} {device}", &BTreeMap::new()).unwrap_err();
        assert!(err.contains("apk") && err.contains("device"), "{err}");
    }

    #[test]
    fn parse_kv_args_validates_keys() {
        let ok = parse_kv_args(&["apk=a.apk".into(), "out-file=x.png".into()]).unwrap();
        assert_eq!(ok["apk"], "a.apk");
        assert!(parse_kv_args(&["noequals".into()]).is_err());
        assert!(parse_kv_args(&["BAD=1".into()]).is_err());
    }

    #[test]
    fn run_verb_captures_tail_and_exit_codes() {
        let repo = tempfile::tempdir().unwrap();
        let v = verbs("ok: \"echo hello\"\nfail: \"echo boom; exit 3\"\nseq: { cmd: [\"echo one\", \"exit 2\", \"echo never\"] }\n");
        let args = BTreeMap::new();
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "ok", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 0);
        assert!(r.tail.contains("hello"));
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "fail", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 3);
        assert!(r.tail.contains("boom"));
        // list form stops at the first failure
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "seq", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 2);
        assert!(r.tail.contains("one") && !r.tail.contains("never"));
        assert_eq!(r.command, "echo one && exit 2");
        // unknown verb lists what exists
        let err = run_verb(&v, repo.path(), &ExecRequest { verb: "nope", args: &args, json: true }).unwrap_err();
        assert!(err.contains("fail, ok, seq"), "{err}");
    }

    #[test]
    fn run_verb_times_out_with_124() {
        let repo = tempfile::tempdir().unwrap();
        let v = verbs("sleepy: { cmd: \"sleep 5\", timeout_secs: 1 }\n");
        let args = BTreeMap::new();
        let r = run_verb(&v, repo.path(), &ExecRequest { verb: "sleepy", args: &args, json: true }).unwrap();
        assert_eq!(r.exit_code, 124);
        assert!(r.tail.contains("timed out"));
    }

    #[test]
    fn tail_lines_keeps_last_n() {
        let s = (1..=50).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let t = tail_lines(&s, 40);
        assert!(t.starts_with("11") && t.ends_with("50"));
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test exec`
Expected: compile error — `mod exec` not declared yet. Add `mod exec;` to `src/main.rs` (after `mod config;`), re-run: tests PASS (module is self-contained). The "failing" gate here is the missing main.rs wiring, verified next.

- [ ] **Step 3: Wire the CLI** — in `src/main.rs`:

(a) add to the `Commands` enum:

```rust
    /// Run a profile/project-defined exec verb: `exec <verb> [k=v ...]`.
    Exec {
        /// Verb to run (e.g. build, test, run). Omit with --list.
        verb: Option<String>,
        /// Placeholder values, e.g. `apk=app/build/outputs/apk/debug/app.apk`.
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
        /// List available verbs for this repo.
        #[arg(long)]
        list: bool,
        /// Emit a JSON outcome (captures output) instead of streaming.
        #[arg(long)]
        json: bool,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
```

(b) dispatch in `run()`:

```rust
        Commands::Exec { verb, args, list, json, profile } => {
            cmd_exec(verb, args, list, json, profile, project)
        }
```

(c) add the handler + best-effort profile resolution (after `cmd_brief`):

```rust
// ── exec: profile-declared execution toolbelt ──────────────────────────────

/// Profile resolution that never fails: exec must work even when the
/// knowledge dir is missing (project-only `exec:` maps).
fn resolve_profile_best_effort(
    global: &GlobalConfig,
    project: Option<&str>,
    profile_flag: Option<&str>,
    kn: Option<&std::path::Path>,
) -> String {
    match kn {
        Some(kn) => resolve_profile(global, project, profile_flag, kn).unwrap_or_default(),
        None => profile_flag.unwrap_or_default().to_string(),
    }
}

fn cmd_exec(
    verb: Option<String>,
    args: Vec<String>,
    list: bool,
    json: bool,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    let kn = knowledge::knowledge_dir(&global).ok();
    let prof = resolve_profile_best_effort(&global, project, profile.as_deref(), kn.as_deref());
    let verbs = exec::merged_verbs(kn.as_deref(), &prof, &repo)?;

    if list {
        if json {
            let m: std::collections::BTreeMap<&String, serde_json::Value> = verbs
                .iter()
                .map(|(k, (spec, src))| {
                    (k, serde_json::json!({ "source": src, "commands": spec.commands() }))
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&m).map_err(|e| e.to_string())?);
        } else if verbs.is_empty() {
            println!("(no exec verbs — add `exec:` to .palugada/config.yaml or bind a profile)");
        } else {
            for (v, (spec, src)) in &verbs {
                println!("{:<12} [{src}] {}", v, spec.commands().join(" && "));
            }
        }
        return Ok(());
    }

    let verb = verb.ok_or("specify a verb (e.g. `palugada exec build`) or use --list")?;
    let kv = exec::parse_kv_args(&args)?;
    let outcome = exec::run_verb(&verbs, &repo, &exec::ExecRequest { verb: &verb, args: &kv, json })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&outcome).map_err(|e| e.to_string())?);
    } else {
        println!("\n[{}] exit {} in {}ms", outcome.verb, outcome.exit_code, outcome.duration_ms);
    }
    if outcome.exit_code != 0 {
        // agents branch on this: palugada's exit code IS the child's
        std::process::exit(outcome.exit_code);
    }
    Ok(())
}
```

- [ ] **Step 4: Run tests + manual smoke**

Run: `cargo test` → PASS.
Run: `printf 'project: t\nexec:\n  hi: "echo hello {name}"\n' > /tmp/plg-smoke/.palugada/config.yaml` after `mkdir -p /tmp/plg-smoke/.palugada`; then `cargo run -q -- exec hi name=world --json` from `/tmp/plg-smoke`... simpler: `cd /tmp/plg-smoke && /Users/septiandwisaputro/Documents/project/tools/palugada/target/debug/palugada exec hi name=world --json` (after `cargo build`).
Expected: JSON with `"exit_code": 0` and `"tail": "hello world"`; `palugada exec hi` (no arg) errors mentioning `name=<value>`.

- [ ] **Step 5: Commit**

```bash
git add src/exec.rs src/main.rs
git commit -m "feat: palugada exec — profile/project-declared verbs with placeholders, timeout, JSON outcome, exit-code propagation"
```

---

### Task 8: `palugada doctor`

**Files:**
- Modify: `src/main.rs` (Commands::Doctor, cmd_doctor)

- [ ] **Step 1: Add the command** — `Commands` enum:

```rust
    /// Check tool + connector readiness for the current repo.
    Doctor {
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
```

dispatch:

```rust
        Commands::Doctor { json } => cmd_doctor(json, project, cli.insecure),
```

- [ ] **Step 2: Implement `cmd_doctor`** (after `cmd_exec`):

```rust
fn cmd_doctor(json: bool, project: Option<&str>, insecure: bool) -> Result<(), String> {
    #[derive(serde::Serialize)]
    struct Check {
        name: String,
        kind: String, // "tool" | "connector"
        ok: bool,
        detail: String,
    }

    let global = GlobalConfig::load_or_default()?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    let kn = knowledge::knowledge_dir(&global).ok();
    let prof = resolve_profile_best_effort(&global, project, None, kn.as_deref());
    let mut checks: Vec<Check> = Vec::new();

    // 1. tool checks: each command of the merged `doctor` verb
    let verbs = exec::merged_verbs(kn.as_deref(), &prof, &repo).unwrap_or_default();
    match verbs.get("doctor") {
        Some((spec, _)) => {
            for cmd_str in spec.commands() {
                let mut buf = String::new();
                let code = exec::run_one_captured(&cmd_str, &repo, std::time::Duration::from_secs(60), &mut buf);
                let first = buf.lines().find(|l| !l.trim().is_empty()).unwrap_or("").to_string();
                checks.push(Check {
                    name: cmd_str.clone(),
                    kind: "tool".into(),
                    ok: matches!(code, Ok(0)),
                    detail: first,
                });
            }
        }
        None => checks.push(Check {
            name: "doctor verb".into(),
            kind: "tool".into(),
            ok: true,
            detail: "(no `doctor` verb defined — tool checks skipped)".into(),
        }),
    }

    // 2. connector checks (only what's configured; skipped without a project)
    let secrets = Secrets::load_or_default().unwrap_or_default();
    match resolve_project(&global, &secrets, project) {
        Ok((_n, pc, auth)) => {
            let mut conns: Vec<(&str, Result<String, String>)> = Vec::new();
            if pc.integrations.issue_tracker.is_some() {
                conns.push(("issue", clients::issue_tracker(&pc, &auth, insecure).and_then(|c| c.verify())));
            }
            if pc.integrations.wiki.is_some() {
                conns.push(("wiki", clients::doc_source(&pc, &auth, insecure).and_then(|c| c.verify())));
            }
            if pc.integrations.git_host.is_some() {
                conns.push(("git", clients::git_host(&pc, &auth, insecure).and_then(|c| c.verify())));
            }
            if pc.integrations.design.is_some() {
                conns.push(("design", clients::design_source(&pc, &auth, insecure).and_then(|c| c.verify())));
            }
            if pc.integrations.ci.is_some() {
                conns.push(("ci", clients::ci_provider(&pc, &auth, insecure).and_then(|c| c.verify())));
            }
            for (tag, r) in conns {
                checks.push(Check {
                    name: tag.into(),
                    kind: "connector".into(),
                    ok: r.is_ok(),
                    detail: r.unwrap_or_else(|e| e),
                });
            }
        }
        Err(_) => checks.push(Check {
            name: "project".into(),
            kind: "connector".into(),
            ok: true,
            detail: "(no project configured — connector checks skipped)".into(),
        }),
    }

    let failed = checks.iter().filter(|c| !c.ok).count();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "ok": failed == 0, "checks": checks }))
                .map_err(|e| e.to_string())?
        );
    } else {
        println!(
            "palugada doctor — repo {} (profile: {})",
            repo.display(),
            if prof.is_empty() { "—" } else { prof.as_str() }
        );
        for c in &checks {
            println!("  [{}] {:<9} {} — {}", if c.ok { "PASS" } else { "FAIL" }, c.kind, c.name, c.detail);
        }
    }
    if failed > 0 {
        return Err(format!("{failed} check(s) failed"));
    }
    Ok(())
}
```

- [ ] **Step 3: Verify**

Run: `cargo build` → clean. Run: `cargo run -q -- doctor` from the palugada repo.
Expected: a report; `(no doctor verb defined — tool checks skipped)` + connector note or verifies, exit 0 when nothing fails.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: palugada doctor — tool checks via the doctor verb + connector verify rollup"
```

---

### Task 9: Rewrite `src/brief.rs` (budget-correct, issue.context / diff.scan / exec.hints, --diff)

**Files:**
- Rewrite: `src/brief.rs`
- Modify: `src/main.rs` (Brief command + cmd_brief)

- [ ] **Step 1: Replace `src/brief.rs` with the complete new file** (tests included):

```rust
//! `palugada brief <flow> [target]` — assemble one budgeted context pack.
//!
//! Reads the flow's step list from the bound profile's `profile.yaml`. Steps
//! degrade to in-pack notes on failure — a pack is ALWAYS produced. Budget
//! accounting covers exactly what is printed; in JSON mode too.

use crate::config::{AuthProfile, ProjectConfig};
use crate::{exec, indexer, knowledge};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Step kinds `brief` can execute — `profile validate` checks against this.
pub const KNOWN_STEP_KINDS: &[&str] = &[
    "convention",
    "recipe",
    "symbol.find",
    "code.recent",
    "issue.context",
    "diff.scan",
    "exec.hints",
];

#[derive(Deserialize, Default)]
struct ProfileFlows {
    #[serde(default)]
    flows: BTreeMap<String, Vec<String>>,
}

pub struct BriefOptions {
    pub flow: String,
    pub target: String,
    pub budget: usize,
    pub json: bool,
    /// Git ref for diff.scan (defaults to HEAD).
    pub diff: Option<String>,
}

/// Connector context: present when a project + auth profile resolved.
pub struct ConnectorCtx {
    pub pc: Option<ProjectConfig>,
    pub auth: Option<AuthProfile>,
    pub insecure: bool,
}

struct Pack {
    step: String,
    title: String,
    content: String,
}

#[derive(Serialize)]
struct PackOut {
    step: String,
    title: String,
    content: String,
    omitted: bool,
}

pub fn run(
    kn: &Path,
    repo: &Path,
    profile: &str,
    opts: &BriefOptions,
    ctx: &ConnectorCtx,
) -> Result<(), String> {
    let pf_path = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = fs::read_to_string(&pf_path).map_err(|e| format!("read {}: {e}", pf_path.display()))?;
    let pf: ProfileFlows =
        serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", pf_path.display()))?;

    let steps = pf.flows.get(&opts.flow).ok_or_else(|| {
        let have: Vec<&str> = pf.flows.keys().map(String::as_str).collect();
        format!(
            "flow '{}' not defined in profile '{}' (available: {})",
            opts.flow,
            profile,
            if have.is_empty() { "none".to_string() } else { have.join(", ") }
        )
    })?;

    let mut packs: Vec<Pack> = Vec::new();
    for step in steps {
        let (kind, arg) = parse_step(step);
        let (title, content) = match kind.as_str() {
            "convention" => (
                format!("convention: {arg}"),
                knowledge::convention_outline(kn, profile, &arg).unwrap_or_else(|e| format!("({e})")),
            ),
            "recipe" => (
                format!("recipe: {arg}"),
                knowledge::recipe_body(kn, profile, &arg).unwrap_or_else(|e| format!("({e})")),
            ),
            "symbol.find" => (
                format!("symbols matching '{}'", opts.target),
                indexer::symbol_report(repo, &opts.target).unwrap_or_else(|e| format!("({e})")),
            ),
            "code.recent" => (
                format!("recent commits for '{}'", opts.target),
                git_recent(repo, &opts.target),
            ),
            "issue.context" => (
                format!("issue context: {}", if opts.target.is_empty() { "(no target)" } else { &opts.target }),
                issue_context(ctx, &opts.target),
            ),
            "diff.scan" => {
                let r = opts.diff.clone().unwrap_or_else(|| "HEAD".to_string());
                (format!("diff scan vs {r}"), diff_scan(kn, profile, repo, &r))
            }
            "exec.hints" => (
                "how to build & test here".to_string(),
                exec_hints(kn, profile, repo),
            ),
            other => (
                other.to_string(),
                format!("(unknown step kind '{step}' — fix the profile's flows; run `palugada profile validate {profile}`)"),
            ),
        };
        packs.push(Pack { step: step.clone(), title, content });
    }

    let (selected, used) = select_packs(packs, opts.budget);

    if opts.json {
        let data = serde_json::to_string_pretty(&serde_json::json!({
            "flow": opts.flow,
            "target": opts.target,
            "budget": opts.budget,
            "used_tokens": used,
            "packs": selected,
        }))
        .map_err(|e| e.to_string())?;
        println!("{data}");
        return Ok(());
    }

    let target = if opts.target.is_empty() { "(no target)" } else { opts.target.as_str() };
    println!("# brief {}: {}", opts.flow, target);
    println!("profile: {profile}   budget: ~{} tokens\n", opts.budget);
    for p in &selected {
        if p.omitted {
            println!("## {}\n(omitted — over budget; run the step directly)\n", p.title);
        } else {
            println!("## {}\n{}\n", p.title, p.content);
        }
    }
    println!("(~{used} tokens)");
    Ok(())
}

/// ~4 chars/token on what actually gets printed for this pack.
fn est_tokens(s: &str) -> usize {
    s.chars().count() / 4 + 2
}

/// Budget selection: packs render in flow order; the FIRST pack truncates to
/// fit rather than being skipped; later overflowing packs become omission
/// notes whose own cost is counted. Returns (packs, used_tokens).
fn select_packs(packs: Vec<Pack>, budget: usize) -> (Vec<PackOut>, usize) {
    let mut out: Vec<PackOut> = Vec::new();
    let mut used: usize = 0;
    for p in packs {
        let body = p.content.trim().to_string();
        let block = format!("## {}\n{}\n\n", p.title, body);
        let cost = est_tokens(&block);
        if used + cost > budget {
            if used == 0 {
                let max_chars = budget.saturating_mul(4).saturating_sub(p.title.chars().count() + 40);
                let cut: String = body.chars().take(max_chars).collect();
                let content = format!("{cut}\n… (truncated to fit budget)");
                used += est_tokens(&format!("## {}\n{}\n\n", p.title, content));
                out.push(PackOut { step: p.step, title: p.title, content, omitted: false });
            } else {
                let note = format!("## {}\n(omitted — over budget)\n\n", p.title);
                used += est_tokens(&note);
                out.push(PackOut { step: p.step, title: p.title, content: String::new(), omitted: true });
            }
            continue;
        }
        used += cost;
        out.push(PackOut { step: p.step, title: p.title, content: body, omitted: false });
    }
    (out, used)
}

/// "convention(errorhandling)" → ("convention", "errorhandling");
/// "symbol.find" → ("symbol.find", "").
fn parse_step(step: &str) -> (String, String) {
    if let Some(open) = step.find('(') {
        if step.ends_with(')') {
            return (step[..open].to_string(), step[open + 1..step.len() - 1].to_string());
        }
    }
    (step.to_string(), String::new())
}

fn git_recent(repo: &Path, target: &str) -> String {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(["log", "--oneline", "-n", "8"]);
    if !target.is_empty() {
        cmd.arg("--").arg(target);
    }
    match cmd.output() {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                "(no recent commits)".to_string()
            } else {
                s
            }
        }
        _ => "(git log unavailable)".to_string(),
    }
}

/// Fetch the ticket when the target looks like one (PROJ-123). Degrades to a
/// note — never fails the pack.
fn issue_context(ctx: &ConnectorCtx, target: &str) -> String {
    let looks_like_ticket = regex::Regex::new(r"^[A-Z][A-Z0-9]+-\d+$").unwrap().is_match(target);
    if !looks_like_ticket {
        return format!("(target '{target}' is not a ticket key — skipped)");
    }
    let (Some(pc), Some(auth)) = (&ctx.pc, &ctx.auth) else {
        return "(no project/auth configured — run `palugada config verify`)".to_string();
    };
    match crate::clients::issue_tracker(pc, auth, ctx.insecure).and_then(|t| t.get_issue(target)) {
        Ok(i) => {
            let desc: String = i.description.chars().take(800).collect();
            format!(
                "{} — {}\nStatus: {}  Type: {}  Assignee: {}\n{}",
                i.key, i.summary, i.status, i.issue_type, i.assignee, desc
            )
        }
        Err(e) => format!("(issue fetch failed: {e})"),
    }
}

/// Changed files vs a ref + the conventions relevant to their file kinds.
fn diff_scan(kn: &Path, profile: &str, repo: &Path, gitref: &str) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["diff", "--name-only", gitref])
        .output();
    let files: Vec<String> = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect(),
        _ => return format!("(git diff --name-only {gitref} failed — not a git repo or bad ref)"),
    };
    if files.is_empty() {
        return format!("(no changes vs {gitref})");
    }
    let mut s = format!("changed files vs {gitref}:\n");
    for f in files.iter().take(40) {
        s.push_str(&format!("  {f}\n"));
    }
    if files.len() > 40 {
        s.push_str(&format!("  … and {} more\n", files.len() - 40));
    }
    let exts: std::collections::BTreeSet<String> = files
        .iter()
        .filter_map(|f| Path::new(f).extension().map(|e| e.to_string_lossy().to_lowercase()))
        .collect();
    let matched = knowledge::topics_matching_tags(kn, profile, &exts);
    if !matched.is_empty() {
        s.push_str("relevant conventions:\n");
        for (id, desc) in matched {
            s.push_str(&format!("  {id} — {desc} (palugada q {id})\n"));
        }
    }
    s
}

/// The repo's runnable verbs — tells the agent exactly how to verify work.
fn exec_hints(kn: &Path, profile: &str, repo: &Path) -> String {
    match exec::merged_verbs(Some(kn), profile, repo) {
        Ok(v) if !v.is_empty() => {
            let mut s = String::from("run these via `palugada exec <verb> [--json]`:\n");
            for (verb, (spec, _)) in &v {
                s.push_str(&format!("  {:<12} {}\n", verb, spec.commands().join(" && ")));
            }
            s
        }
        _ => "(no exec verbs defined — add `exec:` to .palugada/config.yaml)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(step: &str, content: &str) -> Pack {
        Pack { step: step.into(), title: step.into(), content: content.into() }
    }

    #[test]
    fn select_packs_truncates_first_and_omits_rest() {
        let big = "x".repeat(4000); // ~1000 tokens
        let (out, used) = select_packs(vec![pack("a", &big), pack("b", &big)], 100);
        assert!(!out[0].omitted, "first pack must render (truncated)");
        assert!(out[0].content.contains("truncated to fit budget"));
        assert!(out[1].omitted, "second pack must be an omission note");
        assert!(used <= 130, "used {used} should be near the 100 budget");
    }

    #[test]
    fn select_packs_fits_small_packs_without_truncation() {
        let (out, used) = select_packs(vec![pack("a", "hello"), pack("b", "world")], 500);
        assert!(out.iter().all(|p| !p.omitted));
        assert!(out[0].content == "hello" && out[1].content == "world");
        assert!(used > 0 && used < 50);
    }

    #[test]
    fn parse_step_splits_kind_and_arg() {
        assert_eq!(parse_step("convention(testing)"), ("convention".into(), "testing".into()));
        assert_eq!(parse_step("symbol.find"), ("symbol.find".into(), String::new()));
    }

    #[test]
    fn issue_context_skips_non_tickets_and_unconfigured() {
        let ctx = ConnectorCtx { pc: None, auth: None, insecure: false };
        assert!(issue_context(&ctx, "src/main.rs").contains("not a ticket key"));
        assert!(issue_context(&ctx, "PROJ-12").contains("no project/auth configured"));
    }
}
```

- [ ] **Step 2: Update `src/main.rs`:**

(a) `Brief` command gains the diff flag (after `budget`):

```rust
        /// Git ref for the review flow's diff.scan step (default: HEAD).
        #[arg(long)]
        diff: Option<String>,
```

(b) dispatch:

```rust
        Commands::Brief { flow, target, budget, diff, json, profile } => {
            cmd_brief(flow, target, budget, diff, json, profile, project, cli.insecure)
        }
```

(c) replace `cmd_brief`:

```rust
fn cmd_brief(
    flow: String,
    target: String,
    budget: usize,
    diff: Option<String>,
    json: bool,
    profile: Option<String>,
    project: Option<&str>,
    insecure: bool,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    // connectors are optional for brief — a missing/unparseable setup degrades
    let secrets = Secrets::load_or_default().unwrap_or_default();
    let ctx = match resolve_project(&global, &secrets, project) {
        Ok((_n, pc, auth)) => brief::ConnectorCtx { pc: Some(pc), auth: Some(auth), insecure },
        Err(_) => brief::ConnectorCtx { pc: None, auth: None, insecure },
    };
    brief::run(
        &kn,
        &repo,
        &prof,
        &brief::BriefOptions { flow, target, budget, json, diff },
        &ctx,
    )
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: PASS (4 new brief tests).

- [ ] **Step 4: Commit**

```bash
git add src/brief.rs src/main.rs
git commit -m "feat: brief v2 — correct budgets (text+json), issue.context, diff.scan via --diff, exec.hints"
```

---

### Task 10: android-mvvm profile content (six flows, exec verbs, 3 conventions, refactor recipe)

**Files:**
- Modify: `knowledge/profiles/android-mvvm/profile.yaml`, `knowledge/profiles/android-mvvm/conventions/_index.json`, `knowledge/profiles/android-mvvm/recipes/_index.json`
- Create: `knowledge/profiles/android-mvvm/conventions/errorhandling.md`, `.../testing.md`, `.../style.md`, `knowledge/profiles/android-mvvm/recipes/refactor.md`

No code in this task — content only. The check is `cargo test` (existing) + Task 13's `profile validate` later.

- [ ] **Step 1: Replace the `flows:` block and append `exec:` in `profile.yaml`** (keep everything above `flows:` unchanged):

```yaml
# Retrieval flows the agent skill files invoke via `palugada brief <flow> <target>`.
flows:
  plan:     [issue.context, convention(architecture), recipe(feature), symbol.find]
  bugfix:   [code.recent, symbol.find, convention(errorhandling), convention(testing)]
  feature:  [issue.context, recipe(feature), symbol.find, convention(architecture)]
  refactor: [symbol.find, convention(architecture), convention(style), recipe(refactor)]
  review:   [diff.scan, convention(style), convention(testing)]
  test:     [convention(testing), symbol.find, exec.hints]

# Execution toolbelt: `palugada exec <verb>`. android-cli (the `android`
# command from developer.android.com/tools/agents) drives device work;
# gradle drives build/test. Override per-repo in .palugada/config.yaml.
exec:
  build:      { cmd: "./gradlew assembleDebug", timeout_secs: 900 }
  test:       { cmd: "./gradlew testDebugUnitTest", timeout_secs: 900 }
  lint:       { cmd: "./gradlew lint", timeout_secs: 900 }
  run:        { cmd: "android run --apks={apk}" }
  ui-dump:    { cmd: "android layout --pretty" }
  screenshot: { cmd: "android screen capture --output={out}" }
  doctor:     { cmd: ["android -V", "adb version", "./gradlew -v"] }
```

(Note: the old `flows:` referenced `prd.context`/`module.info`/`by-file-kind`, which don't exist as step kinds — exactly the hollow-flow bug. The new lists reference ONLY `KNOWN_STEP_KINDS` and shipped content.)

- [ ] **Step 2: Create `conventions/errorhandling.md`:**

```markdown
---
id: errorhandling
title: Error handling
description: Result-typed repositories, sealed UiState.Error, and coroutine exception discipline.
layer: all
sections:
  - { id: repository-errors, title: "Repository errors as Result", tokens: 170, code: true }
  - { id: uistate-error,     title: "Surfacing errors as UiState", tokens: 150, code: true }
  - { id: coroutines,        title: "Coroutine exception discipline", tokens: 160, code: false }
related:
  - { topic: architecture, why: "UiState shape and unidirectional data flow" }
  - { topic: testing,      why: "Asserting error states in ViewModel tests" }
tags: [kt, error, exception, result, uistate]
---

# Error handling

> Errors are data, not control flow: repositories return `Result`, ViewModels
> map failures to a sealed `UiState.Error`, and nothing crosses a coroutine
> boundary as an unhandled exception.

## Repository errors as Result {#repository-errors}

Repositories catch transport/database exceptions at the boundary and return
`Result<T>` (or a domain error type) — they never let Retrofit/Room exceptions
escape to the ViewModel.

```kotlin
suspend fun fetchOrders(): Result<List<Order>> = runCatching {
    api.orders().map { it.toDomain() }
}
```

Map provider-specific exceptions (HttpException, IOException) to domain errors
here, once, so every caller sees the same vocabulary.

## Surfacing errors as UiState {#uistate-error}

The ViewModel folds the Result into the sealed UiState; the View renders the
error state like any other state — no Toasts from repositories, no silent
swallowing.

```kotlin
viewModelScope.launch {
    _uiState.value = UiState.Loading
    repository.fetchOrders()
        .onSuccess { _uiState.value = UiState.Success(it) }
        .onFailure { _uiState.value = UiState.Error(it.toUserMessage()) }
}
```

## Coroutine exception discipline {#coroutines}

- Launch UI work in `viewModelScope`; never `GlobalScope`.
- `runCatching` belongs at the repository boundary, not sprinkled in the UI.
- Don't catch `CancellationException` (or rethrow it if you must catch broadly)
  — swallowing it breaks structured cancellation.
- One-off failure effects (snackbars, navigation) go through a `SharedFlow`
  effect channel, not state.
```

- [ ] **Step 3: Create `conventions/testing.md`:**

```markdown
---
id: testing
title: Testing
description: What to test on this stack and how — JUnit + MockK + Turbine for ViewModels and repositories; run via palugada exec test.
layer: all
sections:
  - { id: what,        title: "What to test",        tokens: 130, code: false }
  - { id: viewmodels,  title: "ViewModel tests",     tokens: 200, code: true }
  - { id: running,     title: "Running tests",       tokens: 100, code: false }
related:
  - { topic: errorhandling, why: "Error states are the highest-value assertions" }
  - { topic: architecture,  why: "Layer boundaries decide what gets mocked" }
tags: [kt, test, junit, mockk, turbine]
---

# Testing

> Test ViewModels and repositories as plain JVM units: mock the layer below,
> assert the StateFlow/Result the layer above consumes.

## What to test {#what}

- **ViewModels**: state transitions (Loading → Success/Error) per user action.
- **Repositories**: mapping + error translation around a mocked data source.
- Skip UI/instrumented tests for logic that a JVM test can cover — they are
  slower and flakier; reserve them for actual rendering/navigation checks.

## ViewModel tests {#viewmodels}

JUnit + MockK for collaborators, Turbine for Flow assertions, and a main
dispatcher rule for `viewModelScope`:

```kotlin
@OptIn(ExperimentalCoroutinesApi::class)
class OrdersViewModelTest {
    @get:Rule val dispatcherRule = MainDispatcherRule()
    private val repository: OrdersRepository = mockk()

    @Test
    fun `load emits Success with orders`() = runTest {
        coEvery { repository.fetchOrders() } returns Result.success(listOf(order))
        val vm = OrdersViewModel(repository)
        vm.uiState.test {
            vm.load()
            assertEquals(UiState.Loading, awaitItem())
            assertTrue(awaitItem() is UiState.Success)
        }
    }
}
```

## Running tests {#running}

Run the suite through the uniform verb so the same instruction works on every
palugada project: `palugada exec test --json` (wired to
`./gradlew testDebugUnitTest` by this profile). Iterate until `exit_code` is 0;
the `tail` field carries the failing test names.
```

- [ ] **Step 4: Create `conventions/style.md`:**

```markdown
---
id: style
title: Code style
description: Kotlin style and naming for this stack — official Kotlin conventions, immutable-first, small focused classes.
layer: all
sections:
  - { id: kotlin, title: "Kotlin style",  tokens: 140, code: false }
  - { id: naming, title: "Naming",        tokens: 130, code: false }
related:
  - { topic: architecture, why: "Layer suffixes (ViewModel/Repository) come from the architecture" }
tags: [kt, style, naming, kotlin]
---

# Code style

> Follow the official Kotlin coding conventions; everything below is the
> stack-specific delta.

## Kotlin style {#kotlin}

- Immutable first: `val` over `var`, `List` over `MutableList` in signatures.
- Expression bodies for single-expression functions.
- No wildcard imports. ktlint/ktfmt formatting is non-negotiable — run
  `palugada exec lint` before review.
- Coroutines: suspend functions take no callbacks; return values or Flow.

## Naming {#naming}

- Screen-scoped classes carry their layer suffix: `OrdersViewModel`,
  `OrdersRepository`, `OrdersScreen`/`OrdersFragment`.
- UiState subclasses are nouns: `Loading`, `Success`, `Error` — not verbs.
- Test names use backtick sentences: `` `load emits Success with orders` ``.
- One class per file; the file is named after the class.
```

- [ ] **Step 5: Create `recipes/refactor.md`:**

```markdown
---
id: refactor
title: Refactor safely
description: Behavior-preserving refactor recipe — characterize, change in small steps, verify with exec test after each step.
references:
  conventions:
    - { topic: architecture, section: layers, why: "Target shape for moved responsibilities" }
    - { topic: style,        section: kotlin, why: "Formatting and immutability defaults" }
related_recipes: [feature]
tags: [refactor, safety, tests]
---

# Recipe: Refactor safely

## When to use this

You are restructuring existing code without changing behavior: extracting a
repository, splitting a god-ViewModel, moving logic out of a Fragment.

## Steps

1. **Characterize current behavior** — if the code you're moving lacks tests,
   write the missing ViewModel/repository tests FIRST (see `palugada q
   testing.2`); they are your safety net.
2. **Verify green baseline** — `palugada exec test --json`; do not start from
   red.
3. **One structural move at a time** — extract class / move function / rename;
   never combine a move with a logic change in one step.
4. **Re-run after every step** — `palugada exec build --json && palugada exec
   test --json`; a step that breaks the build gets reverted, not patched
   forward.
5. **Re-index when symbols moved** — `palugada index` so `symbol`/`brief`
   reflect the new layout.
6. **Scope the review** — `palugada brief review --diff <base-ref>` to collect
   only the conventions relevant to the changed files.
```

- [ ] **Step 6: Replace `conventions/_index.json`** (adds the three topics; architecture entry unchanged):

```json
{
  "schema_version": "1.0",
  "topics": [
    {
      "id": "architecture",
      "title": "Architecture",
      "file": "architecture.md",
      "description": "MVVM with a repository layer, Hilt DI, Coroutines + Flow, and a sealed UiState. UI-toolkit agnostic.",
      "tags": ["mvvm", "architecture", "hilt", "flow", "stateflow", "repository", "kt"],
      "sections": [
        { "id": "overview",  "title": "MVVM Overview",             "tokens": 170 },
        { "id": "layers",    "title": "Layers & Responsibilities", "tokens": 210 },
        { "id": "uistate",   "title": "Sealed UiState",            "tokens": 190 },
        { "id": "data-flow", "title": "Unidirectional Data Flow",  "tokens": 230 }
      ],
      "related": ["errorhandling", "testing", "style"]
    },
    {
      "id": "errorhandling",
      "title": "Error handling",
      "file": "errorhandling.md",
      "description": "Result-typed repositories, sealed UiState.Error, and coroutine exception discipline.",
      "tags": ["kt", "error", "exception", "result", "uistate"],
      "sections": [
        { "id": "repository-errors", "title": "Repository errors as Result",    "tokens": 170 },
        { "id": "uistate-error",     "title": "Surfacing errors as UiState",    "tokens": 150 },
        { "id": "coroutines",        "title": "Coroutine exception discipline", "tokens": 160 }
      ],
      "related": ["architecture", "testing"]
    },
    {
      "id": "testing",
      "title": "Testing",
      "file": "testing.md",
      "description": "JUnit + MockK + Turbine for ViewModels and repositories; run via palugada exec test.",
      "tags": ["kt", "test", "junit", "mockk", "turbine"],
      "sections": [
        { "id": "what",       "title": "What to test",    "tokens": 130 },
        { "id": "viewmodels", "title": "ViewModel tests", "tokens": 200 },
        { "id": "running",    "title": "Running tests",   "tokens": 100 }
      ],
      "related": ["errorhandling", "architecture"]
    },
    {
      "id": "style",
      "title": "Code style",
      "file": "style.md",
      "description": "Kotlin style and naming for this stack — official Kotlin conventions, immutable-first.",
      "tags": ["kt", "style", "naming", "kotlin"],
      "sections": [
        { "id": "kotlin", "title": "Kotlin style", "tokens": 140 },
        { "id": "naming", "title": "Naming",       "tokens": 130 }
      ],
      "related": ["architecture"]
    }
  ]
}
```

- [ ] **Step 7: Replace `recipes/_index.json`:**

```json
{
  "schema_version": "1.0",
  "recipes": [
    {
      "id": "feature",
      "title": "Scaffold a new feature",
      "description": "End-to-end recipe for a new screen/feature — data source, repository, sealed UiState, ViewModel, and UI.",
      "file": "feature.md",
      "convention_refs": [
        { "topic": "architecture", "section": "layers" },
        { "topic": "architecture", "section": "uistate" },
        { "topic": "architecture", "section": "data-flow" }
      ],
      "related_recipes": ["refactor"],
      "tags": ["feature", "scaffold", "mvvm"]
    },
    {
      "id": "refactor",
      "title": "Refactor safely",
      "description": "Behavior-preserving refactor — characterize, small steps, exec test after each step.",
      "file": "refactor.md",
      "convention_refs": [
        { "topic": "architecture", "section": "layers" },
        { "topic": "style", "section": "kotlin" }
      ],
      "related_recipes": ["feature"],
      "tags": ["refactor", "safety", "tests"]
    }
  ]
}
```

- [ ] **Step 8: Smoke-check and commit**

Run: `cargo run -q -- q errorhandling --profile android-mvvm` and `cargo run -q -- for refactor --profile android-mvvm` (PALUGADA_KNOWLEDGE may be needed: `PALUGADA_KNOWLEDGE=$PWD/knowledge`).
Expected: full convention body / recipe body, no errors. `cargo run -q -- s test --profile android-mvvm` lists the testing convention.

```bash
git add knowledge/profiles/android-mvvm
git commit -m "feat(knowledge): android-mvvm — six loop flows, android-cli/gradle exec verbs, errorhandling/testing/style conventions, refactor recipe"
```

---

### Task 11: web-react + generic profiles

**Files:**
- Create: `knowledge/profiles/web-react/{profile.yaml,extractors.yaml,conventions/_index.json,conventions/architecture.md,conventions/testing.md,recipes/_index.json,recipes/feature.md}`
- Create: `knowledge/profiles/generic/{profile.yaml,extractors.yaml,conventions/_index.json,recipes/_index.json}`

- [ ] **Step 1: `web-react/profile.yaml`:**

```yaml
schema_version: "1.0"
id: web-react
title: "Web · React + TypeScript"
description: >
  Function components + hooks, typed APIs, server-state via a query library.
  Framework-flexible (Vite/Next): exec verbs assume npm scripts — override
  per-repo in .palugada/config.yaml.
languages: [typescript]

fact_families:
  - { id: component, symbol: true }
  - { id: hook,      symbol: true }
  - { id: route,     symbol: false }

flows:
  plan:     [issue.context, convention(architecture), recipe(feature), symbol.find]
  bugfix:   [code.recent, symbol.find, convention(architecture), convention(testing)]
  feature:  [issue.context, recipe(feature), symbol.find, convention(architecture)]
  refactor: [symbol.find, convention(architecture), convention(testing)]
  review:   [diff.scan, convention(architecture), convention(testing)]
  test:     [convention(testing), symbol.find, exec.hints]

exec:
  build:  { cmd: "npm run build", timeout_secs: 900 }
  test:   { cmd: "npm test -- --run", timeout_secs: 900 }
  lint:   "npm run lint"
  run:    "npm run dev"
  doctor: { cmd: ["node -v", "npm -v"] }
```

- [ ] **Step 2: `web-react/extractors.yaml`:**

```yaml
# Declarative extraction rules for the web-react profile (regex MVP).
schema_version: "1.0"

ignore_dirs: [".git", "node_modules", "dist", "build", ".next", ".palugada", "coverage"]

families:
  - id: component
    ext: [tsx, jsx]
    regex: '(?:export\s+default\s+function|export\s+function|function)\s+(?P<name>[A-Z][A-Za-z0-9]*)'

  - id: hook
    ext: [ts, tsx]
    regex: '(?:export\s+)?(?:function|const)\s+(?P<name>use[A-Z][A-Za-z0-9]*)'

  - id: route
    ext: [ts, tsx]
    regex: 'path:\s*"(?P<name>/[^"]*)"'
```

- [ ] **Step 3: `web-react/conventions/architecture.md`:**

```markdown
---
id: architecture
title: Architecture
description: Function components + hooks; server state in a query library, client state local-first; typed API clients.
layer: all
sections:
  - { id: components, title: "Components & hooks",  tokens: 160, code: false }
  - { id: data,       title: "Data fetching",       tokens: 170, code: true }
  - { id: state,      title: "State placement",     tokens: 140, code: false }
related:
  - { topic: testing, why: "Component and hook test strategy" }
tags: [tsx, ts, react, hooks, architecture]
---

# Architecture

> Function components and hooks only; data fetching lives in hooks built on a
> server-state library; components stay presentational.

## Components & hooks {#components}

- Function components exclusively; no class components.
- A component renders props/state — fetching, caching, and business rules live
  in custom hooks (`useOrders`, not `fetch` inside the component body).
- Co-locate by feature (`src/features/<name>/`), not by technical kind.

## Data fetching {#data}

Server state goes through a query hook so caching/retries are uniform:

```tsx
export function useOrders() {
  return useQuery({ queryKey: ["orders"], queryFn: api.orders.list });
}

function OrdersScreen() {
  const { data, error, isLoading } = useOrders();
  if (isLoading) return <Spinner />;
  if (error) return <ErrorBanner error={error} />;
  return <OrderList orders={data} />;
}
```

## State placement {#state}

- Server data: the query library's cache — never copied into useState.
- UI-local state: `useState`/`useReducer` in the component that owns it.
- Cross-cutting client state: context or a small store, only when two+
  unrelated trees need it.
```

- [ ] **Step 4: `web-react/conventions/testing.md`:**

```markdown
---
id: testing
title: Testing
description: Vitest + Testing Library — test behavior through the DOM, mock the network boundary, run via palugada exec test.
layer: all
sections:
  - { id: what,    title: "What to test",   tokens: 120, code: false }
  - { id: how,     title: "Component tests", tokens: 180, code: true }
  - { id: running, title: "Running tests",  tokens: 90,  code: false }
related:
  - { topic: architecture, why: "Hooks own logic, so hooks get the deepest tests" }
tags: [ts, tsx, test, vitest, testing-library]
---

# Testing

> Test what the user sees: render the component, interact via Testing Library,
> assert on the DOM. Mock fetch/API modules, not internal hooks.

## What to test {#what}

- Custom hooks with logic (mapping, branching) — via a tiny harness component
  or `renderHook`.
- Components with conditional rendering (loading/error/success).
- Skip snapshot-only tests; they assert nothing about behavior.

## Component tests {#how}

```tsx
test("shows orders after load", async () => {
  vi.spyOn(api.orders, "list").mockResolvedValue([order]);
  render(<OrdersScreen />, { wrapper: QueryWrapper });
  expect(await screen.findByText(order.title)).toBeInTheDocument();
});
```

## Running tests {#running}

`palugada exec test --json` (wired to `npm test -- --run`). Iterate until
`exit_code` is 0; failures appear in the `tail` field.
```

- [ ] **Step 5: `web-react/conventions/_index.json`:**

```json
{
  "schema_version": "1.0",
  "topics": [
    {
      "id": "architecture",
      "title": "Architecture",
      "file": "architecture.md",
      "description": "Function components + hooks; server state in a query library; typed API clients.",
      "tags": ["tsx", "ts", "react", "hooks", "architecture"],
      "sections": [
        { "id": "components", "title": "Components & hooks", "tokens": 160 },
        { "id": "data",       "title": "Data fetching",      "tokens": 170 },
        { "id": "state",      "title": "State placement",    "tokens": 140 }
      ],
      "related": ["testing"]
    },
    {
      "id": "testing",
      "title": "Testing",
      "file": "testing.md",
      "description": "Vitest + Testing Library — behavior through the DOM; run via palugada exec test.",
      "tags": ["ts", "tsx", "test", "vitest", "testing-library"],
      "sections": [
        { "id": "what",    "title": "What to test",    "tokens": 120 },
        { "id": "how",     "title": "Component tests", "tokens": 180 },
        { "id": "running", "title": "Running tests",   "tokens": 90 }
      ],
      "related": ["architecture"]
    }
  ]
}
```

- [ ] **Step 6: `web-react/recipes/feature.md`:**

```markdown
---
id: feature
title: Scaffold a new feature
description: Vertical slice — typed API client, query hook, component, route — colocated under src/features/<name>/.
references:
  conventions:
    - { topic: architecture, section: components, why: "Component/hook split" }
    - { topic: architecture, section: data,       why: "Query hook pattern" }
related_recipes: []
tags: [feature, scaffold, react]
---

# Recipe: Scaffold a new feature

## Steps

1. **API client** — typed function(s) in `src/features/<name>/api.ts`.
2. **Query hook** — `use<Name>()` wrapping the API call with a stable
   `queryKey` (see `palugada q architecture.2`).
3. **Component(s)** — presentational; handle loading/error/success states.
4. **Route** — register the screen in the router config.
5. **Tests** — one hook test + one component test per state branch
   (`palugada q testing.2`), then `palugada exec test --json` until green.
6. **Re-index** — `palugada index` so `symbol` finds the new names.
```

- [ ] **Step 7: `web-react/recipes/_index.json`:**

```json
{
  "schema_version": "1.0",
  "recipes": [
    {
      "id": "feature",
      "title": "Scaffold a new feature",
      "description": "Vertical slice — typed API client, query hook, component, route.",
      "file": "feature.md",
      "convention_refs": [
        { "topic": "architecture", "section": "components" },
        { "topic": "architecture", "section": "data" }
      ],
      "related_recipes": [],
      "tags": ["feature", "scaffold", "react"]
    }
  ]
}
```

- [ ] **Step 8: `generic/profile.yaml`:**

```yaml
schema_version: "1.0"
id: generic
title: "Generic — any stack"
description: >
  Stack-agnostic fallback: git + index retrieval flows and project-defined
  exec verbs. Bind a real stack profile when one exists; define exec verbs
  in .palugada/config.yaml.
languages: []

fact_families:
  - { id: function, symbol: true }
  - { id: class,    symbol: true }

flows:
  plan:     [issue.context, code.recent, symbol.find]
  bugfix:   [code.recent, symbol.find, exec.hints]
  feature:  [issue.context, code.recent, symbol.find]
  refactor: [symbol.find, code.recent]
  review:   [diff.scan]
  test:     [symbol.find, exec.hints]

exec: {}
```

- [ ] **Step 9: `generic/extractors.yaml`:**

```yaml
# Generic cross-language symbol extraction (regex; intentionally coarse).
schema_version: "1.0"

ignore_dirs: [".git", "node_modules", "dist", "build", "target", ".palugada", ".gradle", ".idea", "coverage", "vendor", ".next"]

families:
  - id: function
    ext: [rs, go, py, kt, swift, ts, tsx, js, jsx, java, rb]
    regex: '(?:fn|func|def|fun|function)\s+(?P<name>[A-Za-z_][A-Za-z0-9_]*)'

  - id: class
    ext: [rs, go, py, kt, swift, ts, tsx, js, jsx, java, rb]
    regex: '(?:class|struct|interface|trait|enum)\s+(?P<name>[A-Z][A-Za-z0-9_]*)'
```

- [ ] **Step 10: `generic/conventions/_index.json` and `generic/recipes/_index.json`** (empty but valid so `q --list`/`s` degrade cleanly):

```json
{ "schema_version": "1.0", "topics": [] }
```

```json
{ "schema_version": "1.0", "recipes": [] }
```

- [ ] **Step 11: Smoke + commit**

Run (from repo root): `PALUGADA_KNOWLEDGE=$PWD/knowledge cargo run -q -- q --list --profile web-react` → two topics. `PALUGADA_KNOWLEDGE=$PWD/knowledge cargo run -q -- q --list --profile generic` → `(no conventions in profile 'generic')`.

```bash
git add knowledge/profiles/web-react knowledge/profiles/generic
git commit -m "feat(knowledge): ship web-react and generic profiles — flows, exec verbs, extractors, starter content"
```

---

### Task 12: Rewrite `src/scaffold.rs` (managed blocks, target table, six namespaced skills, `skills sync`)

**Files:**
- Rewrite: `src/scaffold.rs`
- Modify: `src/main.rs` (Commands::Skills + dispatch; Init default agents)

- [ ] **Step 1: Replace `src/scaffold.rs` with the complete new file** (tests included):

```rust
//! `palugada init` / `palugada skills sync` — offline project scaffolding.
//!
//! Root agent files (CLAUDE.md / AGENTS.md / GEMINI.md) are NEVER clobbered:
//! palugada owns only a marker-delimited block and upserts it idempotently.
//! Wholly palugada-owned files (.palugada/config.yaml, .claude/skills/
//! palugada-*/SKILL.md, .cursor/rules/palugada.mdc) are skipped when present
//! unless --force (init) or always refreshed (sync).

use crate::config::{expand_home, GlobalConfig, ProjectEntry};
use crate::knowledge;
use std::fs;
use std::path::Path;

pub struct InitOptions {
    pub repo: String,
    pub name: Option<String>,
    pub profile: Option<String>,
    pub auth: Option<String>,
    pub agents: Vec<String>,
    pub force: bool,
}

pub const MARK_BEGIN: &str = "<!-- palugada:begin -->";
pub const MARK_END: &str = "<!-- palugada:end -->";

/// (flow id, description, verb phrase, title, after-pack hint)
const FLOWS: &[(&str, &str, &str, &str, &str)] = &[
    (
        "plan", "Plan a ticket or goal before coding.", "plan work", "Plan",
        "Draft the implementation steps from the pack before touching code.",
    ),
    (
        "bugfix", "Fix a bug or crash.", "fix a bug", "Bugfix",
        "After editing, run `palugada exec build --json` and `palugada exec test --json`; on failure repeat with the failing file.",
    ),
    (
        "feature", "Build a new feature or screen.", "build a feature", "Feature",
        "After each edit cycle verify with `palugada exec build --json` then `palugada exec test --json`.",
    ),
    (
        "refactor", "Refactor or restructure code.", "refactor code", "Refactor",
        "Keep behavior identical: run `palugada exec test --json` after each step.",
    ),
    (
        "review", "Review a diff or pull/merge request.", "review a diff", "Review",
        "Use `palugada brief review --diff <ref>` to scope rules to the changed files.",
    ),
    (
        "test", "Write or extend tests.", "write tests", "Test",
        "Run `palugada exec test --json` and iterate until exit_code is 0.",
    ),
];

enum Wrap {
    Plain,
    CursorMdc,
}

struct AgentTarget {
    name: &'static str,
    root: &'static str,
    wrap: Wrap,
    /// Only Claude Code has a real skills directory.
    claude_skills: bool,
    /// Root file is shared with the user (managed block) vs wholly ours.
    managed: bool,
}

const TARGETS: &[AgentTarget] = &[
    AgentTarget { name: "claude", root: "CLAUDE.md", wrap: Wrap::Plain, claude_skills: true, managed: true },
    AgentTarget { name: "codex", root: "AGENTS.md", wrap: Wrap::Plain, claude_skills: false, managed: true },
    AgentTarget { name: "gemini", root: "GEMINI.md", wrap: Wrap::Plain, claude_skills: false, managed: true },
    AgentTarget { name: "cursor", root: ".cursor/rules/palugada.mdc", wrap: Wrap::CursorMdc, claude_skills: false, managed: false },
];

pub fn run(opts: InitOptions) -> Result<(), String> {
    let repo = fs::canonicalize(expand_home(&opts.repo))
        .map_err(|e| format!("repo path not found ({}): {e}", opts.repo))?;
    if !repo.is_dir() {
        return Err(format!("not a directory: {}", repo.display()));
    }
    let repo_str = repo.to_string_lossy().to_string();

    let name = opts.name.clone().unwrap_or_else(|| {
        repo.file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string())
    });
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global).ok();
    let profile = opts
        .profile
        .clone()
        .unwrap_or_else(|| detect_profile(&repo, kn.as_deref()));
    let auth = opts.auth.clone().unwrap_or_else(|| "default".to_string());
    let agents = if opts.agents.is_empty() {
        vec!["claude".to_string()]
    } else {
        opts.agents.clone()
    };

    let mut written: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // 1. per-project config skeleton (wholly ours; skip unless --force)
    let cfg = repo.join(".palugada").join("config.yaml");
    write_owned(&cfg, &config_skeleton(&name, &profile, &auth), opts.force, &mut written, &mut skipped)?;

    // 2. agent files
    emit_agent_files(&repo, &name, &profile, &agents, opts.force, &mut written, &mut skipped)?;

    // 3. register in the global project registry (collision-checked)
    let mut global = GlobalConfig::load_or_default()?;
    if let Some(existing) = global.projects.registered.get(&name) {
        if existing.repo_path != repo_str {
            eprintln!(
                "warning: project '{name}' was registered at {} — overwriting with {repo_str} (use --name to keep both)",
                existing.repo_path
            );
        }
    }
    let workspace = format!("{}/.palugada", repo_str.trim_end_matches('/'));
    global
        .projects
        .registered
        .insert(name.clone(), ProjectEntry { repo_path: repo_str.clone(), workspace });
    let became_active = global.projects.active.is_empty();
    if became_active {
        global.projects.active = name.clone();
    }
    global.save()?;

    // 4. summary
    println!(
        "palugada init — project '{name}' (profile: {profile}, auth: {auth}, agents: {})",
        agents.join(",")
    );
    for w in &written {
        println!("  wrote    {w}");
    }
    for s in &skipped {
        println!("  skipped  {s}  (exists — use --force to overwrite)");
    }
    println!(
        "  registered in {}{}",
        GlobalConfig::default_path().display(),
        if became_active { " (now active)" } else { "" }
    );
    println!("\nDone — 0 network calls. Next:");
    println!("  1. fill the integration base URLs in {}", cfg.display());
    println!("  2. add tokens to ~/.palugada/secrets.yaml under auth-profile '{auth}'");
    println!("  3. `palugada index` then `palugada doctor`");
    Ok(())
}

/// `palugada skills sync` — refresh agent files only. Targets: explicit
/// --agents, else whichever target files already exist, else claude.
pub fn sync(repo: String, agents_csv: String) -> Result<(), String> {
    let repo = fs::canonicalize(expand_home(&repo))
        .map_err(|e| format!("repo path not found ({repo}): {e}"))?;
    let pc = crate::config::ProjectConfig::load_from(&repo.to_string_lossy())
        .map_err(|e| format!("{e}\n(`skills sync` needs an initialized project)"))?;
    let name = if pc.project.is_empty() { "project".to_string() } else { pc.project.clone() };
    let profile = pc.profile.clone();

    let mut agents: Vec<String> = agents_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if agents.is_empty() {
        for t in TARGETS {
            if repo.join(t.root).exists() {
                agents.push(t.name.to_string());
            }
        }
    }
    if agents.is_empty() {
        agents.push("claude".to_string());
    }

    let mut written: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    // force=true: sync's whole purpose is refreshing palugada-owned content
    emit_agent_files(&repo, &name, &profile, &agents, true, &mut written, &mut skipped)?;
    println!("palugada skills sync — project '{name}' (profile: {profile}, agents: {})", agents.join(","));
    for w in &written {
        println!("  wrote    {w}");
    }
    Ok(())
}

fn emit_agent_files(
    repo: &Path,
    name: &str,
    profile: &str,
    agents: &[String],
    force_owned: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<(), String> {
    let guide = agent_guide(name, profile);
    for agent in agents {
        let target = TARGETS
            .iter()
            .find(|t| t.name == agent.as_str())
            .ok_or_else(|| {
                let known: Vec<&str> = TARGETS.iter().map(|t| t.name).collect();
                format!("unknown agent target: '{agent}' (supported: {})", known.join(", "))
            })?;
        let path = repo.join(target.root);
        let body = match target.wrap {
            Wrap::Plain => guide.clone(),
            Wrap::CursorMdc => cursor_wrap(&guide),
        };
        if target.managed {
            write_managed(&path, &body, written)?;
        } else {
            write_owned(&path, &body, force_owned, written, skipped)?;
        }
        if target.claude_skills {
            for &(flow, desc, action, title, hint) in FLOWS {
                let p = repo
                    .join(".claude")
                    .join("skills")
                    .join(format!("palugada-{flow}"))
                    .join("SKILL.md");
                let body = agent_skill(flow, desc, action, title, hint, profile);
                write_owned(&p, &body, force_owned, written, skipped)?;
            }
        }
    }
    Ok(())
}

fn detect_profile(repo: &Path, kn: Option<&Path>) -> String {
    let has = |f: &str| repo.join(f).exists();
    let candidate = if has("build.gradle")
        || has("build.gradle.kts")
        || has("settings.gradle")
        || has("settings.gradle.kts")
    {
        "android-mvvm"
    } else if has("package.json") {
        "web-react"
    } else {
        "generic"
    };
    if let Some(kn) = kn {
        if !kn.join("profiles").join(candidate).join("profile.yaml").exists() {
            eprintln!("warning: detected profile '{candidate}' is not bundled — falling back to 'generic'");
            return "generic".to_string();
        }
    }
    candidate.to_string()
}

/// Insert or replace the palugada-managed block. User content is preserved.
pub fn upsert_managed(existing: &str, block: &str) -> String {
    let managed = format!("{MARK_BEGIN}\n{}\n{MARK_END}", block.trim_end());
    if let (Some(b), Some(e)) = (existing.find(MARK_BEGIN), existing.find(MARK_END)) {
        if e >= b {
            let after = e + MARK_END.len();
            return format!("{}{}{}", &existing[..b], managed, &existing[after..]);
        }
    }
    if existing.trim().is_empty() {
        format!("{managed}\n")
    } else {
        format!("{}\n\n{managed}\n", existing.trim_end())
    }
}

fn write_managed(path: &Path, block: &str, written: &mut Vec<String>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let existing = fs::read_to_string(path).unwrap_or_default();
    let merged = upsert_managed(&existing, block);
    if merged != existing {
        fs::write(path, &merged).map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(format!("{} (managed block)", path.display()));
    }
    Ok(())
}

fn write_owned(
    path: &Path,
    content: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<(), String> {
    if path.exists() && !force {
        skipped.push(path.display().to_string());
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    fs::write(path, content).map_err(|e| format!("write {}: {e}", path.display()))?;
    written.push(path.display().to_string());
    Ok(())
}

// ── templates (placeholder-substituted to avoid brace escaping) ───────────

fn config_skeleton(name: &str, profile: &str, auth: &str) -> String {
    CONFIG_TEMPLATE
        .replace("__PROJECT__", name)
        .replace("__PROFILE__", profile)
        .replace("__AUTH__", auth)
}

fn agent_guide(name: &str, profile: &str) -> String {
    GUIDE_TEMPLATE
        .replace("__PROJECT__", name)
        .replace("__PROFILE__", profile)
}

fn agent_skill(flow: &str, desc: &str, action: &str, title: &str, hint: &str, profile: &str) -> String {
    SKILL_TEMPLATE
        .replace("__FLOW__", flow)
        .replace("__DESC__", desc)
        .replace("__ACTION__", action)
        .replace("__TITLE__", title)
        .replace("__HINT__", hint)
        .replace("__PROFILE__", profile)
}

fn cursor_wrap(body: &str) -> String {
    format!(
        "---\ndescription: palugada — project context + exec CLI guide\nalwaysApply: true\n---\n\n{body}"
    )
}

const CONFIG_TEMPLATE: &str = r#"# palugada per-project config — generated by `palugada init`.
# Tokens are NOT stored here; they live in ~/.palugada/secrets.yaml under the
# auth-profile named below. Fill in the integration base URLs, then run
# `palugada config verify`.

project: __PROJECT__
profile: __PROFILE__
auth_profile: __AUTH__

integrations:
  issue_tracker:
    provider: jira
    base_url: ""
  wiki:
    provider: confluence
    base_url: ""
  git_host:
    provider: gitlab
    base_url: ""
  # design: { provider: figma }
  # ci:     { provider: jenkins, base_url: "" }

# Optional per-repo exec verbs (override/extend the profile's):
# exec:
#   build: "./gradlew assembleDebug"
#   test:  { cmd: "./gradlew testDebugUnitTest", timeout_secs: 900 }
"#;

const GUIDE_TEMPLATE: &str = r#"# __PROJECT__ — palugada guide for AI agents

This project is wired to **palugada** (profile: `__PROFILE__`) — a CLI that
returns small, structured answers so you don't re-derive project knowledge by
reading lots of files. Prefer `--json` when you parse results.

## The loop: plan → code → execute → test → review

    palugada brief plan <ticket|goal> --json   # 1. context pack before coding
    # 2. edit code (use q / for / symbol below while editing)
    palugada exec build --json                 # 3. build; non-zero exit = failure
    palugada exec test --json                  # 4. tests; `tail` holds failures
    palugada brief bugfix <file> --json        #    on failure: focused pack, then retry 3-4
    palugada brief review --diff <ref> --json  # 5. pre-review pack scoped to the diff

`palugada exec --list` shows every runnable verb for this repo (build, test,
run, ui-dump, …) — the same verbs work in every palugada project, whatever the
stack. `palugada doctor` checks required tools and connector auth.

## Knowledge lookups

    palugada q <topic>              # a convention (q --list to enumerate)
    palugada for <task>             # a recipe (for --list)
    palugada s <keyword>            # search conventions + recipes
    palugada symbol <name>          # indexed code symbols (`palugada index` once first)
    palugada brief <flow> <target>  # budgeted pack; flows: plan, bugfix, feature, refactor, review, test

## Connectors (each works once configured in .palugada/config.yaml)

    palugada issue view <KEY>     # issue tracker
    palugada wiki page <ID>       # wiki / docs
    palugada design file <KEY>    # design
    palugada ci status <JOB>      # CI
    palugada git whoami           # git host
    palugada config verify        # test all configured connections

Conventions and recipes live in the bound profile and update without editing
this file. Regenerate this block with `palugada skills sync`.
"#;

const SKILL_TEMPLATE: &str = r#"---
name: palugada-__FLOW__
description: __DESC__ Gather context with palugada before editing.
---

# __TITLE__ (palugada)

When you __ACTION__, get a context pack first:

    palugada brief __FLOW__ <target> --json

Then follow the returned conventions and recipe. __HINT__
Prefer `palugada` output over guessing — knowledge lives in the bound
profile (`__PROFILE__`).
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_managed_appends_replaces_and_is_idempotent() {
        // fresh file
        let v1 = upsert_managed("", "guide v1");
        assert!(v1.starts_with(MARK_BEGIN) && v1.contains("guide v1"));
        // appends after user content
        let with_user = upsert_managed("# my notes\n", "guide v1");
        assert!(with_user.starts_with("# my notes"));
        assert!(with_user.contains(MARK_BEGIN));
        // replaces only the block, preserving user content around it
        let updated = upsert_managed(&format!("{with_user}\n# trailing\n"), "guide v2");
        assert!(updated.contains("# my notes") && updated.contains("# trailing"));
        assert!(updated.contains("guide v2") && !updated.contains("guide v1"));
        // idempotent
        assert_eq!(upsert_managed(&updated, "guide v2"), updated);
    }

    #[test]
    fn detect_profile_validates_against_bundled_profiles() {
        let kn = tempfile::tempdir().unwrap();
        for p in ["android-mvvm", "generic"] {
            let d = kn.path().join("profiles").join(p);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("profile.yaml"), format!("id: {p}\n")).unwrap();
        }
        let repo = tempfile::tempdir().unwrap();
        // bare repo → generic
        assert_eq!(detect_profile(repo.path(), Some(kn.path())), "generic");
        // gradle marker → android-mvvm (bundled → kept)
        fs::write(repo.path().join("build.gradle"), "").unwrap();
        assert_eq!(detect_profile(repo.path(), Some(kn.path())), "android-mvvm");
        // package.json repo → web-react NOT bundled in this fixture → generic
        let repo2 = tempfile::tempdir().unwrap();
        fs::write(repo2.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_profile(repo2.path(), Some(kn.path())), "generic");
        // no knowledge dir → trust the detection
        assert_eq!(detect_profile(repo2.path(), None), "web-react");
    }
}
```

- [ ] **Step 2: Wire `src/main.rs`:**

(a) `Commands` enum addition:

```rust
    /// Manage generated agent instruction files.
    Skills {
        #[command(subcommand)]
        action: SkillsCmd,
    },
```

(b) new subcommand enum (near `ProjectCmd`):

```rust
#[derive(Subcommand)]
enum SkillsCmd {
    /// Regenerate agent files (managed blocks + skills) for this repo.
    Sync {
        /// Repo path (default: current directory).
        #[arg(long, default_value = ".")]
        repo: String,
        /// Comma-separated targets (claude,codex,gemini,cursor).
        /// Default: whichever agent files already exist, else claude.
        #[arg(long, default_value = "")]
        agents: String,
    },
}
```

(c) dispatch:

```rust
        Commands::Skills { action } => match action {
            SkillsCmd::Sync { repo, agents } => scaffold::sync(repo, agents),
        },
```

(d) change `Init`'s default agents so a fresh init covers all four CLIs (managed blocks make this safe):

```rust
        /// Comma-separated agent targets: claude,codex,gemini,cursor.
        #[arg(long, default_value = "claude,codex,gemini,cursor")]
        agents: String,
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: PASS (2 new scaffold tests; all old ones still green).

- [ ] **Step 4: Manual smoke**

```bash
mkdir -p /tmp/plg-init && cd /tmp/plg-init && echo "# Existing notes" > CLAUDE.md
PALUGADA_KNOWLEDGE=/Users/septiandwisaputro/Documents/project/tools/palugada/knowledge \
  /Users/septiandwisaputro/Documents/project/tools/palugada/target/debug/palugada init
```
Expected: CLAUDE.md still starts with `# Existing notes` and now contains the managed block; `.claude/skills/palugada-plan/SKILL.md` … `palugada-test/SKILL.md` exist (6 skills); AGENTS.md, GEMINI.md, `.cursor/rules/palugada.mdc` created; config.yaml has `profile: generic`. Re-run → "skipped" for owned files, no root-file changes.

- [ ] **Step 5: Commit**

```bash
git add src/scaffold.rs src/main.rs
git commit -m "feat: scaffold v2 — managed marker blocks, data-driven agent targets, six namespaced skills, skills sync"
```

---

### Task 13: `src/profiles.rs` — `profile list` / `profile validate`

**Files:**
- Create: `src/profiles.rs`
- Modify: `src/brief.rs` (make `parse_step` pub(crate)), `src/main.rs` (Commands::Profile)

- [ ] **Step 1: In `src/brief.rs`, change `fn parse_step` to `pub(crate) fn parse_step`** (validate reuses it).

- [ ] **Step 2: Create `src/profiles.rs`** (complete file):

```rust
//! `palugada profile list|validate` — the authoring/lint path for profiles.
//! `validate` is the gate that keeps flows honest: every step kind must be
//! executable by `brief`, and every referenced convention/recipe must ship.

use crate::brief::{parse_step, KNOWN_STEP_KINDS};
use crate::config::VerbSpec;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Default)]
struct ProfileFull {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    flows: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    exec: BTreeMap<String, VerbSpec>,
    #[serde(default)]
    fact_families: Vec<FamilyDecl>,
}

#[derive(Deserialize)]
struct FamilyDecl {
    id: String,
}

#[derive(Deserialize, Default)]
struct ExtractorsFile {
    #[serde(default)]
    families: Vec<ExtractorFamily>,
}

#[derive(Deserialize)]
struct ExtractorFamily {
    id: String,
    regex: String,
}

pub fn list(kn: &Path) -> Result<(), String> {
    let dir = kn.join("profiles");
    let entries = fs::read_dir(&dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    let mut found = false;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !e.path().is_dir() || name.starts_with('_') {
            continue;
        }
        let pf: ProfileFull = fs::read_to_string(e.path().join("profile.yaml"))
            .ok()
            .and_then(|raw| serde_yaml::from_str(&raw).ok())
            .unwrap_or_default();
        let title = if pf.title.is_empty() { "(no title)" } else { &pf.title };
        println!("{name:<16} {title}");
        found = true;
    }
    if !found {
        println!("(no profiles under {})", dir.display());
    }
    Ok(())
}

pub fn validate(kn: &Path, id: &str) -> Result<(), String> {
    let dir = kn.join("profiles").join(id);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // profile.yaml
    let pf_path = dir.join("profile.yaml");
    let raw = fs::read_to_string(&pf_path).map_err(|e| format!("read {}: {e}", pf_path.display()))?;
    let pf: ProfileFull = match serde_yaml::from_str(&raw) {
        Ok(p) => p,
        Err(e) => return Err(format!("parse {}: {e}", pf_path.display())),
    };
    if pf.id != id {
        errors.push(format!("profile.yaml id '{}' does not match directory '{id}'", pf.id));
    }

    // flows reference only executable steps + shipped content
    for (flow, steps) in &pf.flows {
        for step in steps {
            let (kind, arg) = parse_step(step);
            if !KNOWN_STEP_KINDS.contains(&kind.as_str()) {
                errors.push(format!("flow '{flow}': unknown step kind '{step}'"));
                continue;
            }
            match kind.as_str() {
                "convention" => {
                    let p = dir.join("conventions").join(format!("{arg}.md"));
                    if !p.exists() {
                        errors.push(format!("flow '{flow}': convention '{arg}' has no file {}", p.display()));
                    }
                }
                "recipe" => {
                    let p = dir.join("recipes").join(format!("{arg}.md"));
                    if !p.exists() {
                        errors.push(format!("flow '{flow}': recipe '{arg}' has no file {}", p.display()));
                    }
                }
                _ => {}
            }
        }
    }

    // exec verbs have non-empty commands
    for (verb, spec) in &pf.exec {
        if spec.commands().iter().any(|c| c.trim().is_empty()) {
            errors.push(format!("exec verb '{verb}' has an empty command"));
        }
    }

    // extractors.yaml: regexes compile, named group present, ids sane
    let ex_path = dir.join("extractors.yaml");
    let mut extractor_ids: Vec<String> = Vec::new();
    if ex_path.exists() {
        let raw = fs::read_to_string(&ex_path).map_err(|e| format!("read {}: {e}", ex_path.display()))?;
        match serde_yaml::from_str::<ExtractorsFile>(&raw) {
            Ok(ex) => {
                for f in &ex.families {
                    extractor_ids.push(f.id.clone());
                    let id_ok = !f.id.is_empty()
                        && f.id.chars().all(|c| {
                            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-'
                        });
                    if !id_ok {
                        errors.push(format!("extractor family id '{}' is invalid ([a-z0-9_-])", f.id));
                    }
                    if let Err(e) = regex::Regex::new(&f.regex) {
                        errors.push(format!("family '{}': regex does not compile: {e}", f.id));
                    } else if !f.regex.contains("(?P<name>") {
                        errors.push(format!("family '{}': regex lacks the (?P<name>…) capture", f.id));
                    }
                }
            }
            Err(e) => errors.push(format!("parse {}: {e}", ex_path.display())),
        }
    }

    // fact_families ↔ extractors drift is a warning, not an error
    for fam in &pf.fact_families {
        if !extractor_ids.is_empty() && !extractor_ids.contains(&fam.id) {
            warnings.push(format!("fact_family '{}' has no extractor in extractors.yaml", fam.id));
        }
    }

    // _index.json files parse when present
    for rel in ["conventions/_index.json", "recipes/_index.json"] {
        let p = dir.join(rel);
        if p.exists() {
            if let Ok(raw) = fs::read_to_string(&p) {
                if let Err(e) = serde_json::from_str::<serde_json::Value>(&raw) {
                    errors.push(format!("parse {}: {e}", p.display()));
                }
            }
        }
    }

    for w in &warnings {
        println!("  warn  {w}");
    }
    if errors.is_empty() {
        println!("profile '{id}' OK ({} flows, {} exec verbs)", pf.flows.len(), pf.exec.len());
        Ok(())
    } else {
        for e in &errors {
            println!("  error {e}");
        }
        Err(format!("profile '{id}': {} error(s)", errors.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_profile(dir: &Path, id: &str, yaml: &str) {
        let d = dir.join("profiles").join(id);
        fs::create_dir_all(d.join("conventions")).unwrap();
        fs::create_dir_all(d.join("recipes")).unwrap();
        fs::write(d.join("profile.yaml"), yaml).unwrap();
    }

    #[test]
    fn validate_passes_a_consistent_profile() {
        let kn = tempfile::tempdir().unwrap();
        write_profile(kn.path(), "p", "id: p\nflows:\n  bugfix: [code.recent, convention(arch)]\nexec:\n  build: \"make\"\n");
        fs::write(kn.path().join("profiles/p/conventions/arch.md"), "# arch").unwrap();
        validate(kn.path(), "p").unwrap();
    }

    #[test]
    fn validate_catches_missing_content_and_unknown_steps() {
        let kn = tempfile::tempdir().unwrap();
        write_profile(kn.path(), "p", "id: p\nflows:\n  bugfix: [convention(missing), module.info]\n");
        let err = validate(kn.path(), "p").unwrap_err();
        assert!(err.contains("2 error(s)"), "{err}");
    }

    #[test]
    fn validate_catches_bad_regex_and_id_mismatch() {
        let kn = tempfile::tempdir().unwrap();
        write_profile(kn.path(), "p", "id: WRONG\n");
        fs::write(
            kn.path().join("profiles/p/extractors.yaml"),
            "families:\n  - id: f\n    regex: '(unclosed'\n",
        )
        .unwrap();
        let err = validate(kn.path(), "p").unwrap_err();
        assert!(err.contains("error(s)"), "{err}");
    }
}
```

- [ ] **Step 3: Wire `src/main.rs`** — `mod profiles;` after `mod knowledge;`; `Commands` enum:

```rust
    /// Inspect and validate knowledge profiles.
    Profile {
        #[command(subcommand)]
        action: ProfileCmd,
    },
```

subcommand enum:

```rust
#[derive(Subcommand)]
enum ProfileCmd {
    /// List bundled profiles.
    List,
    /// Validate a profile: YAML shapes, regexes, flow/content references.
    Validate { id: String },
}
```

dispatch:

```rust
        Commands::Profile { action } => {
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            match action {
                ProfileCmd::List => profiles::list(&kn),
                ProfileCmd::Validate { id } => profiles::validate(&kn, &id),
            }
        }
```

- [ ] **Step 4: Run tests + validate all bundled profiles**

Run: `cargo test` → PASS.
Run: `for p in android-mvvm web-react generic; do PALUGADA_KNOWLEDGE=$PWD/knowledge cargo run -q -- profile validate $p || exit 1; done`
Expected: three `profile '<id>' OK` lines — this is the proof that Task 10/11 content is consistent with the engine.

- [ ] **Step 5: Commit**

```bash
git add src/profiles.rs src/brief.rs src/main.rs
git commit -m "feat: profile list/validate — lint flows, content references, extractors, exec shapes"
```

---

### Task 14: Integration tests (`tests/cli.rs`)

**Files:**
- Create: `tests/cli.rs`

These run the REAL binary with `HOME` pointed at a tempdir (so no test touches `~/.palugada.yaml`) and `PALUGADA_KNOWLEDGE` at the repo's bundled knowledge.

- [ ] **Step 1: Create `tests/cli.rs`** (complete file):

```rust
//! End-to-end tests: run the built `palugada` binary against fixture repos.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn knowledge_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("knowledge")
}

struct Env {
    home: tempfile::TempDir,
    repo: tempfile::TempDir,
}

fn setup() -> Env {
    Env { home: tempfile::tempdir().unwrap(), repo: tempfile::tempdir().unwrap() }
}

fn run_in(env: &Env, dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_palugada"))
        .args(args)
        .env("HOME", env.home.path())
        .env("PALUGADA_KNOWLEDGE", knowledge_dir())
        .current_dir(dir)
        .output()
        .expect("binary runs")
}

fn run(env: &Env, args: &[&str]) -> Output {
    run_in(env, env.repo.path(), args)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

fn write_project_config(repo: &Path, yaml: &str) {
    fs::create_dir_all(repo.join(".palugada")).unwrap();
    fs::write(repo.join(".palugada").join("config.yaml"), yaml).unwrap();
}

#[test]
fn home_unset_is_a_clear_error() {
    let env = setup();
    let o = Command::new(env!("CARGO_BIN_EXE_palugada"))
        .args(["q", "--list"])
        .env_remove("HOME")
        .current_dir(env.repo.path())
        .output()
        .unwrap();
    assert!(!o.status.success());
    assert!(stderr(&o).contains("HOME"), "{}", stderr(&o));
}

#[test]
fn init_preserves_user_content_and_is_idempotent() {
    let env = setup();
    fs::write(env.repo.path().join("CLAUDE.md"), "# My precious notes\n").unwrap();
    let o = run(&env, &["init", "--agents", "claude,codex,gemini,cursor"]);
    assert!(o.status.success(), "{}", stderr(&o));

    let claude = fs::read_to_string(env.repo.path().join("CLAUDE.md")).unwrap();
    assert!(claude.starts_with("# My precious notes"), "user content clobbered:\n{claude}");
    assert!(claude.contains("<!-- palugada:begin -->") && claude.contains("palugada exec build"));
    for f in ["AGENTS.md", "GEMINI.md", ".cursor/rules/palugada.mdc"] {
        assert!(env.repo.path().join(f).exists(), "{f} missing");
    }
    for flow in ["plan", "bugfix", "feature", "refactor", "review", "test"] {
        let p = env.repo.path().join(".claude/skills").join(format!("palugada-{flow}")).join("SKILL.md");
        let body = fs::read_to_string(&p).unwrap_or_else(|_| panic!("missing skill {flow}"));
        assert!(body.contains(&format!("name: palugada-{flow}")));
    }
    // bare repo (no gradle/package.json) → generic
    let cfg = fs::read_to_string(env.repo.path().join(".palugada/config.yaml")).unwrap();
    assert!(cfg.contains("profile: generic"), "{cfg}");

    // second run must not duplicate the managed block
    let o2 = run(&env, &["init", "--agents", "claude"]);
    assert!(o2.status.success());
    let claude2 = fs::read_to_string(env.repo.path().join("CLAUDE.md")).unwrap();
    assert_eq!(claude2.matches("<!-- palugada:begin -->").count(), 1);
}

#[test]
fn exec_runs_verbs_and_propagates_exit_codes() {
    let env = setup();
    write_project_config(
        env.repo.path(),
        "project: t\nprofile: generic\nexec:\n  ok: \"echo hello\"\n  fail: \"exit 3\"\n  greet: \"echo hi {name}\"\n",
    );
    run(&env, &["init", "--agents", "claude"]); // registers the project

    let o = run(&env, &["exec", "ok", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["exit_code"], 0);
    assert!(v["tail"].as_str().unwrap().contains("hello"));

    let o = run(&env, &["exec", "fail", "--json"]);
    assert_eq!(o.status.code(), Some(3), "exit code must propagate");

    let o = run(&env, &["exec", "greet", "name=bob", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert!(v["tail"].as_str().unwrap().contains("hi bob"));

    let o = run(&env, &["exec", "greet"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("name=<value>"), "{}", stderr(&o));

    let o = run(&env, &["exec", "--list"]);
    let out = stdout(&o);
    assert!(out.contains("ok") && out.contains("fail") && out.contains("greet"), "{out}");
}

#[test]
fn brief_respects_budget_and_emits_json() {
    let env = setup();
    // android-mvvm fixture: gradle marker + a ViewModel for the indexer
    fs::write(env.repo.path().join("build.gradle"), "").unwrap();
    fs::write(env.repo.path().join("LoginViewModel.kt"), "class LoginViewModel {}\n").unwrap();
    let o = run(&env, &["init", "--agents", "claude"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let o = run(&env, &["index"]);
    assert!(o.status.success(), "{}", stderr(&o));

    let o = run(&env, &["brief", "bugfix", "LoginViewModel", "--json"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    let packs = v["packs"].as_array().unwrap();
    assert_eq!(packs.len(), 4, "bugfix flow has 4 steps");
    let all = stdout(&o);
    assert!(!all.contains("not yet available"), "hollow step leaked: {all}");
    assert!(!all.contains("unknown step kind"), "unknown step leaked: {all}");

    // tiny budget → used stays near it and later packs are omitted
    let o = run(&env, &["brief", "bugfix", "LoginViewModel", "--budget", "60", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert!(v["used_tokens"].as_u64().unwrap() <= 90, "{}", v["used_tokens"]);
    assert!(v["packs"].as_array().unwrap().iter().any(|p| p["omitted"] == true));

    // test flow includes exec hints from the profile
    let o = run(&env, &["brief", "test", "Login", "--json"]);
    assert!(stdout(&o).contains("gradlew"), "exec.hints missing: {}", stdout(&o));
}

#[test]
fn cwd_beats_active_project_and_typo_is_fatal() {
    let env = setup();
    // project A (registered first → active)
    let repo_a = env.repo.path().to_path_buf();
    write_project_config(&repo_a, "project: aaa\nprofile: generic\nexec:\n  which: \"echo AAA\"\n");
    run(&env, &["init", "--name", "aaa", "--agents", "claude"]);
    // project B in a second dir
    let repo_b = tempfile::tempdir().unwrap();
    write_project_config(repo_b.path(), "project: bbb\nprofile: generic\nexec:\n  which: \"echo BBB\"\n");
    run_in(&env, repo_b.path(), &["init", "--name", "bbb", "--agents", "claude"]);

    // from inside B, the cwd project must win over active A
    let o = run_in(&env, repo_b.path(), &["exec", "which", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert!(v["tail"].as_str().unwrap().contains("BBB"), "{v}");

    // a typo'd --project is a hard error, not a silent fallback
    let o = run_in(&env, repo_b.path(), &["exec", "which", "--project", "nope"]);
    assert!(!o.status.success());
    assert!(stderr(&o).contains("not registered"), "{}", stderr(&o));
}

#[test]
fn doctor_passes_with_true_check_and_fails_with_false() {
    let env = setup();
    write_project_config(env.repo.path(), "project: t\nprofile: generic\nexec:\n  doctor: \"true\"\n");
    run(&env, &["init", "--agents", "claude"]);
    let o = run(&env, &["doctor"]);
    assert!(o.status.success(), "{}\n{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).contains("PASS"));

    write_project_config(env.repo.path(), "project: t\nprofile: generic\nexec:\n  doctor: \"false\"\n");
    let o = run(&env, &["doctor"]);
    assert!(!o.status.success());
    assert!(stdout(&o).contains("FAIL"));
}

#[test]
fn bundled_profiles_validate() {
    let env = setup();
    for p in ["android-mvvm", "web-react", "generic"] {
        let o = run(&env, &["profile", "validate", p]);
        assert!(o.status.success(), "profile {p} invalid:\n{}\n{}", stdout(&o), stderr(&o));
    }
}

#[test]
fn skills_sync_refreshes_only_existing_targets() {
    let env = setup();
    run(&env, &["init", "--agents", "claude"]);
    // mutate the skill, then sync must restore it
    let skill = env.repo.path().join(".claude/skills/palugada-plan/SKILL.md");
    fs::write(&skill, "tampered").unwrap();
    let o = run(&env, &["skills", "sync"]);
    assert!(o.status.success(), "{}", stderr(&o));
    let body = fs::read_to_string(&skill).unwrap();
    assert!(body.contains("name: palugada-plan"), "{body}");
    // codex target wasn't initialized and must not appear
    assert!(!env.repo.path().join("AGENTS.md").exists());
}
```

- [ ] **Step 2: Run the suite**

Run: `cargo test --test cli`
Expected: 8 tests PASS. If `brief_respects_budget_and_emits_json` fails on pack count, check the android-mvvm `flows.bugfix` list (must be exactly 4 steps).

- [ ] **Step 3: Commit**

```bash
git add tests/cli.rs
git commit -m "test: end-to-end CLI suite — init idempotency, exec exit codes, brief budgets, cwd resolution, doctor, profile validation, skills sync"
```

---

### Task 15: Documentation (README + PRD update)

**Files:**
- Modify: `README.md`, `PRD-unified-palugada.md`

- [ ] **Step 1: README.md updates** (keep the existing structure; precise edits):

(a) Add to the bullet list at the top (after the "Budgeted context packs" bullet):

```markdown
- **An execution toolbelt** — `exec <verb>` runs profile-declared commands
  (gradle/[android-cli](https://developer.android.com/tools/agents/android-cli)
  on Android, npm on web) with uniform verbs, exit-code propagation, and
  `--json` outcomes; `doctor` checks tools + connector auth in one report.
```

(b) In the "What you can do" table add:

```markdown
| Build / test / run from any agent | `palugada exec build`, `exec test --json`, `exec --list` |
| Check the repo is agent-ready | `palugada doctor` |
| Refresh agent files after an update | `palugada skills sync` |
| Lint a profile you authored | `palugada profile validate <id>` |
```

(c) In the Commands table add rows:

```markdown
| `palugada exec <verb> [k=v …]` | run a profile/project exec verb (`--list`, `--json`; exit code = child's) |
| `palugada doctor` | tool checks (profile `doctor` verb) + connector verify rollup |
| `palugada skills sync` | regenerate agent files (managed blocks + skills) |
| `palugada profile list` / `validate <id>` | list / lint bundled profiles |
| `palugada project remove <name>` | unregister a project |
```

(d) Replace the agent-files table rows for `claude` (skills are now namespaced/six):

```markdown
| `claude` (default) | `CLAUDE.md` (managed block) + `.claude/skills/palugada-{plan,bugfix,feature,refactor,review,test}/SKILL.md` |
```

and note under the table: `init` writes root agent files as a `<!-- palugada:begin/end -->` managed block — existing content is preserved; default `--agents` is now `claude,codex,gemini,cursor`.

(e) Add a short section "## The end-to-end loop" after Quick start showing the five-command loop from the guide template (plan → edit → exec build → exec test → brief bugfix → brief review --diff).

(f) Update the Roadmap: remove "Flesh out the remaining brief flow steps … diff.scan", keep tree-sitter + provider expansion; add "connector write ops (PR create, CI trigger/log), personal PRD corpus, conventions-overlay merge, stats telemetry" as the deferred list.

- [ ] **Step 2: PRD-unified-palugada.md updates:**

(a) In §4.1's diagram intro add one line: "An `exec` pillar (v2) joins the engine: profile-declared verb→command maps run uniformly across stacks."

(b) Add new subsection at the end of §4 (after 4.5):

```markdown
### 4.6 Execution toolbelt (v2 addition)

`palugada exec <verb>` closes the execute/test leg of the agent loop. Verbs are
data: an `exec:` map in `profile.yaml` (overridable per-repo in
`.palugada/config.yaml`) binds uniform verbs — build / test / lint / run /
ui-dump / screenshot / doctor — to stack commands (gradle + android-cli for
android-mvvm, npm for web-react). The engine is one generic runner: `sh -c`,
per-verb timeout, child exit code propagated as palugada's own, `--json`
outcome `{verb, command, exit_code, duration_ms, tail}`. `palugada doctor`
runs the profile's `doctor` checks plus the connector verify rollup.
Android-cli integration is exactly these verb bindings — no Rust binding to
the `android` tool.
```

(c) In §7.2's table add rows for `skills sync` (now real), `project remove`, `profile list`, `profile validate`; add a §7.6:

```markdown
### 7.6 New — execution verbs

| Command | Does |
|---|---|
| `palugada exec <verb> [k=v …] [--json]` | run the profile/project-declared command(s) for `verb`; `{k}` placeholders from args; exits with the child's code |
| `palugada exec --list` | merged verb list with sources |
| `palugada doctor [--json]` | profile `doctor` tool checks + connector verifies; non-zero exit on any failure |
```

(d) In §4.3's example `profile.yaml`, add `plan`/`test` to `flows:` and an `exec:` example block (copy the android-mvvm one), and note flows may only use step kinds: convention, recipe, symbol.find, code.recent, issue.context, diff.scan, exec.hints — enforced by `profile validate`.

(e) §11 migration table: add row "**5b — Exec layer + loop flows (shipped as v2)** | exec engine + doctor + plan/test flows + brief steps issue.context/diff.scan/exec.hints + managed-block scaffolding | `cargo test` e2e suite green; `profile validate` passes for all bundled profiles".

(f) §13 (out of scope) add: connector write ops (PR create, CI trigger/log), personal PRD corpus + `context` command, conventions-overlay merge, `stats` telemetry — deferred from v2, sequenced next.

- [ ] **Step 3: Verify docs render and commit**

Run: `grep -n "exec" README.md | head` (sanity), `cargo test` (still green).

```bash
git add README.md PRD-unified-palugada.md
git commit -m "docs: document exec layer, doctor, six flows, managed scaffolding; PRD v2 addendum"
```

---

### Task 16: Final verification gate

**Files:** none (verification only)

- [ ] **Step 1: Full build + tests**

```bash
cargo build --release 2>&1 | tail -3   # expect: Finished `release`
cargo test 2>&1 | tail -5              # expect: all green, 0 failed
```

- [ ] **Step 2: Bundled-profile lint** (already covered by tests, re-run explicitly)

```bash
for p in android-mvvm web-react generic; do
  PALUGADA_KNOWLEDGE=$PWD/knowledge ./target/release/palugada profile validate $p || exit 1
done
```

- [ ] **Step 3: Real-repo smoke** (this repo)

```bash
PALUGADA_KNOWLEDGE=$PWD/knowledge ./target/release/palugada doctor || true   # report prints, graceful
PALUGADA_KNOWLEDGE=$PWD/knowledge ./target/release/palugada exec --list || true
./target/release/palugada --help | head -30                                  # new commands visible
```

Expected: doctor prints a report (no panic); --help lists exec, doctor, skills, profile.

- [ ] **Step 4: Re-read the spec** (`docs/superpowers/specs/2026-06-10-exec-layer-multi-cli-design.md`) section by section and tick: §2 exec (Task 7/8/10), §3 flows+brief (Task 9/10/11), §4 agent files (Task 12), §5 fixes (Tasks 1-6), §7 tests (Task 14), §8 PRD (Task 15). Any miss → fix before declaring done.

- [ ] **Step 5: Merge back**

Use the superpowers:finishing-a-development-branch skill: squash-merge or PR `feat/exec-layer-multi-cli` → `main` per the user's preference.

---

## Plan self-review notes (already applied)

- **Deviation from spec, intentional:** the spec's example `plan` flow listed `module.info`; that step kind is NOT implemented, and shipping flows that reference unimplemented steps is precisely review finding #6. All flows in Tasks 10/11 use only `KNOWN_STEP_KINDS`. Same for `convention(by-file-kind)` → replaced by `diff.scan`'s tag mapping + real conventions.
- **Type consistency checked:** `resolve_repo` returns `PathBuf` (callers pass `&Path` to indexer/brief/exec); `VerbSpec` lives in `config.rs` (exec.rs and profiles.rs import it from there); `ConnectorCtx` owns its `ProjectConfig`/`AuthProfile` (no lifetimes); `parse_step` becomes `pub(crate)` in Task 13 Step 1.
- **Test isolation:** every integration test sets `HOME` to a tempdir; no test reads or writes the developer's real `~/.palugada.yaml`. Unit tests never touch `HOME` except read-only `expand_home` assertions.
