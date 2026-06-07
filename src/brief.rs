//! `palugada brief <flow> <target>` — assemble one budgeted context pack.
//!
//! Reads the flow's step list from the bound profile's `profile.yaml`, runs the
//! steps it can (conventions, recipes, indexed symbols, recent commits), and
//! gracefully stubs the ones not built yet (prd.context, module.info, diff.scan).

use crate::{indexer, knowledge};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;

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
}

#[derive(Serialize)]
struct Pack {
    step: String,
    title: String,
    content: String,
}

pub fn run(kn: &Path, repo: &Path, profile: &str, opts: &BriefOptions) -> Result<(), String> {
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
            other => (
                other.to_string(),
                format!("(step '{step}' not yet available in this build)"),
            ),
        };
        packs.push(Pack { step: step.clone(), title, content });
    }

    if opts.json {
        let data = serde_json::to_string_pretty(&packs).map_err(|e| e.to_string())?;
        println!("{data}");
        return Ok(());
    }

    let target = if opts.target.is_empty() { "(no target)" } else { opts.target.as_str() };
    println!("# brief {}: {}", opts.flow, target);
    println!("profile: {profile}   budget: ~{} tokens\n", opts.budget);

    let mut used: usize = 0;
    for p in &packs {
        let cost = p.content.len() / 4 + 8;
        if used > 0 && used + cost > opts.budget {
            println!("## {}\n(omitted — over budget; run the step directly)\n", p.title);
            continue;
        }
        println!("## {}\n{}\n", p.title, p.content.trim());
        used += cost;
    }
    println!("(~{used} tokens)");
    Ok(())
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
