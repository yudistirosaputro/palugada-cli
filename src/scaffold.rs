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

    // 2. agent files (rich, profile-agnostic, gated by configured integrations)
    for agent in &agents {
        if !matches!(agent.as_str(), "claude" | "codex" | "gemini" | "cursor") {
            return Err(format!(
                "unknown agent target: '{agent}' (supported: claude, codex, gemini, cursor)"
            ));
        }
    }
    let pc = crate::config::ProjectConfig::load_from(&repo_str)?;
    let kinds = integration_kinds(&pc);
    for (rel, body) in skill_files(&profile, &kinds, &agents) {
        write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
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

// ── rich, profile-agnostic skill set ───────────────────────────────────────

/// Which integrations a project declares (drives which tool skills get generated).
pub fn integration_kinds(pc: &crate::config::ProjectConfig) -> Vec<&'static str> {
    let i = &pc.integrations;
    let mut k = Vec::new();
    if i.issue_tracker.is_some() { k.push("issue_tracker"); }
    if i.wiki.is_some() { k.push("wiki"); }
    if i.git_host.is_some() { k.push("git_host"); }
    if i.design.is_some() { k.push("design"); }
    if i.ci.is_some() { k.push("ci"); }
    if i.chat.is_some() { k.push("chat"); }
    k
}

/// (flow id, Title, trigger phrase, verb phrase)
const FLOW_SKILLS: &[(&str, &str, &str, &str)] = &[
    ("bugfix", "Bugfix", "fixing a bug, crash, or regression", "fix a bug"),
    ("feature", "Feature", "building a new feature, screen, or endpoint", "build a feature"),
    ("refactor", "Refactor", "refactoring or restructuring existing code", "refactor code"),
    ("review", "Review", "reviewing a diff, pull request, or merge request", "review changes"),
];

/// Build (repo-relative path, body) pairs for the rich, profile-agnostic skill
/// set. Claude gets a thin pointer + on-demand skills; codex/gemini/cursor get a
/// single richer guide file. Tool skills are gated by configured integrations.
pub fn skill_files(profile: &str, kinds: &[&str], agents: &[String]) -> Vec<(String, String)> {
    let has = |k: &str| kinds.contains(&k);
    let mut out: Vec<(String, String)> = Vec::new();
    for agent in agents {
        match agent.as_str() {
            "claude" => {
                out.push(("CLAUDE.md".into(), claude_pointer(profile, kinds)));
                out.push((skill_path("palugada-search"), SKILL_SEARCH.to_string()));
                for &(flow, title, trig, verb) in FLOW_SKILLS {
                    out.push((skill_path(&format!("palugada-{flow}")), skill_flow(flow, title, trig, verb)));
                }
                if has("git_host") {
                    out.push((skill_path("palugada-git"), SKILL_GIT.to_string()));
                }
                if has("issue_tracker") || has("wiki") {
                    out.push((skill_path("palugada-docs"), SKILL_DOCS.to_string()));
                }
                if has("ci") || has("chat") {
                    out.push((skill_path("palugada-ci"), SKILL_CI.to_string()));
                }
                if has("design") {
                    out.push((skill_path("palugada-design"), SKILL_DESIGN.to_string()));
                }
            }
            "codex" => out.push(("AGENTS.md".into(), single_guide(profile, kinds))),
            "gemini" => out.push(("GEMINI.md".into(), single_guide(profile, kinds))),
            "cursor" => {
                out.push((".cursor/rules/palugada.mdc".into(), cursor_wrap(&single_guide(profile, kinds))))
            }
            _ => {}
        }
    }
    out
}

fn skill_path(name: &str) -> String {
    format!(".claude/skills/{name}/SKILL.md")
}

