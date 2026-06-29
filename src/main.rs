//! palugada — project-agnostic dev knowledge & connector CLI (base).
//!
//! This first slice wires the connector layer: config + credentials, a project
//! registry, and Jira / Confluence / Git host commands. Knowledge commands
//! (`q`, `for`, `brief`, the indexer) come on top of this base.

mod brief;
mod clients;
mod config;
mod credentials;
mod effective;
mod exec;
mod http;
mod indexer;
mod inherit;
mod knowledge;
mod personal;
mod profile;
mod scaffold;
mod skillmap;
mod web;

use clap::{Parser, Subcommand};
use config::{mask_secret, resolve_project, GlobalConfig, ProjectEntry, Secrets};

#[derive(Parser)]
#[command(name = "palugada", version, about = "Project-agnostic dev knowledge & connector CLI")]
struct Cli {
    /// Accept self-signed TLS certificates (corporate hosts).
    #[arg(long, global = true)]
    insecure: bool,

    /// Target a specific registered project (defaults to the active one).
    #[arg(long, global = true)]
    project: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold palugada into a project (offline): config + agent files.
    Init {
        /// Repo path (default: current directory).
        #[arg(long, default_value = ".")]
        repo: String,
        /// Project name (default: repo directory name).
        #[arg(long)]
        name: Option<String>,
        /// Stack profile to bind (default: auto-detect).
        #[arg(long)]
        profile: Option<String>,
        /// Auth-profile in ~/.palugada/secrets.yaml (default: "default").
        #[arg(long)]
        auth: Option<String>,
        /// Comma-separated agent targets: claude,codex,gemini,cursor.
        #[arg(long, default_value = "auto")]
        agents: String,
        /// Overwrite existing files.
        #[arg(long)]
        force: bool,
    },
    /// Manage global config and credentials.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
    },
    /// Read a convention from the active profile: `q <topic>[.N]`.
    #[command(name = "q")]
    Query {
        /// Topic id; `.N` for the N-th section or `#id` for a section by anchor (e.g. `architecture.2` or `architecture#data-flow`).
        topic: Option<String>,
        /// Brief: show the section outline only.
        #[arg(short, long)]
        brief: bool,
        /// List all topics.
        #[arg(long)]
        list: bool,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Read a recipe from the active profile: `for <task>`.
    #[command(name = "for")]
    ForTask {
        /// Recipe id (e.g. `feature`).
        task: Option<String>,
        /// List all recipes.
        #[arg(long)]
        list: bool,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Search conventions + recipes by keyword: `s <kw>`.
    #[command(name = "s")]
    Search {
        query: String,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Index the project's code into <repo>/.palugada/index/ (local, per-dev).
    Index {
        /// Repo to index (default: active project's repo, else current dir).
        #[arg(long)]
        repo: Option<String>,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Search indexed project symbols by name: `symbol <query>`.
    Symbol {
        query: String,
        /// Filter by kind (class, object, function, method, property).
        #[arg(long)]
        kind: Option<String>,
        /// Repo to search (default: active project's repo, else current dir).
        #[arg(long)]
        repo: Option<String>,
    },
    /// Look up indexed facts of a profile-declared family: `fact <family> [name]`.
    Fact {
        /// Fact family id declared in the profile (e.g. viewmodel, route).
        family: String,
        /// Optional name substring filter.
        name: Option<String>,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Assemble a budgeted context pack for a flow: `brief <flow> [target]`.
    Brief {
        /// Flow id (e.g. bugfix, feature, refactor, review).
        flow: String,
        /// Target: a file, symbol, ticket, or area (flow-dependent).
        #[arg(default_value = "")]
        target: String,
        /// Token budget for the pack.
        #[arg(long, default_value_t = 2000)]
        budget: usize,
        /// Emit JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
    /// Manage the project registry.
    Project {
        #[command(subcommand)]
        action: ProjectCmd,
    },
    /// List, validate, or scaffold stack profiles.
    Profile {
        #[command(subcommand)]
        action: ProfileCmd,
    },
    /// Author a convention from a plain markdown file.
    Convention {
        #[command(subcommand)]
        action: ConventionCmd,
    },
    /// Author a recipe from a plain markdown file.
    Recipe {
        #[command(subcommand)]
        action: RecipeCmd,
    },
    /// (Re)generate this project's agent skill files.
    Skills {
        #[command(subcommand)]
        action: SkillsCmd,
    },
    /// Work with the project's issue tracker.
    Issue {
        #[command(subcommand)]
        action: IssueCmd,
    },
    /// Work with the project's wiki / doc source.
    Wiki {
        #[command(subcommand)]
        action: WikiCmd,
    },
    /// Work with the project's git host.
    Git {
        #[command(subcommand)]
        action: GitCmd,
    },
    /// Work with the project's pull/merge requests.
    Pr {
        #[command(subcommand)]
        action: PrCmd,
    },
    /// Work with the project's design source.
    Design {
        #[command(subcommand)]
        action: DesignCmd,
    },
    /// Work with the project's CI.
    Ci {
        #[command(subcommand)]
        action: CiCmd,
    },
    /// Send a message to the project's chat: `notify "build failed"`.
    Notify {
        /// The message text to send.
        message: String,
    },
    /// Personal corpus of fetched tickets (`~/.palugada/personal/`).
    Prd {
        #[command(subcommand)]
        action: PrdCmd,
    },
    /// Launch the local authoring console in a browser.
    Web {
        /// Port to bind on 127.0.0.1.
        #[arg(long, default_value_t = 7777)]
        port: u16,
        /// Open the console in your browser.
        #[arg(long)]
        open: bool,
    },
    /// Check tool + connector readiness for the current repo.
    Doctor {
        /// Emit JSON.
        #[arg(long)]
        json: bool,
    },
    /// Run a profile/project-defined exec verb: `exec <verb> [k=v ...]`.
    Exec {
        /// Verb to run (e.g. build, test, run). Omit with --list.
        verb: Option<String>,
        /// Placeholder values, e.g. `apk=app/build/outputs/apk/debug/app.apk`.
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
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Create skeleton ~/.palugada.yaml + ~/.palugada/secrets.yaml.
    Init,
    /// Print config and masked credentials.
    Show,
    /// Test connectivity + auth for the active project's providers.
    Verify,
    /// Manage named auth profiles (per-client credential sets).
    Auth {
        #[command(subcommand)]
        action: AuthCmd,
    },
}

#[derive(Subcommand)]
enum AuthCmd {
    /// List auth-profile names.
    List,
    /// Create an empty auth profile (add tokens via `palugada web`).
    Add { name: String },
    /// Delete an auth profile (blocked if a project still uses it).
    Rm { name: String },
    /// Show a profile's masked secrets.
    Show { name: String },
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Register a project: `palugada project add <name> <repo_path>`.
    Add { name: String, repo_path: String },
    /// List registered projects.
    List,
    /// Set the active project.
    Use { name: String },
    /// Show the effective rules (profile + per-project overlay) for a project.
    Rules { name: String },
    /// Remove a project from the registry (files on disk are untouched).
    Remove { name: String },
}

#[derive(Subcommand)]
enum SkillsCmd {
    /// Write any missing skill files for the active (or `--project`) project.
    Sync {
        /// Comma-separated agent targets, or `auto` to detect existing guide files.
        #[arg(long, default_value = "auto")]
        agents: String,
        /// Overwrite existing skill files instead of skipping them.
        #[arg(long)]
        force: bool,
    },
    /// Scaffold a custom skill in a profile: `skills new <name> [--profile <id>]`.
    New {
        name: String,
        /// Profile to add the skill to (default: the active project's profile).
        #[arg(long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum ProfileCmd {
    /// List bundled + locally-authored profiles.
    List,
    /// Lint a profile against the schema: `profile validate <id>`.
    Validate { id: String },
    /// Scaffold a new profile from a minimal template: `profile new <id>`.
    New {
        id: String,
        /// Inherit conventions/recipes from a base profile: `--extends <id>`.
        #[arg(long)]
        extends: Option<String>,
    },
    /// Bind the active (or `--project`) project to a profile: `profile use <id>`.
    Use { id: String },
}

#[derive(Subcommand)]
enum ConventionCmd {
    /// Import a markdown file as a convention: `convention add <file.md>`.
    /// Writes to the profile, or to a project's overlay with global `--project`.
    Add {
        /// Path to the markdown file (front-matter + `# Title` + `## Section`s).
        file: String,
        /// Profile override (ignored when `--project` selects an overlay).
        #[arg(long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum RecipeCmd {
    /// Import a markdown file as a recipe: `recipe add <file.md>`.
    Add {
        /// Path to the markdown file (front-matter + body).
        file: String,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum IssueCmd {
    /// View an issue by key: `palugada issue view PROJ-123`.
    View { key: String },
}

#[derive(Subcommand)]
enum WikiCmd {
    /// Fetch a page by id: `palugada wiki page 12345`.
    Page { id: String },
}

#[derive(Subcommand)]
enum GitCmd {
    /// Show the authenticated git-host user (connectivity check).
    Whoami,
}

#[derive(Subcommand)]
enum PrCmd {
    /// Recent commits touching a file, from the git host: `pr recent <file>`.
    Recent { file: String },
}

#[derive(Subcommand)]
enum PrdCmd {
    /// Fetch an issue into the corpus: `prd fetch PROJ-123`.
    Fetch { key: String },
    /// List saved corpus docs.
    List,
    /// Print a saved corpus doc: `prd cat PROJ-123`.
    Cat { name: String },
    /// Keyword search across the corpus: `prd search <kw>`.
    Search { query: String },
}

#[derive(Subcommand)]
enum DesignCmd {
    /// Fetch a design file's metadata: `palugada design file <FILE_KEY>`.
    File { key: String },
}

#[derive(Subcommand)]
enum CiCmd {
    /// Show a job's last build status: `palugada ci status <JOB>`.
    Status { job: String },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let have_home = ["HOME", "USERPROFILE"]
        .iter()
        .any(|v| std::env::var(v).map(|h| !h.is_empty()).unwrap_or(false));
    if !have_home {
        return Err("neither HOME nor USERPROFILE is set — palugada needs one to locate ~/.palugada.yaml and ~/.palugada/secrets.yaml".into());
    }
    if cli.insecure {
        eprintln!("warning: --insecure accepts ANY TLS certificate for every host this run");
    }
    let project = cli.project.as_deref();
    match cli.command {
        Commands::Init { repo, name, profile, auth, agents, force } => {
            cmd_init(repo, name, profile, auth, agents, force)
        }
        Commands::Config { action } => cmd_config(action, project, cli.insecure),
        Commands::Query { topic, brief, list, profile } => cmd_query(topic, brief, list, profile, project),
        Commands::ForTask { task, list, profile } => cmd_for(task, list, profile, project),
        Commands::Search { query, profile } => cmd_search(query, profile, project),
        Commands::Index { repo, profile } => cmd_index(repo, profile, project),
        Commands::Symbol { query, kind, repo } => cmd_symbol(query, kind, repo, project),
        Commands::Fact { family, name, profile } => cmd_fact(family, name, profile, project),
        Commands::Brief { flow, target, budget, json, profile } => {
            cmd_brief(flow, target, budget, json, profile, project, cli.insecure)
        }
        Commands::Project { action } => cmd_project(action),
        Commands::Profile { action } => cmd_profile(action, project),
        Commands::Convention { action } => cmd_convention(action, project),
        Commands::Recipe { action } => cmd_recipe(action, project),
        Commands::Skills { action } => cmd_skills(action, project),
        Commands::Issue { action } => cmd_issue(action, project, cli.insecure),
        Commands::Wiki { action } => cmd_wiki(action, project, cli.insecure),
        Commands::Git { action } => cmd_git(action, project, cli.insecure),
        Commands::Pr { action } => cmd_pr(action, project, cli.insecure),
        Commands::Design { action } => cmd_design(action, project, cli.insecure),
        Commands::Ci { action } => cmd_ci(action, project, cli.insecure),
        Commands::Notify { message } => cmd_notify(message, project, cli.insecure),
        Commands::Prd { action } => cmd_prd(action, project, cli.insecure),
        Commands::Web { port, open } => web::run(port, open),
        Commands::Doctor { json } => cmd_doctor(json, project, cli.insecure),
        Commands::Exec { verb, args, list, json, profile } => {
            cmd_exec(verb, args, list, json, profile, project)
        }
    }
}

// ── init ─────────────────────────────────────────────────────────────────

fn cmd_init(
    repo: String,
    name: Option<String>,
    profile: Option<String>,
    auth: Option<String>,
    agents: String,
    force: bool,
) -> Result<(), String> {
    let agents: Vec<String> = agents
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    scaffold::run(scaffold::InitOptions { repo, name, profile, auth, agents, force })
}

// ── knowledge: q / for / s ─────────────────────────────────────────────────

fn cmd_query(
    topic: Option<String>,
    brief: bool,
    list: bool,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    if list {
        return knowledge::list_topics(&kn, &prof);
    }
    let topic = topic.ok_or("specify a topic (e.g. `palugada q architecture`) or use --list")?;
    knowledge::query(&kn, &prof, &topic, brief)
}

fn cmd_for(
    task: Option<String>,
    list: bool,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    if list {
        return knowledge::list_recipes(&kn, &prof);
    }
    let task = task.ok_or("specify a recipe (e.g. `palugada for feature`) or use --list")?;
    knowledge::recipe(&kn, &prof, &task)
}

fn cmd_search(query: String, profile: Option<String>, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    knowledge::search(&kn, &prof, &query)
}

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

// ── index: indexer + symbol lookup ─────────────────────────────────────────

fn cmd_index(repo: Option<String>, profile: Option<String>, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, repo, &cwd)?;
    indexer::run(&repo_path, &kn, &prof)
}

fn cmd_symbol(
    query: String,
    kind: Option<String>,
    repo: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, repo, &cwd)?;
    indexer::symbol_search(&repo_path, &query, kind.as_deref())
}

fn cmd_fact(
    family: String,
    name: Option<String>,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    let report = indexer::fact_report(&repo, &kn, &prof, &family, name.as_deref())?;
    println!("{}", report.trim_end());
    Ok(())
}

// ── brief: flow context packs ──────────────────────────────────────────────

fn cmd_brief(
    flow: String,
    target: String,
    budget: usize,
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
    // Best-effort: brief works without a project (local steps); only prd.context needs this.
    let connectors = Secrets::load_or_default()
        .ok()
        .and_then(|s| config::resolve_project(&global, &s, project).ok())
        .map(|(_n, pc, auth)| brief::BriefConnectors { pc, auth, insecure });
    brief::run(&kn, &repo, &prof, &brief::BriefOptions { flow, target, budget, json }, connectors.as_ref())
}

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

// ── doctor ────────────────────────────────────────────────────────────────

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
                let code = exec::run_one_captured(
                    &cmd_str,
                    &repo,
                    std::time::Duration::from_secs(60),
                    &mut buf,
                );
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
            if pc.integrations.chat.is_some() {
                conns.push(("chat", clients::chat_notify(&pc, &auth, insecure).and_then(|c| c.verify())));
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
            println!(
                "  [{}] {:<9} {} — {}",
                if c.ok { "PASS" } else { "FAIL" },
                c.kind,
                c.name,
                c.detail
            );
        }
    }
    if failed > 0 {
        return Err(format!("{failed} check(s) failed"));
    }
    Ok(())
}

// ── config ───────────────────────────────────────────────────────────────

fn cmd_config(action: ConfigCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        ConfigCmd::Init => {
            let mut global = GlobalConfig::load_or_default()?;
            if global.engine.knowledge_path.is_empty() {
                if let Some(kn) = knowledge::detect_knowledge_dir() {
                    global.engine.knowledge_path = kn.to_string_lossy().to_string();
                }
            }
            global.save()?;
            let mut secrets = Secrets::load_or_default()?;
            secrets.auth_profiles.entry("default".to_string()).or_default();
            secrets.save()?;
            println!(
                "Wrote {} and {} (secrets chmod 0600).",
                GlobalConfig::default_path().display(),
                Secrets::default_path().display()
            );
            println!(
                "Next:\n  1. add your tokens under auth_profiles in {}\n  2. `palugada project add <name> <repo_path>`\n  3. `palugada config verify`",
                Secrets::default_path().display()
            );
            Ok(())
        }
        ConfigCmd::Show => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let yaml = serde_yaml::to_string(&global).map_err(|e| e.to_string())?;
            println!("# {}", GlobalConfig::default_path().display());
            println!("{yaml}");
            println!("# auth-profiles (masked):");
            if secrets.auth_profiles.is_empty() {
                println!("  (none — run `palugada config init`)");
            }
            for (name, a) in &secrets.auth_profiles {
                println!("  {name}:");
                println!("    jira_token:    {}", mask_secret(&a.jira_token));
                println!("    jira_email:    {}", if a.jira_email.is_empty() { "(unset)".into() } else { a.jira_email.clone() });
                println!("    wiki_token:    {}", mask_secret(&a.wiki_token));
                println!("    wiki_email:    {}", if a.wiki_email.is_empty() { "(unset)".into() } else { a.wiki_email.clone() });
                println!("    figma_token:   {}", mask_secret(&a.figma_token));
                println!("    git_token:     {}", mask_secret(&a.git_token));
                println!("    jenkins_token: {}", mask_secret(&a.jenkins_token));
                println!("    jenkins_user:  {}", if a.jenkins_user.is_empty() { "(unset)".into() } else { a.jenkins_user.clone() });
                println!("    chat_webhook:  {}", mask_secret(&a.chat_webhook));
            }
            Ok(())
        }
        ConfigCmd::Verify => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let prof = if pc.profile.is_empty() { "—" } else { pc.profile.as_str() };
            let ap = if pc.auth_profile.is_empty() { "—" } else { pc.auth_profile.as_str() };
            println!("Verifying project '{name}' (profile: {prof}, auth: {ap})");

            if pc.integrations.issue_tracker.is_some() {
                report("issue", clients::issue_tracker(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            if pc.integrations.wiki.is_some() {
                report("wiki", clients::doc_source(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            if pc.integrations.git_host.is_some() {
                report("git", clients::git_host(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            if pc.integrations.design.is_some() {
                report("design", clients::design_source(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            if pc.integrations.ci.is_some() {
                report("ci", clients::ci_provider(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            if pc.integrations.chat.is_some() {
                report("chat", clients::chat_notify(&pc, &auth, insecure).and_then(|c| c.verify()));
            }
            Ok(())
        }
        ConfigCmd::Auth { action } => cmd_config_auth(action),
    }
}

fn report(tag: &str, result: Result<String, String>) {
    match result {
        Ok(msg) => println!("  [{tag}] {msg}"),
        Err(e) => println!("  [{tag}] FAIL: {e}"),
    }
}

fn cmd_config_auth(action: AuthCmd) -> Result<(), String> {
    match action {
        AuthCmd::List => {
            let secrets = Secrets::load_or_default()?;
            let names = secrets.list_auth_profiles();
            if names.is_empty() {
                println!("(no auth profiles — add one with `palugada config auth add <name>`)");
            } else {
                for n in names {
                    println!("{n}");
                }
            }
            Ok(())
        }
        AuthCmd::Add { name } => {
            let mut secrets = Secrets::load_or_default()?;
            secrets.add_auth_profile(&name)?;
            secrets.save()?;
            println!(
                "created auth profile '{}' — add tokens via `palugada web` (Connectors → profile switcher)",
                name.trim()
            );
            Ok(())
        }
        AuthCmd::Rm { name } => {
            credentials::delete_auth_profile(&name)?;
            println!("deleted auth profile '{name}'");
            Ok(())
        }
        AuthCmd::Show { name } => {
            let secrets = Secrets::load_or_default()?;
            let a = secrets
                .auth_profiles
                .get(&name)
                .ok_or_else(|| format!("auth profile '{name}' not found"))?;
            let or_unset = |s: &str| if s.is_empty() { "(unset)".to_string() } else { s.to_string() };
            println!("auth profile '{name}' (masked):");
            println!("  jira_token:    {}", mask_secret(&a.jira_token));
            println!("  jira_email:    {}", or_unset(&a.jira_email));
            println!("  wiki_token:    {}", mask_secret(&a.wiki_token));
            println!("  wiki_email:    {}", or_unset(&a.wiki_email));
            println!("  figma_token:   {}", mask_secret(&a.figma_token));
            println!("  git_token:     {}", mask_secret(&a.git_token));
            println!("  jenkins_token: {}", mask_secret(&a.jenkins_token));
            println!("  jenkins_user:  {}", or_unset(&a.jenkins_user));
            println!("  chat_webhook:  {}", mask_secret(&a.chat_webhook));
            Ok(())
        }
    }
}

// ── project registry ───────────────────────────────────────────────────────

fn cmd_project(action: ProjectCmd) -> Result<(), String> {
    match action {
        ProjectCmd::Add { name, repo_path } => {
            let mut global = GlobalConfig::load_or_default()?;
            let repo = std::fs::canonicalize(config::expand_home(&repo_path))
                .map_err(|e| format!("repo path not found ({repo_path}): {e}"))?;
            if !repo.is_dir() {
                return Err(format!("not a directory: {}", repo.display()));
            }
            let repo = repo.to_string_lossy().to_string();
            let default_workspace = format!("{repo}/.palugada");
            let workspace = if let Some(existing) = global.projects.registered.get(&name) {
                if existing.repo_path != repo {
                    eprintln!(
                        "warning: project '{name}' was registered at {} — overwriting with {repo}",
                        existing.repo_path
                    );
                }
                // Preserve a customised workspace; only reset if it was the default for the OLD repo
                let old_default = format!("{}/.palugada", existing.repo_path);
                if existing.workspace.is_empty() || existing.workspace == old_default {
                    default_workspace
                } else {
                    existing.workspace.clone()
                }
            } else {
                default_workspace
            };
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
        ProjectCmd::List => {
            let global = GlobalConfig::load_or_default()?;
            if global.projects.registered.is_empty() {
                println!("No projects registered. Use `palugada project add <name> <repo_path>`.");
                return Ok(());
            }
            for (name, e) in &global.projects.registered {
                let marker = if *name == global.projects.active { "*" } else { " " };
                let prof = config::ProjectConfig::load_from(&e.repo_path)
                    .map(|c| c.profile)
                    .unwrap_or_default();
                let prof = if prof.is_empty() { "—".to_string() } else { prof };
                println!("{marker} {name}  profile={prof}  ->  {}", e.repo_path);
            }
            println!("\n(* = active)");
            Ok(())
        }
        ProjectCmd::Use { name } => {
            let mut global = GlobalConfig::load_or_default()?;
            if !global.projects.registered.contains_key(&name) {
                return Err(format!("project '{name}' is not registered"));
            }
            global.projects.active = name.clone();
            global.save()?;
            println!("Active project is now '{name}'.");
            Ok(())
        }
        ProjectCmd::Rules { name } => {
            let global = GlobalConfig::load_or_default()?;
            let eff = effective::effective_rules(&global, &name)?;
            println!("Effective rules for '{}' (profile: {})\n", eff.project, eff.profile);
            println!("Conventions:");
            for c in &eff.conventions {
                let tag = match c.origin {
                    effective::Origin::Profile => "[profile]",
                    effective::Origin::Project => "[project]",
                    effective::Origin::Overridden => "[overridden]",
                };
                println!("  {:<12} {:<16} {}", tag, c.id, c.description);
            }
            println!("\nreview_map:");
            for e in &eff.review_map {
                let tag = if e.origin == effective::Origin::Project { "[project]" } else { "[profile]" };
                println!("  {:<10} {} -> {}", tag, e.family, e.conventions.join(", "));
            }
            for w in &eff.warnings {
                eprintln!("warning: {w}");
            }
            Ok(())
        }
    }
}

fn cmd_profile(action: ProfileCmd, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    match action {
        ProfileCmd::List => {
            let profs = profile::list(&kn)?;
            if profs.is_empty() {
                println!("(no profiles in {})", kn.join("profiles").display());
            }
            for (id, title) in profs {
                println!("  {:<16} {}", id, title);
            }
            Ok(())
        }
        ProfileCmd::Validate { id } => {
            let checks = profile::validate(&kn, &id);
            let mut failed = 0;
            for c in &checks {
                if !c.ok {
                    failed += 1;
                }
                println!("[{}] {:<18} {}", if c.ok { "ok  " } else { "FAIL" }, c.name, c.detail);
            }
            if failed > 0 {
                Err(format!("{failed} check(s) failed for profile '{id}'"))
            } else {
                println!("profile '{id}' OK");
                Ok(())
            }
        }
        ProfileCmd::New { id, extends } => {
            let written = profile::scaffold_new(&kn, &id, extends.as_deref())?;
            println!("scaffolded profile '{id}':");
            for p in written {
                println!("  {}", p.display());
            }
            if let Some(parent) = &extends {
                println!("inherits conventions/recipes from '{parent}' (author only what differs)");
            }
            println!("validate with:  palugada profile validate {id}");
            Ok(())
        }
        ProfileCmd::Use { id } => {
            let profs = profile::list(&kn)?;
            if !profs.iter().any(|(pid, _)| pid == &id) {
                return Err(format!(
                    "unknown profile '{id}' (available: {})",
                    profs.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
            let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
            let name = config::resolve_project_name(&global, project, &cwd)?;
            let entry = global
                .projects
                .registered
                .get(&name)
                .ok_or_else(|| format!("project '{name}' is not registered"))?;
            config::set_profile(&entry.repo_path, &id)?;
            println!("project '{name}' now uses profile '{id}'");
            println!("knowledge & symbols already follow it; run `palugada index` only if this profile adds new fact families.");
            Ok(())
        }
    }
}

fn cmd_convention(action: ConventionCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        ConventionCmd::Add { file, profile } => {
            if !file.ends_with(".md") {
                return Err(format!("expected a .md file, got '{file}'"));
            }
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let dir = if let Some(name) = project {
                // Per-project convention overlay (cycle C).
                let entry = global
                    .projects
                    .registered
                    .get(name)
                    .ok_or_else(|| format!("project '{name}' is not registered"))?;
                effective::overlay_dir(&entry.repo_path)
            } else {
                let prof = resolve_profile(&global, None, profile.as_deref(), &kn)?;
                kn.join("profiles").join(&prof).join("conventions")
            };
            let (id, replaced) = knowledge::add_convention_from_markdown(&dir, &raw)?;
            let verb = if replaced { "updated" } else { "created" };
            println!("{verb} {id} -> {}", dir.join(format!("{id}.md")).display());
            Ok(())
        }
    }
}

fn cmd_recipe(action: RecipeCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        RecipeCmd::Add { file, profile } => {
            if project.is_some() {
                return Err("recipes are profile-scoped; drop --project and use --profile".to_string());
            }
            if !file.ends_with(".md") {
                return Err(format!("expected a .md file, got '{file}'"));
            }
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let prof = resolve_profile(&global, None, profile.as_deref(), &kn)?;
            let dir = kn.join("profiles").join(&prof).join("recipes");
            let (id, replaced) = knowledge::add_recipe_from_markdown(&dir, &raw)?;
            let verb = if replaced { "updated" } else { "created" };
            println!("{verb} {id} -> {}", dir.join(format!("{id}.md")).display());
            Ok(())
        }
    }
}

fn cmd_skills(action: SkillsCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        SkillsCmd::New { name, profile } => {
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
            let p = scaffold::new_custom_skill(&kn, &prof, &name)?;
            println!("created custom skill '{name}' for profile '{prof}':");
            println!("  {}", p.display());
            println!("edit it, then `palugada skills sync` into a project bound to '{prof}'.");
            Ok(())
        }
        SkillsCmd::Sync { agents, force } => {
            let global = GlobalConfig::load_or_default()?;
            let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
            let name = config::resolve_project_name(&global, project, &cwd)?;
            let entry = global
                .projects
                .registered
                .get(&name)
                .ok_or_else(|| format!("project '{name}' is not registered"))?;
            let pc = config::ProjectConfig::load_from(&entry.repo_path)?;
            let kinds = scaffold::integration_kinds(&pc);
            let agents: Vec<String> = if agents.trim() == "auto" {
                scaffold::detect_agents(std::path::Path::new(&entry.repo_path))
            } else {
                agents.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
            };
            let repo = std::path::Path::new(&entry.repo_path);
            let mut files = scaffold::skill_files(&pc.profile, &kinds, &agents);
            let kn = knowledge::knowledge_dir(&global)?;
            if agents.iter().any(|a| a == "claude") {
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile, ".claude/skills"));
            }
            if agents.iter().any(|a| a == "codex") {
                files.extend(scaffold::custom_skill_files(&kn, &pc.profile, ".agents/skills"));
            }
            let (mut written, mut skipped, mut merged) = (Vec::new(), Vec::new(), Vec::new());
            for (rel, body) in files {
                scaffold::write_agent_file(&repo.join(&rel), &body, force, &mut written, &mut skipped, &mut merged)?;
            }
            for w in &written {
                println!("  wrote    {w}");
            }
            for m in &merged {
                println!("  merged   {m}  (palugada section)");
            }
            for s in &skipped {
                println!("  skipped  {s}  (exists)");
            }
            println!(
                "skills sync — '{name}' (profile {}): {} wrote, {} merged, {} skipped",
                pc.profile,
                written.len(),
                merged.len(),
                skipped.len()
            );
            if !skipped.is_empty() && !force {
                println!("  use --force to overwrite existing palugada-owned files");
            }
            Ok(())
        }
    }
}

// ── issue / wiki / git ───────────────────────────────────────────────────

fn cmd_issue(action: IssueCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        IssueCmd::View { key } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let tracker = clients::issue_tracker(&pc, &auth, insecure)?;
            let i = tracker.get_issue(&key)?;
            println!("{} — {}", i.key, i.summary);
            println!("Status: {}   Type: {}   Assignee: {}", i.status, i.issue_type, i.assignee);
            if !i.description.is_empty() {
                println!("\n{}", i.description);
            }
            Ok(())
        }
    }
}

fn cmd_wiki(action: WikiCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        WikiCmd::Page { id } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let docs = clients::doc_source(&pc, &auth, insecure)?;
            let page = docs.get_page(&id)?;
            println!("# {} (id {})", page.title, page.id);
            println!("\n{}", page.body_html);
            Ok(())
        }
    }
}

fn cmd_git(action: GitCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        GitCmd::Whoami => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let git = clients::git_host(&pc, &auth, insecure)?;
            let u = git.whoami()?;
            println!("{} ({}) @ {}", u.username, u.name, u.host);
            Ok(())
        }
    }
}

fn cmd_pr(action: PrCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        PrCmd::Recent { file } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let git = clients::git_host(&pc, &auth, insecure)?;
            let commits = git.recent_commits(&file, 10)?;
            if commits.is_empty() {
                println!("(no recent commits touching {file})");
                return Ok(());
            }
            for c in &commits {
                let short = &c.sha[..c.sha.len().min(8)];
                println!("{short}  {}  ({})", c.title, c.author);
                if !c.url.is_empty() {
                    println!("        {}", c.url);
                }
            }
            Ok(())
        }
    }
}

fn cmd_prd(action: PrdCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    let dir = personal::dir();
    match action {
        PrdCmd::Fetch { key } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let tracker = clients::issue_tracker(&pc, &auth, insecure)?;
            let issue = tracker.get_issue(&key)?;
            let ts = chrono::Utc::now().to_rfc3339();
            let path = personal::save_issue(&dir, &issue, &ts)?;
            println!("saved {} -> {}", issue.key, path.display());
            Ok(())
        }
        PrdCmd::List => {
            let names = personal::list(&dir)?;
            if names.is_empty() {
                println!("(corpus empty — {})", dir.display());
            }
            for n in names {
                println!("  {n}");
            }
            Ok(())
        }
        PrdCmd::Cat { name } => {
            println!("{}", personal::cat(&dir, &name)?.trim_end());
            Ok(())
        }
        PrdCmd::Search { query } => {
            let hits = personal::search(&dir, &query)?;
            if hits.is_empty() {
                println!("(no corpus matches '{query}')");
            }
            for (name, line) in hits {
                println!("{:<20} {}", name, line);
            }
            Ok(())
        }
    }
}

fn cmd_design(action: DesignCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        DesignCmd::File { key } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let design = clients::design_source(&pc, &auth, insecure)?;
            let f = design.get_file(&key)?;
            println!("{} (key {})", f.name, f.key);
            println!("version: {}   last modified: {}", f.version, f.last_modified);
            Ok(())
        }
    }
}

fn cmd_ci(action: CiCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        CiCmd::Status { job } => {
            let global = GlobalConfig::load_or_default()?;
            let secrets = Secrets::load_or_default()?;
            let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
            let ci = clients::ci_provider(&pc, &auth, insecure)?;
            let b = ci.job_status(&job)?;
            let state = if b.building { "building".to_string() } else { b.result.clone() };
            println!("{} #{} — {}", b.job, b.number, state);
            Ok(())
        }
    }
}

fn cmd_notify(message: String, project: Option<&str>, insecure: bool) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let secrets = Secrets::load_or_default()?;
    let (_name, pc, auth) = resolve_project(&global, &secrets, project)?;
    let chat = clients::chat_notify(&pc, &auth, insecure)?;
    let status = chat.notify(&message)?;
    println!("notify: {status}");
    Ok(())
}
