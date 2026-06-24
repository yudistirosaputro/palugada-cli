//! Profile inheritance (`extends`): resolve a child profile's `extends` chain
//! and merge inherited conventions/recipes. A profile with no `extends` has a
//! chain of `[self]`, so every resolver here is a no-op for flat profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Maximum length of an `extends` chain (cycle/runaway backstop).
pub const MAX_DEPTH: usize = 8;

/// Read just the `extends:` scalar from `<id>/profile.yaml` (None if absent,
/// empty, unreadable, or unparseable — a malformed parent simply ends the chain
/// and is surfaced separately by `profile validate`).
#[allow(dead_code)] // temporary: consumed by Task 2 merge logic
pub fn read_extends(kn: &Path, id: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct E {
        #[serde(default)]
        extends: Option<String>,
    }
    let p = kn.join("profiles").join(id).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).ok()?;
    let e: E = serde_yaml::from_str(&raw).ok()?;
    e.extends.filter(|s| !s.is_empty())
}

/// The `extends` chain for `id`, most-derived first: `[id, parent, ...]`.
/// Errors on a missing profile in the chain, a cycle, or a chain deeper than
/// `MAX_DEPTH`.
#[allow(dead_code)] // temporary: consumed by Task 2 merge logic
pub fn resolve_chain(kn: &Path, id: &str) -> Result<Vec<String>, String> {
    let mut chain: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cur = id.to_string();
    loop {
        if !seen.insert(cur.clone()) {
            let mut path = chain.clone();
            path.push(cur.clone());
            return Err(format!("inheritance cycle: {}", path.join(" \u{2192} ")));
        }
        if !kn
            .join("profiles")
            .join(&cur)
            .join("profile.yaml")
            .is_file()
        {
            return match chain.last() {
                Some(child) => Err(format!(
                    "profile '{child}' extends '{cur}' which does not exist"
                )),
                None => Err(format!(
                    "profile '{cur}' has no profile.yaml at {}",
                    kn.join("profiles").join(&cur).display()
                )),
            };
        }
        chain.push(cur.clone());
        if chain.len() > MAX_DEPTH {
            return Err(format!(
                "inheritance chain too deep (> {MAX_DEPTH}) starting at '{id}'"
            ));
        }
        match read_extends(kn, &cur) {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    Ok(chain)
}

/// One `## Heading {#anchor}` section of a convention body.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // temporary: consumed by later tasks
pub struct MergedSection {
    /// The `{#anchor}` id; falls back to `slug(title)` when no explicit anchor.
    pub anchor: String,
    pub title: String,
    /// Body text after the heading, up to the next `## ` (trailing newline kept).
    pub body: String,
}

/// A convention body split into its leading preamble (everything before the
/// first `## ` — typically the `# H1` + any intro prose) and its sections.
#[derive(Clone, Debug, PartialEq)]
#[allow(dead_code)] // temporary: consumed by later tasks
pub struct ParsedConvention {
    pub preamble: String,
    pub sections: Vec<MergedSection>,
}

/// Parse a (front-matter-stripped) convention body. Fence-aware: `## ` lines
/// inside ``` fences are body text, not headings.
#[allow(dead_code)] // temporary: consumed by later tasks
pub fn parse_convention(body: &str) -> ParsedConvention {
    let mut preamble = String::new();
    let mut sections: Vec<MergedSection> = Vec::new();
    let mut cur: Option<MergedSection> = None;
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            if let Some(rest) = line.strip_prefix("## ") {
                if let Some(s) = cur.take() {
                    sections.push(s);
                }
                let title = rest.split("{#").next().unwrap_or(rest).trim().to_string();
                let anchor = rest
                    .find("{#")
                    .and_then(|i| {
                        rest[i + 2..]
                            .find('}')
                            .map(|j| rest[i + 2..i + 2 + j].trim().to_string())
                    })
                    .filter(|a| !a.is_empty())
                    .unwrap_or_else(|| crate::knowledge::slug(&title));
                cur = Some(MergedSection {
                    anchor,
                    title,
                    body: String::new(),
                });
                continue;
            }
        }
        match cur.as_mut() {
            Some(s) => {
                s.body.push_str(line);
                s.body.push('\n');
            }
            None => {
                preamble.push_str(line);
                preamble.push('\n');
            }
        }
    }
    if let Some(s) = cur.take() {
        sections.push(s);
    }
    ParsedConvention { preamble, sections }
}

/// Just the sections of a convention body (anchor-aware).
#[allow(dead_code)] // temporary: consumed by later tasks
pub fn parse_sections(body: &str) -> Vec<MergedSection> {
    parse_convention(body).sections
}

