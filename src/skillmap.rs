//! Resolve, for a registered project, the exact set of generated skills and the
//! concrete steps/conventions/recipes each routes to — the data behind the web
//! console's per-project "skill flow" view.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::config::GlobalConfig;

#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Step {
    Engine { token: String, label: String },
    Convention { id: String, exists: bool },
    Recipe { id: String, exists: bool },
    ReviewMap { expand: Vec<ReviewKind> },
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ReviewKind {
    pub family: String,
    pub conventions: Vec<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct ToolSkill {
    pub name: String,
    pub requires: Vec<String>,
    pub enabled: bool,
}

/// Tool skills and the integration kinds that enable them. Mirrors the gating in
/// `scaffold::skill_files` (kept honest by `tool_skills_gate_on_kinds`).
const TOOL_SKILLS: &[(&str, &[&str])] = &[
    ("palugada-git", &["git_host"]),
    ("palugada-docs", &["issue_tracker", "wiki"]),
    ("palugada-ci", &["ci", "chat"]),
    ("palugada-design", &["design"]),
];

fn engine_label(token: &str) -> String {
    match token {
        "code.recent" => "recent changes",
        "symbol.find" => "relevant symbols",
        "prd.context" => "the linked issue / PRD",
        "module.info" => "module overview",
        "diff.scan" => "scan the diff",
        other => other,
    }
    .to_string()
}

/// `convention(errorhandling)` → Some("errorhandling") for head "convention".
fn paren<'a>(s: &'a str, head: &str) -> Option<&'a str> {
    s.strip_prefix(head)?.strip_prefix('(')?.strip_suffix(')')
}

fn review_expand(review_map: &BTreeMap<String, Vec<String>>) -> Vec<ReviewKind> {
    review_map
        .iter()
        .map(|(family, conventions)| ReviewKind { family: family.clone(), conventions: conventions.clone() })
        .collect()
}

/// Classify one `flows:` step token into a typed Step.
pub fn classify_step(
    step: &str,
    conv_ids: &BTreeSet<String>,
    recipe_ids: &BTreeSet<String>,
    review_map: &BTreeMap<String, Vec<String>>,
) -> Step {
    let s = step.trim();
    if let Some(inner) = paren(s, "convention") {
        if inner == "by-file-kind" {
            return Step::ReviewMap { expand: review_expand(review_map) };
        }
        return Step::Convention { id: inner.to_string(), exists: conv_ids.contains(inner) };
    }
    if let Some(inner) = paren(s, "recipe") {
        return Step::Recipe { id: inner.to_string(), exists: recipe_ids.contains(inner) };
    }
    if s == "by-file-kind" {
        return Step::ReviewMap { expand: review_expand(review_map) };
    }
    Step::Engine { token: s.to_string(), label: engine_label(s) }
}

/// Tool skills with enabled-status for the given configured integration kinds.
pub fn tool_skills(kinds: &[&str]) -> Vec<ToolSkill> {
    TOOL_SKILLS
        .iter()
        .map(|(name, req)| ToolSkill {
            name: (*name).to_string(),
            requires: req.iter().map(|s| (*s).to_string()).collect(),
            enabled: req.iter().any(|k| kinds.contains(k)),
        })
        .collect()
}

#[derive(Serialize, Debug, PartialEq)]
pub struct SkillMap {
    pub project: String,
    pub profile: String,
    pub skills: Vec<serde_json::Value>,
    pub warnings: Vec<String>,
}

fn load_flows(kn: &Path, profile: &str) -> Result<crate::manifest::ProfileManifest, String> {
    crate::manifest::ProfileManifest::load(kn, profile)
}

