//! palugada — project-agnostic dev knowledge & connector CLI (base).
//!
//! This first slice wires the connector layer: config + credentials, a project
//! registry, and Jira / Confluence / Git host commands. Knowledge commands
//! (`q`, `for`, `brief`, the indexer) come on top of this base.

mod clients;
mod config;
mod http;

use clap::{Parser, Subcommand};
use config::{
    mask_secret, resolve_project, GlobalConfig, ProjectEntry, Secrets,
};

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
    /// Manage global config and credentials.
    Config {
        #[command(subcommand)]
        action: ConfigCmd,
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

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    let project = cli.project.as_deref();
    match cli.command {
        Commands::Config { action } => cmd_config(action, project, cli.insecure),
        Commands::Project { action } => cmd_project(action),
        Commands::Issue { action } => cmd_issue(action, project, cli.insecure),
        Commands::Wiki { action } => cmd_wiki(action, project, cli.insecure),
        Commands::Git { action } => cmd_git(action, project, cli.insecure),
    }
}

// ── config ───────────────────────────────────────────────────────────────

fn cmd_config(action: ConfigCmd, project: Option<&str>, insecure: bool) -> Result<(), String> {
    match action {
        ConfigCmd::Init => {
            let global = GlobalConfig::load_or_default()?;
            global.save()?;
            let mut secrets = Secrets::load_or_default()?;
            secrets.auth_profiles.entry("default".to_string()).or_default();
            secrets.save()?;
            println!(
                "Wrote {} and {} (chmod 0600).",
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
                println!("    wiki_token:    {}", mask_secret(&a.wiki_token));
                println!("    figma_token:   {}", mask_secret(&a.figma_token));
                println!("    git_token:     {}", mask_secret(&a.git_token));
                println!("    jenkins_token: {}", mask_secret(&a.jenkins_token));
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
            let repo = repo_path.trim_end_matches('/').to_string();
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