/// Merge per-section across chain levels given ancestor→descendant.
/// `levels[0]` is the most-distant ancestor that defines the topic, the last is
/// the most-derived. Spine = `levels[0]`'s order; later levels replace matching
/// anchors in place and append new anchors (in that level's file order).
#[allow(dead_code)] // temporary: consumed by later tasks
fn merge_section_lists(levels: &[Vec<MergedSection>]) -> Vec<MergedSection> {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, MergedSection> = BTreeMap::new();
    for lvl in levels {
        for s in lvl {
            if by_id.contains_key(&s.anchor) {
                by_id.insert(s.anchor.clone(), s.clone());
            } else {
                order.push(s.anchor.clone());
                by_id.insert(s.anchor.clone(), s.clone());
            }
        }
    }
    order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Write a minimal `profile.yaml` for `id` with an optional `extends`.
    fn profile(kn: &Path, id: &str, extends: Option<&str>) {
        let dir = kn.join("profiles").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut y = format!("id: {id}\n");
        if let Some(e) = extends {
            y.push_str(&format!("extends: {e}\n"));
        }
        std::fs::write(dir.join("profile.yaml"), y).unwrap();
    }

    #[test]
    fn chain_for_flat_profile_is_self_only() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "rust-cli", None);
        assert_eq!(
            resolve_chain(kn.path(), "rust-cli").unwrap(),
            vec!["rust-cli".to_string()]
        );
        assert_eq!(read_extends(kn.path(), "rust-cli"), None);
    }

    #[test]
    fn chain_walks_parents_most_derived_first() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-base", None);
        profile(kn.path(), "android-mvvm", Some("android-base"));
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        assert_eq!(
            resolve_chain(kn.path(), "android-mvi").unwrap(),
            vec![
                "android-mvi".to_string(),
                "android-mvvm".to_string(),
                "android-base".to_string()
            ]
        );
    }

    #[test]
    fn missing_parent_errors_with_child_name() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvi", Some("android-foo"));
        let err = resolve_chain(kn.path(), "android-mvi").unwrap_err();
        assert!(
            err.contains("android-mvi") && err.contains("android-foo"),
            "{err}"
        );
    }

    #[test]
    fn cycle_is_detected() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "a", Some("b"));
        profile(kn.path(), "b", Some("a"));
        let err = resolve_chain(kn.path(), "a").unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn over_max_depth_errors() {
        let kn = tempfile::tempdir().unwrap();
        // p0 (root) <- p1 <- ... <- p9  => chain of 10 > MAX_DEPTH (8)
        profile(kn.path(), "p0", None);
        for i in 1..=9 {
            profile(kn.path(), &format!("p{i}"), Some(&format!("p{}", i - 1)));
        }
        let err = resolve_chain(kn.path(), "p9").unwrap_err();
        assert!(err.contains("too deep"), "{err}");
    }

    #[test]
    fn parse_captures_anchor_or_slugs_title() {
        let body = "# Architecture\n> intro\n\n## UI State {#uistate}\nsealed state\n\n## Data Flow\nflow\n";
        let pc = parse_convention(body);
        assert!(pc.preamble.contains("> intro"));
        assert!(pc.preamble.contains("# Architecture"));
        assert_eq!(pc.sections.len(), 2);
        assert_eq!(pc.sections[0].anchor, "uistate"); // explicit {#uistate}
        assert_eq!(pc.sections[0].title, "UI State");
        assert_eq!(pc.sections[1].anchor, "data-flow"); // slug(title)
        assert!(pc.sections[0].body.contains("sealed state"));
    }

    #[test]
    fn parse_ignores_headings_in_fences() {
        let body = "## Real {#real}\ntext\n```\n## fake {#fake}\n```\nmore\n";
        let secs = parse_sections(body);
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].anchor, "real");
        assert!(secs[0].body.contains("## fake"));
    }

    fn sec(anchor: &str, title: &str, body: &str) -> MergedSection {
        MergedSection {
            anchor: anchor.into(),
            title: title.into(),
            body: body.into(),
        }
    }

    #[test]
    fn merge_overrides_in_place_and_appends_new() {
        // ancestor: layers, uistate, data-flow ; child: overrides data-flow, adds reducer
        let parent = vec![
            sec("layers", "Layers", "L"),
            sec("uistate", "UI State", "U"),
            sec("data-flow", "Data Flow", "live"),
        ];
        let child = vec![
            sec("data-flow", "Data Flow", "stateflow"),
            sec("reducer", "Reducer", "R"),
        ];
        let merged = merge_section_lists(&[parent, child]);
        let order: Vec<&str> = merged.iter().map(|s| s.anchor.as_str()).collect();
        assert_eq!(order, vec!["layers", "uistate", "data-flow", "reducer"]); // spine kept, new appended
        let df = merged.iter().find(|s| s.anchor == "data-flow").unwrap();
        assert_eq!(df.body, "stateflow"); // child wins in place
    }

    #[test]
    fn merge_three_levels_most_derived_wins() {
        let root = vec![sec("a", "A", "root-a")];
        let mid = vec![sec("a", "A", "mid-a"), sec("b", "B", "mid-b")];
        let child = vec![sec("b", "B", "child-b")];
        let merged = merge_section_lists(&[root, mid, child]);
        assert_eq!(
            merged.iter().find(|s| s.anchor == "a").unwrap().body,
            "mid-a"
        );
        assert_eq!(
            merged.iter().find(|s| s.anchor == "b").unwrap().body,
            "child-b"
        );
    }
}
