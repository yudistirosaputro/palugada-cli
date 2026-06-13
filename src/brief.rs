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
    #[serde(skip)]
    kind: String,
    title: String,
    content: String,
    #[serde(skip)]
    rerun: String,
}

enum Render {
    Full,
    Truncated { kept: String, dropped: usize },
    Omitted,
}

fn est_tokens(s: &str) -> usize {
    s.len() / 4 + 8
}

/// Higher = more valuable, kept first under a tight budget.
fn priority(kind: &str) -> u8 {
    match kind {
        "prd.context" => 5,
        "symbol.find" | "module.info" | "diff.scan" => 4,
        "convention" => 3,
        "recipe" => 2,
        "code.recent" => 1,
        _ => 0,
    }
}

/// Keep whole lines while the running token estimate stays within `max_tokens`;
/// always keep at least the first line. Returns (kept_text, dropped_line_count).
fn truncate_to_tokens(content: &str, max_tokens: usize) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0usize;
    for line in &lines {
        let cost = line.len() / 4 + 1;
        if !kept.is_empty() && used + cost > max_tokens {
            break;
        }
        kept.push(line);
        used += cost;
    }
    (kept.join("\n"), lines.len() - kept.len())
}

/// Decide each pack's fate by descending priority: full while it fits, then
/// truncate the one that overflows, then omit the rest. The top-priority pack
/// is always included (truncated if it alone exceeds the budget).
fn budget_packs(packs: &[Pack], budget: usize) -> Vec<Render> {
    let mut order: Vec<usize> = (0..packs.len()).collect();
    order.sort_by(|&a, &b| priority(&packs[b].kind).cmp(&priority(&packs[a].kind)).then(a.cmp(&b)));
    let mut renders: Vec<Render> = packs.iter().map(|_| Render::Omitted).collect();
    let mut used = 0usize;
    for (rank, &i) in order.iter().enumerate() {
        let cost = est_tokens(&packs[i].content);
        if used + cost <= budget {
            renders[i] = Render::Full;
            used += cost;
        } else {
            let remaining = budget.saturating_sub(used);
            if remaining > 0 || rank == 0 {
                let (kept, dropped) = truncate_to_tokens(&packs[i].content, remaining);
                renders[i] = Render::Truncated { kept, dropped };
                used = budget;
            }
        }
    }
    renders
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
        packs.push(Pack {
            step: step.clone(),
            kind: kind.clone(),
            title,
            content,
            rerun: rerun_hint(&kind, &arg, &opts.target),
        });
    }

    if opts.json {
        let data = serde_json::to_string_pretty(&packs).map_err(|e| e.to_string())?;
        println!("{data}");
        return Ok(());
    }

    let target = if opts.target.is_empty() { "(no target)" } else { opts.target.as_str() };
    println!("# brief {}: {}", opts.flow, target);
    println!("profile: {profile}   budget: ~{} tokens\n", opts.budget);

    let renders = budget_packs(&packs, opts.budget);
    let mut used = 0usize;
    for (p, r) in packs.iter().zip(&renders) {
        match r {
            Render::Full => {
                println!("## {}\n{}\n", p.title, p.content.trim());
                used += est_tokens(&p.content);
            }
            Render::Truncated { kept, dropped } => {
                println!("## {}\n{}", p.title, kept.trim());
                println!("(+{dropped} lines truncated — run `{}` for the rest)\n", p.rerun);
                used += est_tokens(kept);
            }
            Render::Omitted => {
                println!("## {}\n(omitted — over budget; run `{}`)\n", p.title, p.rerun);
            }
        }
    }
    println!("(~{used} tokens)");
    Ok(())
}

/// The command an agent runs to get a step's full content (shown when truncated/omitted).
fn rerun_hint(kind: &str, arg: &str, target: &str) -> String {
    match kind {
        "convention" => format!("palugada q {arg}"),
        "recipe" => format!("palugada for {arg}"),
        "symbol.find" => format!("palugada symbol {target}"),
        "module.info" => "palugada index".to_string(),
        "diff.scan" => format!("git diff {}", if target.is_empty() { "HEAD" } else { target }),
        "prd.context" => format!("palugada issue view {target}"),
        "code.recent" => format!("git log -- {target}"),
        _ => "palugada q --list".to_string(),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(kind: &str, content: &str) -> Pack {
        Pack { step: kind.into(), kind: kind.into(), title: kind.into(), content: content.into(), rerun: "x".into() }
    }

    #[test]
    fn truncate_keeps_at_least_one_line_and_counts_dropped() {
        let (kept, dropped) = truncate_to_tokens("a\nb\nc\nd", 0);
        assert_eq!(kept, "a");
        assert_eq!(dropped, 3);
    }

    #[test]
    fn budget_prefers_high_priority_and_omits_low() {
        // 128 chars → est_tokens = 128/4 + 8 = 40, so one pack fills budget 40 exactly,
        // leaving 0 for the next → the low-priority pack is omitted, not truncated.
        let big = "x".repeat(128);
        assert_eq!(est_tokens(&big), 40);
        let packs = vec![pack("code.recent", &big), pack("prd.context", &big)];
        let r = budget_packs(&packs, 40);
        // prd.context (priority 5) kept full; code.recent (priority 1) omitted.
        assert!(matches!(r[1], Render::Full));
        assert!(matches!(r[0], Render::Omitted));
    }

    #[test]
    fn top_priority_pack_never_omitted_even_over_budget() {
        let huge = "y".repeat(10_000);
        let packs = vec![pack("prd.context", &huge)];
        let r = budget_packs(&packs, 10);
        assert!(matches!(r[0], Render::Truncated { .. }));
    }
}
