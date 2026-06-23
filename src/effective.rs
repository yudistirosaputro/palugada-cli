//! Per-project convention overlay + effective-rules merge (web cycle C).
//!
//! A project bound to a shared profile can add/override/remap conventions for
//! itself only, stored in its repo `.palugada/`. This module owns the merge of
//! profile rules with the per-project overlay ("effective rules") and the I/O
//! resolver that the CLI, web console, and `brief` consume.

use crate::knowledge::TopicMeta;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Profile,
    Project,
    Overridden,
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct EffectiveConvention {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub origin: Origin,
}

#[derive(serde::Serialize, Debug, PartialEq)]
pub struct EffectiveReviewEntry {
    pub family: String,
    pub conventions: Vec<String>,
    pub origin: Origin,
}

#[derive(serde::Serialize, Debug)]
pub struct EffectiveRules {
    pub project: String,
    pub profile: String,
    pub conventions: Vec<EffectiveConvention>,
    pub review_map: Vec<EffectiveReviewEntry>,
    pub warnings: Vec<String>,
}

// ── pure merge layer ───────────────────────────────────────────────────────

/// Merge conventions by id: an overlay id that matches a profile id →
/// `Overridden` (overlay metadata wins), overlay-only → `Project`, profile-only
/// → `Profile`. Profile order first, then overlay-only ids.
pub fn merge_conventions(profile: &[TopicMeta], overlay: &[TopicMeta]) -> Vec<EffectiveConvention> {
    let overlay_ids: BTreeSet<&str> = overlay.iter().map(|t| t.id.as_str()).collect();
    let profile_ids: BTreeSet<&str> = profile.iter().map(|t| t.id.as_str()).collect();
    let mut out: Vec<EffectiveConvention> = Vec::new();
    for t in profile {
        let origin = if overlay_ids.contains(t.id.as_str()) { Origin::Overridden } else { Origin::Profile };
        // When overridden, prefer the overlay's metadata.
        let src = if origin == Origin::Overridden {
            overlay.iter().find(|o| o.id == t.id).unwrap_or(t)
        } else {
            t
        };
        out.push(EffectiveConvention {
            id: src.id.clone(),
            title: src.title.clone(),
            description: src.description.clone(),
            tags: src.tags.clone(),
            origin,
        });
    }
    for o in overlay {
        if !profile_ids.contains(o.id.as_str()) {
            out.push(EffectiveConvention {
                id: o.id.clone(),
                title: o.title.clone(),
                description: o.description.clone(),
                tags: o.tags.clone(),
                origin: Origin::Project,
            });
        }
    }
    out
}

/// Replace-per-family: an overlay family replaces the profile's list for that
/// family; families absent from the overlay keep the profile's list.
pub fn apply_review_overlay(
    profile: &BTreeMap<String, Vec<String>>,
    overlay: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, Vec<String>> {
    let mut out = profile.clone();
    for (fam, ids) in overlay {
        out.insert(fam.clone(), ids.clone());
    }
    out
}

/// Build the effective review_map with provenance: families present in the
/// overlay are `Project`, the rest `Profile`.
pub fn merge_review_map(
    profile: &BTreeMap<String, Vec<String>>,
    overlay: &BTreeMap<String, Vec<String>>,
) -> Vec<EffectiveReviewEntry> {
    apply_review_overlay(profile, overlay)
        .into_iter()
        .map(|(family, conventions)| {
            let origin = if overlay.contains_key(&family) { Origin::Project } else { Origin::Profile };
            EffectiveReviewEntry { family, conventions, origin }
        })
        .collect()
}

/// Warn for review_map references to convention ids that exist in neither layer.
pub fn dangling_review_refs(review: &[EffectiveReviewEntry], known_ids: &BTreeSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    for e in review {
        for c in &e.conventions {
            if !known_ids.contains(c) {
                out.push(format!(
                    "review_map family '{}' references convention '{}' not found in profile or overlay",
                    e.family, c
                ));
            }
        }
    }
    out
}

// ── I/O resolver ───────────────────────────────────────────────────────────

/// `<repo>/.palugada/conventions` — the per-project convention overlay dir.
pub fn overlay_dir(repo_path: &str) -> PathBuf {
    crate::config::expand_home(repo_path).join(".palugada").join("conventions")
}

#[derive(serde::Deserialize, Default)]
struct ProfileReview {
    #[serde(default)]
    review_map: BTreeMap<String, Vec<String>>,
}

/// The profile's `review_map` (family → convention ids) from its `profile.yaml`.
pub fn profile_review_map(kn: &Path, profile: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let p = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let pr: ProfileReview = serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))?;
    Ok(pr.review_map)
}

/// Outline for a convention id, preferring the project overlay over the profile.
pub fn convention_outline_overlaid(kn: &Path, profile: &str, overlay: &Path, id: &str) -> String {
    if overlay.join(format!("{id}.md")).exists() {
        crate::knowledge::convention_outline_in(overlay, id).unwrap_or_else(|e| format!("({e})"))
    } else {
        crate::knowledge::convention_outline(kn, profile, id).unwrap_or_else(|e| format!("({e})"))
    }
}

