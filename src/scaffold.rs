//! `palugada init` — instant, offline project scaffolding.
//!
//! Writes a per-project `.palugada/config.yaml`, registers the project, and
//! generates agent instruction files for whichever AI tools the team uses.
//! No network calls and no credential prompts (tokens live in
//! `~/.palugada/secrets.yaml`, entered separately).

use crate::config::{expand_home, GlobalConfig, ProjectEntry};
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

/// (flow id, description, verb phrase, title)
const FLOWS: &[(&str, &str, &str, &str)] = &[
    ("bugfix", "Fix a bug or crash.", "fix a bug", "Bugfix"),
    ("feature", "Build a new feature or screen.", "build a feature", "Feature"),
    ("refactor", "Refactor or restructure code.", "refactor code", "Refactor"),
    ("review", "Review a diff or pull/merge request.", "review a diff", "Review"),
];

/// Result of generating a project's scaffold (files + registry), reusable by
/// both `cmd_init` (CLI) and the web console's `/api/init`.
pub struct GenerateOutcome {
    pub name: String,
    pub profile: String,
    pub auth: String,
    pub agents: Vec<String>,
    pub written: Vec<String>,
    pub skipped: Vec<String>,
    pub became_active: bool,
    pub config_path: String,
}

/// Generate a project's config skeleton + agent files + registry entry. No
/// printing — the caller reports. This is the reusable core of `init`.
pub fn generate(opts: &InitOptions) -> Result<GenerateOutcome, String> {
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
    let profile = opts.profile.clone().unwrap_or_else(|| detect_profile(&repo));
    let auth = opts.auth.clone().unwrap_or_else(|| "default".to_string());
    let agents = if opts.agents.is_empty() {
        vec!["claude".to_string()]
    } else {
        opts.agents.clone()
    };

    let mut written: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    // 1. per-project config skeleton
    let cfg = repo.join(".palugada").join("config.yaml");
    write_file(&cfg, &config_skeleton(&name, &profile, &auth), opts.force, &mut written, &mut skipped)?;

    // 2. agent files (shared body, per-tool filename/format)
    let guide = agent_guide(&name, &profile);
    for agent in &agents {
        match agent.as_str() {
            "claude" => {
                write_file(&repo.join("CLAUDE.md"), &guide, opts.force, &mut written, &mut skipped)?;
                for &(flow, desc, action, title) in FLOWS {
                    let p = repo.join(".claude").join("skills").join(flow).join("SKILL.md");
                    let body = agent_skill(flow, desc, action, title, &profile);
                    write_file(&p, &body, opts.force, &mut written, &mut skipped)?;
                }
            }
            "codex" => {
                write_file(&repo.join("AGENTS.md"), &guide, opts.force, &mut written, &mut skipped)?;
            }
            "gemini" => {
                write_file(&repo.join("GEMINI.md"), &guide, opts.force, &mut written, &mut skipped)?;
            }
            "cursor" => {
                let p = repo.join(".cursor").join("rules").join("palugada.mdc");
                write_file(&p, &cursor_wrap(&guide), opts.force, &mut written, &mut skipped)?;
            }
            other => {
                return Err(format!(
                    "unknown agent target: '{other}' (supported: claude, codex, gemini, cursor)"
                ));
            }
        }
    }

    // 3. register in the global project registry
    let mut global = GlobalConfig::load_or_default()?;
    let workspace = format!("{}/.palugada", repo_str.trim_end_matches('/'));
    global.projects.registered.insert(
        name.clone(),
        ProjectEntry { repo_path: repo_str.clone(), workspace },
    );
    let became_active = global.projects.active.is_empty();
    if became_active {
        global.projects.active = name.clone();
    }
    global.save()?;

    Ok(GenerateOutcome {
        name,
        profile,
        auth,
        agents,
        written,
        skipped,
        became_active,
        config_path: cfg.display().to_string(),
    })
}

pub fn run(opts: InitOptions) -> Result<(), String> {
    let out = generate(&opts)?;
    println!(
        "palugada init — project '{}' (profile: {}, auth: {}, agents: {})",
        out.name, out.profile, out.auth, out.agents.join(",")
    );
    for w in &out.written {
        println!("  wrote    {w}");
    }
    for s in &out.skipped {
        println!("  skipped  {s}  (exists — use --force to overwrite)");
    }
    println!(
        "  registered in {}{}",
        GlobalConfig::default_path().display(),
        if out.became_active { " (now active)" } else { "" }
    );
    println!("\nDone — 0 network calls. Next:");
    println!("  1. fill the integration base URLs in {}", out.config_path);
    println!("  2. add tokens to ~/.palugada/secrets.yaml under auth-profile '{}'", out.auth);
    println!("  3. run `palugada config verify`");
    Ok(())
}

fn detect_profile(repo: &Path) -> String {
    let has = |f: &str| repo.join(f).exists();
    if has("build.gradle") || has("build.gradle.kts") || has("settings.gradle") || has("settings.gradle.kts") {
        "android-mvvm".to_string()
    } else if has("package.json") {
        "web-react".to_string()
    } else {
        // android-mvvm is the only profile shipped with content today.
        "android-mvvm".to_string()
    }
}

fn write_file(
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

fn agent_skill(flow: &str, desc: &str, action: &str, title: &str, profile: &str) -> String {
    SKILL_TEMPLATE
        .replace("__FLOW__", flow)
        .replace("__DESC__", desc)
        .replace("__ACTION__", action)
        .replace("__TITLE__", title)
        .replace("__PROFILE__", profile)
}

fn cursor_wrap(body: &str) -> String {
    format!(
        "---\ndescription: palugada — project context CLI guide\nalwaysApply: true\n---\n\n{body}"
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
"#;

const GUIDE_TEMPLATE: &str = r#"# __PROJECT__ — working with palugada

This project is wired to **palugada**, a CLI that gives AI agents token-cheap,
always-current engineering context. Bound profile: `__PROFILE__`.

**Principle:** don't re-derive project knowledge by reading lots of files — ask
`palugada`; it returns small, structured answers.

## Connectors (work today)

    palugada issue view <KEY>     # issue tracker (Jira)
    palugada wiki page <ID>       # wiki / docs (Confluence)
    palugada design file <KEY>    # design (Figma)
    palugada ci status <JOB>      # CI (Jenkins)
    palugada git whoami           # git host (GitLab/GitHub)
    palugada config verify        # check every configured connection

## Knowledge & flows

For a scoped task, ask for one budgeted context pack:

    palugada brief bugfix   <file|symbol>
    palugada brief feature  <ticket|area>
    palugada brief refactor <module|file>
    palugada brief review   --diff <ref>

Or look things up directly:

    palugada q <topic>            # a convention (e.g. palugada q architecture)
    palugada for <task>           # a recipe (e.g. palugada for feature)

> The knowledge layer (brief / q / for / indexed facts) is being rolled out; the
> connector commands above already work. Conventions and recipes live in the
> bound profile and update without editing this file — regenerate with
> `palugada init` or `palugada skills sync`.
"#;

const SKILL_TEMPLATE: &str = r#"---
name: __FLOW__
description: __DESC__ Gather context with palugada before editing.
---

# __TITLE__

When you __ACTION__, get a context pack first:

    palugada brief __FLOW__ <target>

Then follow the returned conventions and recipe. Prefer `palugada` output over
guessing — the knowledge lives in the bound profile (`__PROFILE__`).
"#;
