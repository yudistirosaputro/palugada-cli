//! Resolve, for a registered project, the exact set of generated skills and the
//! concrete steps/conventions/recipes each routes to — the data behind the web
//! console's per-project "skill flow" view.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

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
}