/// Resolve the effective rules (profile + per-project overlay) for a project.
pub fn effective_rules(global: &crate::config::GlobalConfig, name: &str) -> Result<EffectiveRules, String> {
    let kn = crate::knowledge::knowledge_dir(global)?;
    let entry = global
        .projects
        .registered
        .get(name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?;
    let pc = crate::config::ProjectConfig::load_from(&entry.repo_path)?;
    let profile = pc.profile.clone();

    let profile_convs = crate::knowledge::conventions(&kn, &profile)?;
    let overlay_convs = crate::knowledge::conventions_in(&overlay_dir(&entry.repo_path))?;
    let conventions = merge_conventions(&profile_convs, &overlay_convs);

    let prof_map = profile_review_map(&kn, &profile).unwrap_or_default();
    let review_map = merge_review_map(&prof_map, &pc.review_map);

    let known: BTreeSet<String> = conventions.iter().map(|c| c.id.clone()).collect();
    let warnings = dangling_review_refs(&review_map, &known);

    Ok(EffectiveRules { project: name.to_string(), profile, conventions, review_map, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{GlobalConfig, ProjectConfig, ProjectEntry};

    fn meta(id: &str) -> TopicMeta {
        TopicMeta {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            tags: vec![],
            sections: vec![],
            related: vec![],
        }
    }

    #[test]
    fn merge_conventions_classifies_origin() {
        let profile = vec![meta("architecture"), meta("errorhandling")];
        let overlay = vec![meta("architecture"), meta("ours")];
        let eff = merge_conventions(&profile, &overlay);
        let by: BTreeMap<_, _> = eff.iter().map(|c| (c.id.clone(), c.origin)).collect();
        assert_eq!(by["architecture"], Origin::Overridden);
        assert_eq!(by["errorhandling"], Origin::Profile);
        assert_eq!(by["ours"], Origin::Project);
        assert_eq!(eff.len(), 3);
    }

    #[test]
    fn apply_review_overlay_replaces_per_family() {
        let mut profile = BTreeMap::new();
        profile.insert("viewmodel".to_string(), vec!["architecture".to_string(), "testing".to_string()]);
        profile.insert("repository".to_string(), vec!["architecture".to_string()]);
        let mut overlay = BTreeMap::new();
        overlay.insert("viewmodel".to_string(), vec!["ours".to_string()]);
        let merged = apply_review_overlay(&profile, &overlay);
        assert_eq!(merged["viewmodel"], vec!["ours".to_string()]); // replaced
        assert_eq!(merged["repository"], vec!["architecture".to_string()]); // kept
    }

    #[test]
    fn merge_review_map_marks_origin() {
        let mut profile = BTreeMap::new();
        profile.insert("repository".to_string(), vec!["architecture".to_string()]);
        let mut overlay = BTreeMap::new();
        overlay.insert("viewmodel".to_string(), vec!["ours".to_string()]);
        let eff = merge_review_map(&profile, &overlay);
        let by: BTreeMap<_, _> = eff.iter().map(|e| (e.family.clone(), e.origin)).collect();
        assert_eq!(by["viewmodel"], Origin::Project);
        assert_eq!(by["repository"], Origin::Profile);
    }

    #[test]
    fn dangling_refs_reported() {
        let eff = vec![EffectiveReviewEntry {
            family: "service".into(),
            conventions: vec!["architecture".into(), "nope".into()],
            origin: Origin::Profile,
        }];
        let known: BTreeSet<String> = ["architecture".to_string()].into_iter().collect();
        let warns = dangling_review_refs(&eff, &known);
        assert_eq!(warns.len(), 1);
        assert!(warns[0].contains("nope") && warns[0].contains("service"));
    }

    fn write(p: &std::path::Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn effective_rules_merges_profile_and_overlay() {
        let home = tempfile::tempdir().unwrap();
        let kn = home.path().join("kn");
        write(
            &kn.join("profiles/p/profile.yaml"),
            "flows:\n  review: [diff.scan, convention(by-file-kind)]\nreview_map:\n  viewmodel: [architecture]\n",
        );
        let arch = crate::knowledge::ConventionSpec {
            id: "architecture".into(),
            title: "Arch".into(),
            description: "d".into(),
            tags: vec![],
            sections: vec![],
        };
        crate::knowledge::add_convention_in(&kn.join("profiles/p/conventions"), &arch).unwrap();

        let repo = home.path().join("repo");
        let ours = crate::knowledge::ConventionSpec {
            id: "ours".into(),
            title: "Ours".into(),
            description: "team".into(),
            tags: vec![],
            sections: vec![],
        };
        crate::knowledge::add_convention_in(&overlay_dir(repo.to_str().unwrap()), &ours).unwrap();
        let mut rm = BTreeMap::new();
        rm.insert("viewmodel".to_string(), vec!["ours".to_string()]);
        let pc = ProjectConfig { project: "app".into(), profile: "p".into(), review_map: rm, ..Default::default() };
        pc.save_to(repo.to_str().unwrap()).unwrap();

        let mut global = GlobalConfig::default();
        global.engine.knowledge_path = kn.to_string_lossy().to_string();
        global.projects.registered.insert(
            "app".into(),
            ProjectEntry { repo_path: repo.to_string_lossy().to_string(), workspace: String::new() },
        );

        let eff = effective_rules(&global, "app").unwrap();
        assert_eq!(eff.profile, "p");
        assert!(eff.conventions.iter().any(|c| c.id == "ours" && c.origin == Origin::Project));
        assert!(eff.conventions.iter().any(|c| c.id == "architecture" && c.origin == Origin::Profile));
        let vm = eff.review_map.iter().find(|e| e.family == "viewmodel").unwrap();
        assert_eq!(vm.conventions, vec!["ours".to_string()]);
        assert_eq!(vm.origin, Origin::Project);
    }
}
