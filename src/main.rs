//! palugada — project-agnostic dev knowledge & connector CLI (base).
//!
//! This first slice wires the connector layer: config + credentials, a project
//! registry, and Jira / Confluence / Git host commands. Knowledge commands
//! (`q`, `for`, `brief`, the indexer) come on top of this base.

mod brief;
mod clients;
mod config;
mod exec;
mod http;
mod indexer;
mod knowledge;
mod scaffold;

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
        #[arg(long, default_value = "claude")]
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
        /// Topic id, optionally `.N` for one section (e.g. `architecture.2`).
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
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Create skeleton ~/.palugada.yaml + ~/.palugada/secrets.yaml.
    Init,
    /// Print config and masked credentials.
    Show,
    /// Test connectivity + auth for the active project's providers.
    Verify,
}

#[derive(Subcommand)]
enum ProjectCmd {
    /// Register a project: `palugada project add <name> <repo_path>`.
    Add { name: String, repo_path: String },
    /// List registered projects.
    List,
    /// Set the active project.
    Use { name: String },
    /// Remove a project from the registry (files on disk are untouched).
    Remove { name: String },
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
    if std::env::var("HOME").map(|h| h.is_empty()).unwrap_or(true) {
        return Err("HOME is not set — palugada needs it to locate ~/.palugada.yaml and ~/.palugada/secrets.yaml".into());
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
        Commands::Symbol { query } => cmd_symbol(query, project),
        Commands::Brief { flow, target, budget, json, profile } => {
            cmd_brief(flow, target, budget, json, profile, project)
        }
        Commands::Project { action } => cmd_project(action),
        Commands::Issue { action } => cmd_issue(action, project, cli.insecure),
        Commands::Wiki { action } => cmd_wiki(action, project, cli.insecure),
        Commands::Git { action } => cmd_git(action, project, cli.insecure),
        Commands::Design { action } => cmd_design(action, project, cli.insecure),
        Commands::Ci { action } => cmd_ci(action, project, cli.insecure),
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

fn cmd_symbol(query: String, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, None, &cwd)?;
    indexer::symbol_search(&repo_path, &query)
}

// ── brief: flow context packs ──────────────────────────────────────────────

fn cmd_brief(
    flow: String,
    target: String,
    budget: usize,
    json: bool,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    brief::run(&kn, &repo, &prof, &brief::BriefOptions { flow, target, budget, json })
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
            Ok(())
        }
    }
}

fn report(tag: &str, result: Result<String, String>) {
    match result {
        Ok(msg) => println!("  [{tag}] {msg}"),
        Err(e) => println!("  [{tag}] FAIL: {e}"),
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
                println!("{marker} {name}  ->  {}", e.repo_path);
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
