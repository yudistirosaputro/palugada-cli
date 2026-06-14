//! `palugada brief <flow> <target>` — assemble one budgeted context pack.
//!
//! Reads the flow's step list from the bound profile's `profile.yaml` and runs
//! each step: conventions, recipes, indexed symbols, recent commits,
//! `module.info`, `diff.scan` + `convention(by-file-kind)` (driven by the
//! profile's `review_map`), and `prd.context` (the only networked step — its
//! IssueTracker is built lazily and every failure degrades to an inline note).
//! A priority-fill budget keeps the highest-value steps and truncates the rest.

use crate::clients;
use crate::config::{AuthProfile, ProjectConfig};
use crate::{indexer, knowledge};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize, Default)]
struct ProfileMeta {
    #[serde(default)]
    flows: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    review_map: BTreeMap<String, Vec<String>>,
}

#[derive(Default)]
struct BriefContext {
    touched_families: BTreeSet<String>,
    diff_scanned: bool,
}

/// Group changed files by their fact family (unmatched → "(unclassified)") and
/// collect the set of families touched.
fn classify_files(
    files: &[String],
    families: &[indexer::CompiledFamily],
) -> (BTreeMap<String, Vec<String>>, BTreeSet<String>) {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut touched: BTreeSet<String> = BTreeSet::new();
    for f in files {
        let ext = Path::new(f).extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ids = indexer::families_for_path(f, &ext, families);
        if ids.is_empty() {
            groups.entry("(unclassified)".to_string()).or_default().push(f.clone());
        } else {
            for id in ids {
                touched.insert(id.clone());
                groups.entry(id).or_default().push(f.clone());
            }
        }
    }
    (groups, touched)
}

/// Deduped, sorted convention topics for every touched family present in the map.
fn mapped_topics(review_map: &BTreeMap<String, Vec<String>>, touched: &BTreeSet<String>) -> Vec<String> {
    let mut topics: BTreeSet<String> = BTreeSet::new();
    for fam in touched {
        if let Some(ts) = review_map.get(fam) {
            for t in ts {
                topics.insert(t.clone());
            }
        }
    }
    topics.into_iter().collect()
}

