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
    /// Skip the post-scaffold code index (default: index so `symbol`/`brief`
    /// work immediately).
    pub no_index: bool,
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
    pub merged: Vec<String>,
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
    // Never scaffold a project bound to a profile that isn't on disk — that used
    // to bind JS repos to a nonexistent `web-react` and then fail every command
    // with a raw `os error 2` (P1). Validate up front with a clear message.
    let mut global = GlobalConfig::load_or_default()?;
    if let Ok(kn) = crate::knowledge::knowledge_dir(&global) {
        if !kn.join("profiles").join(&profile).is_dir() {
            let avail: Vec<String> =
                crate::profile::list(&kn).unwrap_or_default().into_iter().map(|(id, _)| id).collect();
            return Err(format!(
                "profile '{}' does not exist under {}. Available: {}. Pass --profile <id>.",
                profile,
                kn.join("profiles").display(),
                if avail.is_empty() { "(none)".to_string() } else { avail.join(", ") }
            ));
        }
    }
    let auth = opts.auth.clone().unwrap_or_else(|| "default".to_string());
    let auto = opts.agents.is_empty() || (opts.agents.len() == 1 && opts.agents[0] == "auto");
    let agents = if auto { detect_agents(&repo) } else { opts.agents.clone() };

    let mut written: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    let mut merged: Vec<String> = Vec::new();

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
        write_agent_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped, &mut merged)?;
    }
    // per-profile custom skills (claude → .claude/skills, codex → .agents/skills;
    // additive — never fatal to init)
    if let Ok(kn) = crate::knowledge::knowledge_dir(&global) {
        let mut custom: Vec<(String, String)> = Vec::new();
        if agents.iter().any(|a| a == "claude") {
            custom.extend(custom_skill_files(&kn, &profile, ".claude/skills"));
        }
        if agents.iter().any(|a| a == "codex") {
            custom.extend(custom_skill_files(&kn, &profile, ".agents/skills"));
        }
        for (rel, body) in custom {
            write_file(&repo.join(&rel), &body, opts.force, &mut written, &mut skipped)?;
        }
    }

    // 3. register in the global project registry
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
        merged,
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
    for m in &out.merged {
        println!("  merged   {m}  (palugada section)");
    }
    for s in &out.skipped {
        println!("  skipped  {s}  (exists — use --force to overwrite)");
    }
    println!(
        "  registered in {}{}",
        GlobalConfig::default_path().display(),
        if out.became_active { " (now active)" } else { "" }
    );
    // Build the local symbol index now so `symbol`/`fact`/`brief` work on the
    // first try. Local-only (no network) and best-effort — never fails init.
    if !opts.no_index {
        index_after_init(&opts.repo, &out.profile);
    }
    println!("\nDone — 0 network calls. Next:");
    println!("  1. fill the integration base URLs in {}", out.config_path);
    println!("  2. add tokens to ~/.palugada/secrets.yaml under auth-profile '{}'", out.auth);
    println!("  3. run `palugada config verify`");
    Ok(())
}

/// Best-effort code index right after scaffolding. A profile without extractors,
/// a missing grammar, or an unresolved knowledge dir just prints a note and
/// leaves init succeeding — the user can always run `palugada index` later.
fn index_after_init(repo: &str, profile: &str) {
    let repo_path = match fs::canonicalize(expand_home(repo)) {
        Ok(p) => p,
        Err(_) => return,
    };
    let kn = match GlobalConfig::load_or_default().and_then(|g| crate::knowledge::knowledge_dir(&g)) {
        Ok(k) => k,
        Err(_) => return,
    };
    match crate::indexer::run(&repo_path, &kn, profile) {
        Ok(()) => {} // indexer::run prints "Indexed <repo> -> <out>"
        Err(e) => println!("  (index skipped: {e} — run `palugada index` when ready)"),
    }
}