fn custom_skill_names(kn: &Path, profile: &str) -> Vec<String> {
    let dir = kn.join("profiles").join(profile).join("skills");
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            if e.path().is_dir() {
                out.push(e.file_name().to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

/// Build the per-project skill flow map (project → bound profile + integrations).
pub fn skillmap(global: &GlobalConfig, name: &str) -> Result<SkillMap, String> {
    let kn = crate::knowledge::knowledge_dir(global)?;
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = crate::config::ProjectConfig::load_from(&entry.repo_path)?;
    let profile = pc.profile.clone();
    let kinds = crate::scaffold::integration_kinds(&pc);

    let pf = load_flows(&kn, &profile)?;
    let conv_ids: BTreeSet<String> =
        crate::knowledge::conventions(&kn, &profile)?.into_iter().map(|c| c.id).collect();
    let recipe_ids: BTreeSet<String> =
        crate::knowledge::recipes(&kn, &profile)?.into_iter().map(|r| r.id).collect();

    let mut warnings: Vec<String> = Vec::new();
    let mut skills: Vec<serde_json::Value> = Vec::new();

    skills.push(serde_json::json!({
        "name": "palugada-search", "kind": "search",
        "command": "palugada symbol <q>  /  palugada fact <family>",
    }));

    for &(flow, _title, _trig, _verb) in crate::scaffold::FLOW_SKILLS {
        let steps: Vec<Step> = match pf.flows.get(flow) {
            Some(tokens) => tokens
                .iter()
                .map(|t| classify_step(t, &conv_ids, &recipe_ids, &pf.review_map))
                .collect(),
            None => {
                warnings.push(format!("flow '{flow}' has no entry in profile '{profile}' flows:"));
                Vec::new()
            }
        };
        for st in &steps {
            match st {
                Step::Convention { id, exists: false } => {
                    warnings.push(format!("{flow}: convention({id}) is referenced but missing in '{profile}'"));
                }
                Step::Recipe { id, exists: false } => {
                    warnings.push(format!("{flow}: recipe({id}) is referenced but missing in '{profile}'"));
                }
                _ => {}
            }
        }
        skills.push(serde_json::json!({
            "name": format!("palugada-{flow}"), "kind": "flow", "flow": flow,
            "command": format!("palugada brief {flow} <target>"),
            "steps": steps,
        }));
    }

    for flow in pf.flows.keys() {
        if !crate::scaffold::FLOW_SKILLS.iter().any(|(f, _, _, _)| f == flow) {
            warnings.push(format!("profile '{profile}' defines flow '{flow}' with no generated skill"));
        }
    }

    for ts in tool_skills(&kinds) {
        skills.push(serde_json::json!({
            "name": ts.name, "kind": "tool", "enabled": ts.enabled, "needs": ts.requires,
        }));
    }

    for cs in custom_skill_names(&kn, &profile) {
        skills.push(serde_json::json!({ "name": cs, "kind": "custom" }));
    }

    Ok(SkillMap { project: name.to_string(), profile, skills, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(v: &[&str]) -> BTreeSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_convention_recipe_engine() {
        let conv = ids(&["errorhandling"]);
        let rec = ids(&["feature"]);
        let rm = BTreeMap::new();
        assert_eq!(
            classify_step("convention(errorhandling)", &conv, &rec, &rm),
            Step::Convention { id: "errorhandling".into(), exists: true }
        );
        assert_eq!(
            classify_step("convention(perf)", &conv, &rec, &rm),
            Step::Convention { id: "perf".into(), exists: false }
        );
        assert_eq!(
            classify_step("recipe(feature)", &conv, &rec, &rm),
            Step::Recipe { id: "feature".into(), exists: true }
        );
        assert_eq!(
            classify_step("code.recent", &conv, &rec, &rm),
            Step::Engine { token: "code.recent".into(), label: "recent changes".into() }
        );
        assert_eq!(
            classify_step("weird.step", &conv, &rec, &rm),
            Step::Engine { token: "weird.step".into(), label: "weird.step".into() }
        );
    }

    #[test]
    fn classify_by_file_kind_expands_review_map() {
        let mut rm = BTreeMap::new();
        rm.insert("viewmodel".to_string(), vec!["architecture".to_string(), "testing".to_string()]);
        assert_eq!(
            classify_step("convention(by-file-kind)", &ids(&[]), &ids(&[]), &rm),
            Step::ReviewMap {
                expand: vec![ReviewKind {
                    family: "viewmodel".into(),
                    conventions: vec!["architecture".into(), "testing".into()],
                }]
            }
        );
    }

    #[test]
    fn tool_skills_gate_on_kinds() {
        let ts = tool_skills(&["git_host", "wiki"]);
        let get = |n: &str| ts.iter().find(|t| t.name == n).unwrap();
        assert!(get("palugada-git").enabled);
        assert!(get("palugada-docs").enabled); // wiki present
        assert!(!get("palugada-ci").enabled);
        assert!(!get("palugada-design").enabled);
    }

    #[test]
    fn skillmap_resolves_flows_tools_and_warnings() {
        let tmp = tempfile::tempdir().unwrap();
        let kn = tmp.path().join("kn");
        let prof = kn.join("profiles").join("p");
        fs::create_dir_all(prof.join("recipes")).unwrap();
        fs::write(
            prof.join("profile.yaml"),
            "id: p\nflows:\n  bugfix: [code.recent, convention(errorhandling)]\n  feature: [recipe(feature)]\nreview_map:\n  viewmodel: [architecture]\n",
        ).unwrap();
        fs::write(prof.join("recipes").join("_index.json"), r#"{"schema_version":"1.0","recipes":[]}"#).unwrap();
        crate::knowledge::add_convention(&kn, "p", &crate::knowledge::ConventionSpec {
            id: "errorhandling".into(), title: "E".into(), description: "".into(), tags: vec![], sections: vec![],
        }).unwrap();

        let repo = tmp.path().join("repo");
        fs::create_dir_all(repo.join(".palugada")).unwrap();
        fs::write(
            repo.join(".palugada").join("config.yaml"),
            "project: app\nprofile: p\nauth_profile: default\nintegrations:\n  git_host:\n    provider: github\n    base_url: https://api.github.com\n",
        ).unwrap();

        let mut global = GlobalConfig::default();
        global.engine.knowledge_path = kn.display().to_string();
        global.projects.registered.insert(
            "app".into(),
            crate::config::ProjectEntry { repo_path: repo.display().to_string(), workspace: String::new() },
        );

        let m = skillmap(&global, "app").unwrap();
        assert_eq!(m.profile, "p");
        let bugfix = m.skills.iter().find(|s| s["name"] == "palugada-bugfix").unwrap();
        assert_eq!(bugfix["steps"][1]["kind"], "convention");
        assert_eq!(bugfix["steps"][1]["id"], "errorhandling");
        assert_eq!(bugfix["steps"][1]["exists"], true);
        let git = m.skills.iter().find(|s| s["name"] == "palugada-git").unwrap();
        assert_eq!(git["enabled"], true);
        let docs = m.skills.iter().find(|s| s["name"] == "palugada-docs").unwrap();
        assert_eq!(docs["enabled"], false);
        assert!(m.warnings.iter().any(|w| w.contains("recipe(feature)")));
    }
}