/// User-authored custom skills for a profile: `profiles/<profile>/skills/<name>/SKILL.md`
/// → (".claude/skills/<name>/SKILL.md", body) pairs (sorted; empty if none).
pub fn custom_skill_files(kn: &Path, profile: &str) -> Vec<(String, String)> {
    let dir = kn.join("profiles").join(profile).join("skills");
    let mut out: Vec<(String, String)> = Vec::new();
    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let skill = entry.path().join("SKILL.md");
        if let Ok(body) = fs::read_to_string(&skill) {
            out.push((format!(".claude/skills/{name}/SKILL.md"), body));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Scaffold a starter custom skill into `profiles/<profile>/skills/<name>/SKILL.md`.
pub fn new_custom_skill(kn: &Path, profile: &str, name: &str) -> Result<std::path::PathBuf, String> {
    let valid = !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid {
        return Err(format!("invalid skill name '{name}' — use only [a-z0-9-_]"));
    }
    if name.starts_with("palugada-") {
        return Err(format!(
            "'{name}' uses the reserved 'palugada-' prefix (that namespace is the generated standard set)"
        ));
    }
    let dir = kn.join("profiles").join(profile).join("skills").join(name);
    let p = dir.join("SKILL.md");
    if p.exists() {
        return Err(format!("custom skill '{name}' already exists at {}", p.display()));
    }
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    fs::write(&p, custom_skill_template(name)).map_err(|e| format!("write {}: {e}", p.display()))?;
    Ok(p)
}

fn custom_skill_template(name: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: >\n  TRIGGER when … (describe when an agent should use this skill).\nallowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit\n---\n\n# {name}\n\nDescribe the task here. Pull rules from the profile instead of inlining them:\n\n    palugada for <task>     # a recipe   (`palugada for --list`)\n    palugada q <topic>      # a convention (`palugada q --list`)\n\nLocate code with the `palugada-search` skill before grepping.\n"
    )
}

fn claude_pointer(profile: &str, kinds: &[&str]) -> String {
    let mut s = String::new();
    s.push_str("# Working with palugada\n\n");
    s.push_str("This project uses **palugada** for token-cheap, always-current engineering\n");
    s.push_str("context — ask palugada instead of re-reading lots of files.\n\n");
    s.push_str("**Before** grepping for code (`grep`/`find`/`rg`/Glob), use the index:\n");
    s.push_str("`palugada symbol <name>` / `palugada fact <family>`.\n\n");
    s.push_str("On-demand skills (loaded by trigger):\n");
    s.push_str("- `palugada-search` — locate code/symbols (use before grep)\n");
    s.push_str("- `palugada-bugfix` / `-feature` / `-refactor` / `-review` — scoped task packs via `palugada brief`\n");
    if kinds.contains(&"git_host") {
        s.push_str("- `palugada-git` — git, PR/MR, commit conventions\n");
    }
    if kinds.contains(&"issue_tracker") || kinds.contains(&"wiki") {
        s.push_str("- `palugada-docs` — issues, wiki pages, PRDs\n");
    }
    if kinds.contains(&"ci") || kinds.contains(&"chat") {
        s.push_str("- `palugada-ci` — CI status & chat notify\n");
    }
    if kinds.contains(&"design") {
        s.push_str("- `palugada-design` — Figma files\n");
    }
    s.push_str("\nDiscover: `palugada q --list` (conventions) · `palugada for --list` (recipes) · `palugada <cmd> --help`.\n");
    s.push_str(&format!("Bound profile: `{profile}` — switch with `palugada profile use <id>` (skills follow the active profile automatically).\n"));
    s
}

fn skill_flow(flow: &str, title: &str, trig: &str, verb: &str) -> String {
    let review_note = if flow == "review" {
        "\nThis flow is diff-scoped — point it at a ref: `palugada brief review <ref>`.\n"
    } else {
        "\nLocate code with the `palugada-search` skill — never blind-grep.\n"
    };
    format!(
        "---\nname: palugada-{flow}\ndescription: TRIGGER when {trig}. Gather a context pack with palugada before editing.\nallowed-tools: Bash(palugada *), Read, Grep, Glob, Write, Edit\n---\n\n# {title}\n\nWhen you {verb}, get ONE budgeted context pack first:\n\n    palugada brief {flow} <target>     # recent changes + symbols + the relevant conventions\n\nThen pull only the rules you need (don't guess — the knowledge lives in the profile):\n\n    palugada for <task>                # a recipe; `palugada for --list` to see all\n    palugada q <topic>                 # a convention; `palugada q --list` to see all\n{review_note}"
    )
}

fn single_guide(profile: &str, kinds: &[&str]) -> String {
    let mut s = String::new();
    s.push_str("# Working with palugada\n\n");
    s.push_str("This project uses **palugada** for token-cheap, always-current context.\n");
    s.push_str("Ask palugada instead of re-reading files.\n\n");
    s.push_str("## Find code FIRST (before grep)\n\n");
    s.push_str("    palugada symbol <name> [--kind function]   # any definition (class/function/method/...)\n");
    s.push_str("    palugada fact <family> [name]              # curated facts (viewmodel/route/...)\n\n");
    s.push_str("Run these BEFORE any `grep`/`find`/`rg`; grep is the fallback only when the index is empty.\n\n");
    s.push_str("## Scoped task packs\n\n");
    s.push_str("    palugada brief bugfix|feature|refactor|review <target>\n");
    s.push_str("    palugada for <task>      # a recipe   (`palugada for --list`)\n");
    s.push_str("    palugada q <topic>       # a convention (`palugada q --list`)\n\n");
    let mut conn: Vec<&str> = Vec::new();
    if kinds.contains(&"issue_tracker") || kinds.contains(&"wiki") {
        conn.push("    palugada issue view <KEY> | wiki page <ID> | prd fetch <KEY>\n");
    }
    if kinds.contains(&"git_host") {
        conn.push("    palugada git whoami | pr recent <file>\n");
    }
    if kinds.contains(&"ci") || kinds.contains(&"chat") {
        conn.push("    palugada ci status <JOB> | notify \"<message>\"\n");
    }
    if kinds.contains(&"design") {
        conn.push("    palugada design file <KEY>\n");
    }
    if !conn.is_empty() {
        s.push_str("## Connectors\n\n");
        for c in conn {
            s.push_str(c);
        }
        s.push('\n');
    }
    s.push_str(&format!("Bound profile `{profile}` — switch with `palugada profile use <id>`. `palugada <cmd> --help` for anything.\n"));
    s
}

const SKILL_SEARCH: &str = r#"---
name: palugada-search
description: >
  TRIGGER when locating code — find a function/class/symbol, "where is X defined",
  what calls/lives in a module — and BEFORE any grep/find/rg/Glob over the repo.
allowed-tools: Bash(palugada *), Grep, Glob, Read
---

# Locate code via palugada FIRST

The project is indexed. Use the index before grepping.

    palugada symbol <name>                   # any definition: class/function/method/property
    palugada symbol <name> --kind function   # narrow by kind
    palugada fact <family> [name]            # curated facts (e.g. viewmodel, route)

**Hard rule:** run `palugada symbol` / `palugada fact` BEFORE any `grep`,
`find`, `rg`, or `Glob` for code. grep is the fallback ONLY when the index
returns nothing — and when that happens, say so (the indexer missed something
worth fixing) and refresh with `palugada index`.
"#;

const SKILL_GIT: &str = r#"---
name: palugada-git
description: >
  TRIGGER for git work — branch, commit, push, rebase/merge conflict, pull/merge
  request, pipeline. Also when the user mentions gh, glab, GitHub, or GitLab.
allowed-tools: Bash(palugada *), Bash(git *), Bash(gh *), Bash(glab *), Read, Grep, Glob, Write, Edit
---

# Git & PR/MR

    palugada git whoami           # confirm the authenticated git-host user
    palugada pr recent <file>     # recent commits touching a file (host reverse-index)

## Commits & branches

    type(scope): lowercase summary      e.g. feat(watchlist): add sort
    type/TICKET-short-description        e.g. feat/UATP-1602-watchlist-sort

## PR / MR

Use `gh` (GitHub) or `glab` (GitLab) to create / list / review / merge.

## Safety

- Never `git push --force` — use `--force-with-lease`.
- Only rebase YOUR feature branch, never a shared one.
- Resolve conflicts per-hunk; build + test after; `git rebase --abort` is safe if unsure.
"#;

const SKILL_DOCS: &str = r#"---
name: palugada-docs
description: TRIGGER for tickets, issues, wiki/Confluence/Notion pages, PRDs, or specs.
allowed-tools: Bash(palugada *), Read
---

# Issues, wiki & PRDs

    palugada issue view <KEY>     # a ticket (Jira / GitHub Issues)
    palugada wiki page <ID>       # a wiki/doc page (Confluence / Notion)
    palugada prd fetch <KEY>      # save a ticket into the personal corpus
    palugada prd list             # list saved corpus docs
    palugada prd cat <name>       # read one
    palugada prd search <kw>      # search the corpus offline
"#;

const SKILL_CI: &str = r#"---
name: palugada-ci
description: TRIGGER for CI/build status, pipelines, or notifying the team chat.
allowed-tools: Bash(palugada *), Read
---

# CI & notify

    palugada ci status <JOB>      # last build status (Jenkins / GH Actions / GitLab CI)
    palugada notify "<message>"   # post to the project chat (Slack)
"#;

const SKILL_DESIGN: &str = r#"---
name: palugada-design
description: TRIGGER for Figma design files or design specs.
allowed-tools: Bash(palugada *), Read
---

# Design

    palugada design file <KEY>    # a Figma file's metadata (name, version, last modified)
"#;

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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_skill_files_reads_profile_skills_dir() {
        let kn = tempfile::tempdir().unwrap();
        let d = kn.path().join("profiles").join("p").join("skills").join("mvi-state");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "---\nname: mvi-state\n---\n# MVI\n").unwrap();
        let files = custom_skill_files(kn.path(), "p");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, ".claude/skills/mvi-state/SKILL.md");
        assert!(files[0].1.contains("name: mvi-state"));
        assert!(custom_skill_files(kn.path(), "other").is_empty());
    }

    #[test]
    fn new_custom_skill_scaffolds_and_guards() {
        let kn = tempfile::tempdir().unwrap();
        let p = new_custom_skill(kn.path(), "p", "mvi-state").unwrap();
        assert!(p.exists());
        assert!(fs::read_to_string(&p).unwrap().contains("name: mvi-state"));
        assert!(new_custom_skill(kn.path(), "p", "mvi-state").is_err()); // duplicate
        assert!(new_custom_skill(kn.path(), "p", "palugada-foo").unwrap_err().contains("reserved"));
        assert!(new_custom_skill(kn.path(), "p", "Bad Name").is_err());
        assert!(custom_skill_files(kn.path(), "p").iter().any(|(rel, _)| rel.contains("mvi-state")));
    }

    #[test]
    fn skill_files_gates_tool_skills_by_integration() {
        let only_git = skill_files("android-mvvm", &["git_host"], &["claude".to_string()]);
        let names: Vec<&str> = only_git.iter().map(|(p, _)| p.as_str()).collect();
        assert!(names.iter().any(|p| p.contains("palugada-search/SKILL.md")));
        assert!(names.iter().any(|p| p.contains("palugada-bugfix/SKILL.md")));
        assert!(names.iter().any(|p| p.contains("palugada-git/SKILL.md")));
        assert!(!names.iter().any(|p| p.contains("palugada-docs/SKILL.md")));
        assert!(!names.iter().any(|p| p.contains("palugada-ci/SKILL.md")));
        assert!(!names.iter().any(|p| p.contains("palugada-design/SKILL.md")));

        let all = skill_files(
            "android-mvvm",
            &["git_host", "wiki", "issue_tracker", "ci", "design"],
            &["claude".to_string()],
        );
        let an: Vec<&str> = all.iter().map(|(p, _)| p.as_str()).collect();
        for s in ["palugada-docs", "palugada-ci", "palugada-design"] {
            assert!(an.iter().any(|p| p.contains(s)), "missing {s}");
        }
    }

    #[test]
    fn skill_bodies_are_profile_agnostic_references() {
        let files = skill_files("android-mvvm", &["git_host"], &["claude".to_string()]);
        let search = files.iter().find(|(p, _)| p.contains("palugada-search")).unwrap();
        assert!(search.1.contains("palugada symbol"), "search must reference symbol");
        assert!(
            search.1.to_lowercase().contains("before") && search.1.contains("grep"),
            "search-first rule missing"
        );
        let flow = files.iter().find(|(p, _)| p.contains("palugada-feature")).unwrap();
        assert!(flow.1.contains("palugada brief feature"));

        // codex → single guide file, no skills dir
        let guide = skill_files("android-mvvm", &["git_host"], &["codex".to_string()]);
        assert!(guide.iter().any(|(p, _)| p == "AGENTS.md"));
        assert!(!guide.iter().any(|(p, _)| p.contains(".claude/skills")));
    }
}