fn detect_profile(repo: &Path) -> String {
    let has = |f: &str| repo.join(f).exists();
    if has("build.gradle") || has("build.gradle.kts") || has("settings.gradle") || has("settings.gradle.kts") {
        "android-mvvm".to_string()
    } else if has("Cargo.toml") {
        "rust-cli".to_string()
    } else if has("pubspec.yaml") {
        "flutter-bloc".to_string()
    } else {
        // No confident signal — default to android-mvvm (a bundled profile).
        // `generate` validates the result exists before scaffolding.
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

const MARK_START: &str = "<!-- palugada:start -->";
const MARK_END: &str = "<!-- palugada:end -->";

#[derive(Debug, PartialEq)]
enum SectionWrite {
    Created,
    Merged,
    Unchanged,
}

fn marked_block(content: &str) -> String {
    format!("{MARK_START}\n{}\n{MARK_END}\n", content.trim_end())
}

/// Insert-or-replace a palugada marker block in `path`, preserving any other
/// content. Creates the file (and parents) if absent.
fn upsert_marked_section(path: &Path, content: &str) -> Result<SectionWrite, String> {
    let block = marked_block(content);
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
        }
        fs::write(path, &block).map_err(|e| format!("write {}: {e}", path.display()))?;
        return Ok(SectionWrite::Created);
    }
    let existing = fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let updated = match (existing.find(MARK_START), existing.find(MARK_END)) {
        (Some(s), Some(e)) if e > s => {
            let end = e + MARK_END.len();
            format!("{}{}{}", &existing[..s], block.trim_end(), &existing[end..])
        }
        _ => {
            let sep = if existing.ends_with('\n') { "\n" } else { "\n\n" };
            format!("{existing}{sep}{block}")
        }
    };
    if updated == existing {
        return Ok(SectionWrite::Unchanged);
    }
    fs::write(path, &updated).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(SectionWrite::Merged)
}

/// Write one generated agent file. The three user-owned root guides
/// (CLAUDE.md/AGENTS.md/GEMINI.md) upsert a marker block (preserving user
/// content); every other path uses the write-if-missing/`--force` path.
pub fn write_agent_file(
    path: &Path,
    content: &str,
    force: bool,
    written: &mut Vec<String>,
    skipped: &mut Vec<String>,
    merged: &mut Vec<String>,
) -> Result<(), String> {
    let base = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    if matches!(base, "CLAUDE.md" | "AGENTS.md" | "GEMINI.md") {
        match upsert_marked_section(path, content)? {
            SectionWrite::Created => written.push(path.display().to_string()),
            SectionWrite::Merged => merged.push(path.display().to_string()),
            SectionWrite::Unchanged => skipped.push(path.display().to_string()),
        }
        Ok(())
    } else {
        write_file(path, content, force, written, skipped)
    }
}