/// `git -C <repo> diff --name-only <ref>` → changed file paths.
fn git_changed_files(repo: &Path, gitref: &str) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C").arg(repo)
        .args(["diff", "--name-only", gitref])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !out.status.success() {
        return Err("git diff failed".to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .filter(|l| !l.is_empty())
        .collect())
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

pub struct BriefConnectors {
    pub pc: ProjectConfig,
    pub auth: AuthProfile,
    pub insecure: bool,
}

fn format_issue_pack(i: &clients::Issue) -> String {
    let excerpt: String = i.description.chars().take(600).collect();
    format!(
        "{} — {}\nStatus: {} · Type: {} · Assignee: {}\nSpec excerpt: {}",
        i.key, i.summary, i.status, i.issue_type, i.assignee, excerpt
    )
}

/// Fetch the target ticket via the project's IssueTracker. Every failure path
/// degrades to an inline `(…)` note so `brief` never aborts on the network.
fn prd_context_content(connectors: Option<&BriefConnectors>, target: &str) -> String {
    match connectors {
        None => "(no project/credentials resolved — run brief inside a registered project)".to_string(),
        Some(_) if target.is_empty() => "(no target ticket)".to_string(),
        Some(c) => match clients::issue_tracker(&c.pc, &c.auth, c.insecure) {
            Err(e) => format!("({e})"),
            Ok(tracker) => match tracker.get_issue(target) {
                Err(e) => format!("(could not fetch {target}: {e})"),
                Ok(i) => format_issue_pack(&i),
            },
        },
    }
}

pub fn run(
    kn: &Path,
    repo: &Path,
    profile: &str,
    opts: &BriefOptions,
    connectors: Option<&BriefConnectors>,
) -> Result<(), String> {
    let pf_path = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = fs::read_to_string(&pf_path).map_err(|e| format!("read {}: {e}", pf_path.display()))?;
    let pf: ProfileMeta =
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
    let mut ctx = BriefContext::default();
    for step in steps {
        let (kind, arg) = parse_step(step);
        let (title, content) = match kind.as_str() {
            "convention" if arg == "by-file-kind" => {
                let content = if !ctx.diff_scanned {
                    "(run diff.scan first — no touched families recorded)".to_string()
                } else if ctx.touched_families.is_empty() {
                    "(no fact-family files changed — nothing to check)".to_string()
                } else {
                    let topics = mapped_topics(&pf.review_map, &ctx.touched_families);
                    if topics.is_empty() {
                        "(no review_map entries for the touched families)".to_string()
                    } else {
                        topics
                            .iter()
                            .map(|t| {
                                format!(
                                    "### {t}\n{}",
                                    knowledge::convention_outline(kn, profile, t)
                                        .unwrap_or_else(|e| format!("({e})"))
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    }
                };
                ("conventions by file kind".to_string(), content)
            }
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
                indexer::symbol_report(repo, &opts.target, None).unwrap_or_else(|e| format!("({e})")),
            ),
            "code.recent" => (
                format!("recent commits for '{}'", opts.target),
                git_recent(repo, &opts.target),
            ),
            "module.info" => (
                format!("module info for '{}'", opts.target),
                indexer::module_report(repo, &opts.target),
            ),
            "prd.context" => (
                format!("ticket context for '{}'", opts.target),
                prd_context_content(connectors, &opts.target),
            ),
            "diff.scan" => {
                let gitref = if opts.target.is_empty() { "HEAD" } else { opts.target.as_str() };
                let content = match (indexer::load_families(kn, profile), git_changed_files(repo, gitref)) {
                    (Err(e), _) => format!("({e})"),
                    (_, Err(e)) => format!("({e})"),
                    (Ok(_), Ok(files)) if files.is_empty() => {
                        ctx.diff_scanned = true;
                        format!("(no changed files vs {gitref})")
                    }
                    (Ok((_ignore, families)), Ok(files)) => {
                        ctx.diff_scanned = true;
                        let (groups, touched) = classify_files(&files, &families);
                        ctx.touched_families = touched;
                        let mut s = String::new();
                        for (fam, fs) in &groups {
                            s.push_str(&format!("{fam}: {}\n", fs.join(", ")));
                        }
                        s
                    }
                };
                (format!("changed files vs {gitref}"), content)
            }
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

    #[test]
    fn mapped_topics_dedupes_across_touched_families() {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        map.insert("viewmodel".into(), vec!["architecture".into(), "testing".into()]);
        map.insert("route".into(), vec!["architecture".into()]);
        let mut touched = BTreeSet::new();
        touched.insert("viewmodel".to_string());
        touched.insert("route".to_string());
        let topics = mapped_topics(&map, &touched);
        assert_eq!(topics, vec!["architecture".to_string(), "testing".to_string()]);
    }

    #[test]
    fn prd_context_degrades_without_connectors() {
        assert!(prd_context_content(None, "PROJ-1").contains("no project"));
    }

    #[test]
    fn prd_context_notes_empty_target() {
        let c = BriefConnectors {
            pc: crate::config::ProjectConfig::default(),
            auth: crate::config::AuthProfile::default(),
            insecure: false,
        };
        assert!(prd_context_content(Some(&c), "").contains("no target ticket"));
    }

    #[test]
    fn prd_context_notes_missing_tracker() {
        // default ProjectConfig has no issue_tracker → factory errors, degraded to a note.
        let c = BriefConnectors {
            pc: crate::config::ProjectConfig::default(),
            auth: crate::config::AuthProfile::default(),
            insecure: false,
        };
        let out = prd_context_content(Some(&c), "PROJ-1");
        assert!(out.starts_with('(') && out.contains("issue_tracker"));
    }

    #[test]
    fn format_issue_pack_includes_key_and_excerpt() {
        let i = crate::clients::Issue {
            key: "T-1".into(), summary: "Add export".into(), status: "Open".into(),
            issue_type: "Story".into(), assignee: "me".into(), description: "spec body".into(),
        };
        let s = format_issue_pack(&i);
        assert!(s.contains("T-1 — Add export") && s.contains("spec body"));
    }

    #[test]
    fn classify_files_groups_and_collects_families() {
        let cfg: indexer::Extractors = serde_yaml::from_str(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'x'\n  - id: i18n\n    ext: [xml]\n    path_contains: values\n    regex: 'x'\n",
        ).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fams = indexer::compile_families(&cfg, dir.path()).unwrap();
        let files = vec!["a/Login.kt".to_string(), "a/values/strings.xml".to_string(), "README.md".to_string()];
        let (groups, touched) = classify_files(&files, &fams);
        assert!(touched.contains("viewmodel") && touched.contains("i18n"));
        assert!(groups.get("(unclassified)").map(|v| v.contains(&"README.md".to_string())).unwrap_or(false));
    }
}