/// Agents a repo already uses, by which guide files exist; `["claude"]` if none.
pub fn detect_agents(repo: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if repo.join("CLAUDE.md").exists() {
        out.push("claude".to_string());
    }
    if repo.join("AGENTS.md").exists() {
        out.push("codex".to_string());
    }
    if repo.join("GEMINI.md").exists() {
        out.push("gemini".to_string());
    }
    if repo.join(".cursor").exists() {
        out.push("cursor".to_string());
    }
    if out.is_empty() {
        out.push("claude".to_string());
    }
    out
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
pub const FLOW_SKILLS: &[(&str, &str, &str, &str)] = &[
    ("bugfix", "Bugfix", "fixing a bug, crash, or regression", "fix a bug"),
    ("feature", "Feature", "building a new feature, screen, or endpoint", "build a feature"),
    ("refactor", "Refactor", "refactoring or restructuring existing code", "refactor code"),
    ("review", "Review", "reviewing a diff, pull request, or merge request", "review changes"),
];

/// Build (repo-relative path, body) pairs for the rich, profile-agnostic skill
/// set. Claude gets a thin pointer + on-demand skills; codex/gemini/cursor get a
/// single richer guide file. Tool skills are gated by configured integrations.
pub fn skill_files(profile: &str, kinds: &[&str], agents: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for agent in agents {
        match agent.as_str() {
            "claude" => {
                out.push(("CLAUDE.md".into(), claude_pointer(profile, kinds)));
                out.extend(standard_skill_set(kinds, ".claude/skills"));
            }
            "codex" => {
                out.push(("AGENTS.md".into(), single_guide(profile, kinds)));
                out.extend(standard_skill_set(kinds, ".agents/skills"));
            }
            "gemini" => out.push(("GEMINI.md".into(), single_guide(profile, kinds))),
            "cursor" => {
                out.push((".cursor/rules/palugada.mdc".into(), cursor_wrap(&single_guide(profile, kinds))))
            }
            _ => {}
        }
    }
    out
}

/// The granular skill set under `base` (e.g. ".claude/skills" or ".agents/skills"):
/// search + one per flow + tool skills gated by configured integration kinds.
fn standard_skill_set(kinds: &[&str], base: &str) -> Vec<(String, String)> {
    let has = |k: &str| kinds.contains(&k);
    let path = |name: &str| format!("{base}/{name}/SKILL.md");
    let mut out = vec![(path("palugada-search"), SKILL_SEARCH.to_string())];
    for &(flow, title, trig, verb) in FLOW_SKILLS {
        out.push((path(&format!("palugada-{flow}")), skill_flow(flow, title, trig, verb)));
    }
    if has("git_host") {
        out.push((path("palugada-git"), SKILL_GIT.to_string()));
    }
    if has("issue_tracker") || has("wiki") {
        out.push((path("palugada-docs"), SKILL_DOCS.to_string()));
    }
    if has("ci") || has("chat") {
        out.push((path("palugada-ci"), SKILL_CI.to_string()));
    }
    if has("design") {
        out.push((path("palugada-design"), SKILL_DESIGN.to_string()));
    }
    out
}

/// User-authored custom skills for a profile: `profiles/<profile>/skills/<name>/SKILL.md`
/// → (`<base>/<name>/SKILL.md`, body) pairs (sorted; empty if none).
pub fn custom_skill_files(kn: &Path, profile: &str, base: &str) -> Vec<(String, String)> {
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
            out.push((format!("{base}/{name}/SKILL.md"), body));
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
    s.push_str("On-demand granular skills are generated under `.agents/skills/` for agents that load them (Codex); otherwise this file is the full guide.\n");
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

When you need a ticket / PRD / spec / wiki page, SEARCH this project's docs FIRST —
fetched docs are cached locally per-project, so it may already be here:

    palugada prd search <kw>      # search this project's fetched docs (offline)
    palugada prd list             # list them
    palugada prd cat <name>       # read one in full

To pull a NEW one — it is auto-saved into this project's `.palugada/docs/` cache
(so the next `prd search` finds it and the web console shows it):

    palugada wiki page <ID>       # a wiki/doc PAGE (Notion / Confluence)
    palugada prd fetch <KEY>      # a TICKET (Jira / GitHub Issues)
    palugada issue view <KEY>     # quick ticket view (no save)

Note: `prd fetch` reads the ISSUE TRACKER; for a Notion/wiki page use `wiki page <ID>`.
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
# auth-profile named below.
#
# Integrations are OPTIONAL and OFF by default — palugada's knowledge/index/brief
# features need none of them. Uncomment only the connectors you use, fill their
# base_url (+ repo for GitHub/CI), add tokens to ~/.palugada/secrets.yaml, then
# run `palugada config verify`.

project: __PROJECT__
profile: __PROFILE__
auth_profile: __AUTH__

integrations: {}
  # issue_tracker: { provider: jira,       base_url: "https://your.atlassian.net" }
  # wiki:          { provider: confluence, base_url: "https://your.atlassian.net/wiki" }
  # git_host:      { provider: gitlab,     base_url: "https://gitlab.com", repo: "owner/name" }
  # design:        { provider: figma }
  # ci:            { provider: jenkins,    base_url: "https://ci.example.com" }
  # chat:          { provider: slack }
"#;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_profile_maps_known_stacks_and_never_returns_web_react() {
        let d = tempfile::tempdir().unwrap();
        // no signal → android-mvvm (a bundled profile), never the old ghost.
        assert_eq!(detect_profile(d.path()), "android-mvvm");
        // package.json alone must NOT map to web-react (P1).
        std::fs::write(d.path().join("package.json"), "{}").unwrap();
        assert_ne!(detect_profile(d.path()), "web-react");
        // Rust / Flutter markers map to their bundled profiles.
        let r = tempfile::tempdir().unwrap();
        std::fs::write(r.path().join("Cargo.toml"), "").unwrap();
        assert_eq!(detect_profile(r.path()), "rust-cli");
        let f = tempfile::tempdir().unwrap();
        std::fs::write(f.path().join("pubspec.yaml"), "").unwrap();
        assert_eq!(detect_profile(f.path()), "flutter-bloc");
    }

    #[test]
    fn upsert_marked_section_creates_appends_replaces() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("AGENTS.md");

        assert!(matches!(upsert_marked_section(&p, "hello").unwrap(), SectionWrite::Created));
        let v1 = std::fs::read_to_string(&p).unwrap();
        assert!(v1.contains(MARK_START) && v1.contains("hello") && v1.contains(MARK_END));

        assert!(matches!(upsert_marked_section(&p, "hello").unwrap(), SectionWrite::Unchanged));

        assert!(matches!(upsert_marked_section(&p, "world").unwrap(), SectionWrite::Merged));
        let v2 = std::fs::read_to_string(&p).unwrap();
        assert!(v2.contains("world") && !v2.contains("hello"));
        assert_eq!(v2.matches(MARK_START).count(), 1);

        let u = tmp.path().join("CLAUDE.md");
        std::fs::write(&u, "# My notes\nkeep me\n").unwrap();
        assert!(matches!(upsert_marked_section(&u, "palu").unwrap(), SectionWrite::Merged));
        let uv = std::fs::read_to_string(&u).unwrap();
        assert!(uv.starts_with("# My notes\nkeep me\n"));
        assert!(uv.contains(MARK_START) && uv.contains("palu"));
    }

    #[test]
    fn detect_agents_reads_existing_guides() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        assert_eq!(detect_agents(r), vec!["claude".to_string()]);
        std::fs::write(r.join("AGENTS.md"), "x").unwrap();
        assert_eq!(detect_agents(r), vec!["codex".to_string()]);
        std::fs::write(r.join("CLAUDE.md"), "x").unwrap();
        std::fs::write(r.join("GEMINI.md"), "x").unwrap();
        assert_eq!(detect_agents(r), vec!["claude".to_string(), "codex".to_string(), "gemini".to_string()]);
    }

    #[test]
    fn custom_skill_files_reads_profile_skills_dir() {
        let kn = tempfile::tempdir().unwrap();
        let d = kn.path().join("profiles").join("p").join("skills").join("mvi-state");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("SKILL.md"), "---\nname: mvi-state\n---\n# MVI\n").unwrap();
        let files = custom_skill_files(kn.path(), "p", ".claude/skills");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, ".claude/skills/mvi-state/SKILL.md");
        assert!(files[0].1.contains("name: mvi-state"));
        assert!(custom_skill_files(kn.path(), "other", ".claude/skills").is_empty());
        // base dir is honored (codex → .agents/skills)
        let codex = custom_skill_files(kn.path(), "p", ".agents/skills");
        assert_eq!(codex[0].0, ".agents/skills/mvi-state/SKILL.md");
    }

    #[test]
    fn skill_files_emits_codex_agents_skills() {
        let f = skill_files("p", &["git_host"], &["codex".to_string()]);
        let has = |p: &str| f.iter().any(|(rel, _)| rel == p);
        assert!(has("AGENTS.md"));
        assert!(has(".agents/skills/palugada-search/SKILL.md"));
        assert!(has(".agents/skills/palugada-bugfix/SKILL.md"));
        assert!(has(".agents/skills/palugada-git/SKILL.md")); // gated by git_host
        // claude still uses .claude/skills
        let c = skill_files("p", &[], &["claude".to_string()]);
        assert!(c.iter().any(|(rel, _)| rel == ".claude/skills/palugada-search/SKILL.md"));
        // codex with no integrations → no tool skill
        let n = skill_files("p", &[], &["codex".to_string()]);
        assert!(!n.iter().any(|(rel, _)| rel == ".agents/skills/palugada-git/SKILL.md"));
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
        assert!(custom_skill_files(kn.path(), "p", ".claude/skills").iter().any(|(rel, _)| rel.contains("mvi-state")));
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
