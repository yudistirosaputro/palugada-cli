//! Profile inheritance (`extends`): resolve a child profile's `extends` chain
//! and merge inherited conventions/recipes. A profile with no `extends` has a
//! chain of `[self]`, so every resolver here is a no-op for flat profiles.

use crate::knowledge::{RecipeMeta, SectionMeta, TopicMeta};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Maximum length of an `extends` chain (cycle/runaway backstop).
pub const MAX_DEPTH: usize = 8;

/// Read just the `extends:` scalar from `<id>/profile.yaml` (None if absent,
/// empty, unreadable, or unparseable — a malformed parent simply ends the chain
/// and is surfaced separately by `profile validate`).
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
        let profile_dir = kn.join("profiles").join(&cur);
        let yaml_path = profile_dir.join("profile.yaml");
        // If profile.yaml is absent but the profile dir exists, treat as no-extends
        // (backward-compat: profiles created before profile.yaml was required still work).
        // If neither the yaml nor the dir exists, and we arrived via an extends reference,
        // that is a user error.
        if !yaml_path.is_file() {
            if chain.is_empty() {
                // Starting profile — no profile.yaml is tolerated (chain = [self]).
                chain.push(cur.clone());
                break;
            } else {
                // A parent referenced via extends must have a profile.yaml.
                match chain.last() {
                    Some(child) => return Err(format!("profile '{child}' extends '{cur}' which does not exist")),
                    None => return Err(format!("profile '{cur}' has no profile.yaml at {}", yaml_path.display())),
                }
            }
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
pub struct ParsedConvention {
    pub preamble: String,
    pub sections: Vec<MergedSection>,
}

/// Parse a (front-matter-stripped) convention body. Fence-aware: `## ` lines
/// inside ``` fences are body text, not headings.
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
pub fn parse_sections(body: &str) -> Vec<MergedSection> {
    parse_convention(body).sections
}

/// Merge per-section across chain levels given ancestor→descendant.
/// `levels[0]` is the most-distant ancestor that defines the topic, the last is
/// the most-derived. Spine = `levels[0]`'s order; later levels replace matching
/// anchors in place and append new anchors (in that level's file order).
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

// ── Task 3: resolve a single convention across the chain ─────────────────────

/// Resolve a convention `topic` for `profile` across its `extends` chain into a
/// single synthetic markdown string (front-matter + `# H1`/preamble + merged
/// `## ` sections). A topic defined at exactly one level is returned verbatim.
pub fn resolve_convention_raw(
    kn: &Path,
    profile: &str,
    topic: &str,
) -> Result<Option<String>, String> {
    let chain = resolve_chain(kn, profile)?;
    // Collect the raw .md from each chain level that defines the topic, child first.
    let mut present: Vec<String> = Vec::new();
    for p in &chain {
        let md = kn
            .join("profiles")
            .join(p)
            .join("conventions")
            .join(format!("{topic}.md"));
        match std::fs::read_to_string(&md) {
            Ok(raw) => present.push(raw),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", md.display())),
        }
    }
    if present.is_empty() {
        return Ok(None);
    }
    if present.len() == 1 {
        return Ok(Some(present.remove(0))); // verbatim — no merge, no regression
    }
    // Merge ancestor→descendant (present is child-first, so iterate reversed).
    let parsed: Vec<ParsedConvention> = present
        .iter()
        .rev()
        .map(|raw| parse_convention(crate::knowledge::strip_frontmatter(raw)))
        .collect();
    let levels: Vec<Vec<MergedSection>> = parsed.iter().map(|p| p.sections.clone()).collect();
    let merged = merge_section_lists(&levels);
    let preamble = parsed
        .last()
        .map(|p| p.preamble.clone())
        .unwrap_or_default();
    // Metadata: nearest non-empty value scanning child-first, so a child that
    // overrides a topic but omits title/description still shows the parent's.
    let pick = |key: &str| {
        present.iter().find_map(|raw| crate::knowledge::frontmatter_field(raw, key).filter(|s| !s.is_empty())).unwrap_or_default()
    };
    let title = pick("title");
    let description = pick("description");
    let tags = pick("tags");
    Ok(Some(render_merged(
        topic,
        &title,
        &description,
        &tags,
        &preamble,
        &merged,
    )))
}

/// Reassemble a merged convention into a front-matter-bearing markdown string.
fn render_merged(
    topic: &str,
    title: &str,
    description: &str,
    tags: &str,
    preamble: &str,
    secs: &[MergedSection],
) -> String {
    let mut out = format!("---\nid: {topic}\ntitle: {title}\ndescription: {description}\n");
    if !tags.is_empty() {
        out.push_str(&format!("tags: {tags}\n"));
    }
    out.push_str("---\n\n");
    out.push_str(preamble.trim_end());
    out.push('\n');
    for s in secs {
        out.push_str(&format!(
            "\n## {} {{#{}}}\n{}\n",
            s.title,
            s.anchor,
            s.body.trim_end()
        ));
    }
    out
}

// ── Task 5: recipe resolution across the chain ───────────────────────────────

/// Resolve a recipe's raw markdown across the chain: the nearest level (child
/// first) that defines `<task>.md` wins whole. `None` if absent everywhere.
pub fn resolve_recipe_raw(kn: &Path, profile: &str, task: &str) -> Result<Option<String>, String> {
    let chain = resolve_chain(kn, profile)?;
    for p in &chain {
        let path = kn
            .join("profiles")
            .join(p)
            .join("recipes")
            .join(format!("{task}.md"));
        match std::fs::read_to_string(&path) {
            Ok(raw) => return Ok(Some(raw)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        }
    }
    Ok(None)
}

// ── Task 4: merged index helpers ─────────────────────────────────────────────

/// Merge two section-meta lists by id (descendant overrides in place, appends new).
fn merge_section_metas(base: &[SectionMeta], over: &[SectionMeta]) -> Vec<SectionMeta> {
    let mut order: Vec<String> = base.iter().map(|s| s.id.clone()).collect();
    let mut by: BTreeMap<String, SectionMeta> =
        base.iter().map(|s| (s.id.clone(), s.clone())).collect();
    for s in over {
        if !by.contains_key(&s.id) {
            order.push(s.id.clone());
        }
        by.insert(s.id.clone(), s.clone());
    }
    order.into_iter().filter_map(|id| by.remove(&id)).collect()
}

/// Merged convention catalog across the chain: union by topic id (child wins
/// metadata), with each topic's sections merged by section id.
pub fn merged_conventions(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, TopicMeta> = BTreeMap::new();
    for p in chain.iter().rev() {
        // root → child
        for t in crate::knowledge::conventions(kn, p)? {
            match by_id.get_mut(&t.id) {
                Some(existing) => {
                    existing.sections = merge_section_metas(&existing.sections, &t.sections);
                    existing.title = t.title;
                    existing.description = t.description;
                    existing.tags = t.tags;
                    existing.related = t.related;
                }
                None => {
                    order.push(t.id.clone());
                    by_id.insert(t.id.clone(), t);
                }
            }
        }
    }
    Ok(order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect())
}

/// Merged recipe catalog across the chain: whole-recipe override by id.
pub fn merged_recipes(kn: &Path, profile: &str) -> Result<Vec<RecipeMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, RecipeMeta> = BTreeMap::new();
    for p in chain.iter().rev() {
        for r in crate::knowledge::recipes(kn, p)? {
            if !by_id.contains_key(&r.id) {
                order.push(r.id.clone());
            }
            by_id.insert(r.id.clone(), r);
        }
    }
    Ok(order
        .into_iter()
        .filter_map(|id| by_id.remove(&id))
        .collect())
}

/// Like `merged_conventions` but fills each topic's and section's `origin`/`from`
/// provenance relative to the active profile (`chain[0]`).
pub fn merged_conventions_provenance(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let active = chain.first().cloned().unwrap_or_default();
    // Per topic id: ordered build + the set of profiles that define it, and per
    // section id: the set of profiles that define that section.
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, TopicMeta> = BTreeMap::new();
    let mut topic_levels: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut sec_levels: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    let mut sec_owner: BTreeMap<(String, String), String> = BTreeMap::new();
    for p in chain.iter().rev() {
        // root → child: later (more-derived) wins
        for t in crate::knowledge::conventions(kn, p)? {
            topic_levels.entry(t.id.clone()).or_default().push(p.clone());
            for s in &t.sections {
                sec_levels.entry((t.id.clone(), s.id.clone())).or_default().push(p.clone());
                sec_owner.insert((t.id.clone(), s.id.clone()), p.clone());
            }
            match by_id.get_mut(&t.id) {
                Some(existing) => {
                    existing.sections = merge_section_metas(&existing.sections, &t.sections);
                    existing.title = t.title;
                    existing.description = t.description;
                    existing.tags = t.tags;
                    existing.related = t.related;
                }
                None => {
                    order.push(t.id.clone());
                    by_id.insert(t.id.clone(), t);
                }
            }
        }
    }
    let mut out: Vec<TopicMeta> = Vec::new();
    for id in order {
        let mut t = match by_id.remove(&id) { Some(t) => t, None => continue };
        let levels = topic_levels.get(&id).cloned().unwrap_or_default();
        let (origin, from) = classify(&active, &levels);
        t.origin = origin;
        t.from = from;
        for s in &mut t.sections {
            let key = (id.clone(), s.id.clone());
            let slevels = sec_levels.get(&key).cloned().unwrap_or_default();
            let (so, _sf) = classify(&active, &slevels);
            s.origin = so;
            s.from = sec_owner.get(&key).cloned().unwrap_or_default();
        }
        out.push(t);
    }
    Ok(out)
}

/// Like `merged_recipes` but fills each recipe's `origin`/`from` (whole-doc).
pub fn merged_recipes_provenance(kn: &Path, profile: &str) -> Result<Vec<RecipeMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let active = chain.first().cloned().unwrap_or_default();
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, RecipeMeta> = BTreeMap::new();
    let mut levels: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut owner: BTreeMap<String, String> = BTreeMap::new();
    for p in chain.iter().rev() {
        for r in crate::knowledge::recipes(kn, p)? {
            levels.entry(r.id.clone()).or_default().push(p.clone());
            owner.insert(r.id.clone(), p.clone());
            if !by_id.contains_key(&r.id) {
                order.push(r.id.clone());
            }
            by_id.insert(r.id.clone(), r);
        }
    }
    let mut out: Vec<RecipeMeta> = Vec::new();
    for id in order {
        let mut r = match by_id.remove(&id) { Some(r) => r, None => continue };
        let lv = levels.get(&id).cloned().unwrap_or_default();
        let (origin, _from) = classify(&active, &lv);
        r.origin = origin;
        r.from = owner.get(&id).cloned().unwrap_or_default();
        out.push(r);
    }
    Ok(out)
}

/// Classify provenance from the set of profiles (in any order) that define a
/// topic/section, relative to the active profile id.
/// own = only active; overridden = active + ≥1 ancestor; inherited = only ancestor(s).
/// `from` = active when active is present, else the most-derived ancestor (last in
/// the root→child push order = the winning ancestor).
fn classify(active: &str, levels: &[String]) -> (String, String) {
    let has_active = levels.iter().any(|p| p == active);
    if has_active {
        if levels.iter().any(|p| p != active) {
            ("overridden".to_string(), active.to_string())
        } else {
            ("own".to_string(), active.to_string())
        }
    } else {
        let from = levels.last().cloned().unwrap_or_default();
        ("inherited".to_string(), from)
    }
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

    // ── Task 3 tests: resolve_convention_raw ────────────────────────────────

    /// Author a convention `<topic>.md` directly under a profile (no index needed
    /// for resolve_convention_raw, which reads the .md files).
    fn conv(kn: &Path, profile: &str, topic: &str, md: &str) {
        let dir = kn.join("profiles").join(profile).join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{topic}.md")), md).unwrap();
    }

    #[test]
    fn flat_profile_convention_returned_verbatim() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        let md = "---\nid: arch\ntitle: Arch\n---\n\n# Arch\n> intro\n\n## Layers {#layers}\nL\n";
        conv(kn.path(), "p", "arch", md);
        assert_eq!(
            resolve_convention_raw(kn.path(), "p", "arch")
                .unwrap()
                .as_deref(),
            Some(md)
        );
    }

    #[test]
    fn child_overrides_one_section_inherits_rest() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvvm", None);
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        conv(kn.path(), "android-mvvm", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Layers {#layers}\nlayers body\n\n## Data Flow {#data-flow}\nLiveData wiring\n");
        conv(kn.path(), "android-mvi", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nStateFlow + reducer\n");
        let raw = resolve_convention_raw(kn.path(), "android-mvi", "architecture")
            .unwrap()
            .unwrap();
        assert!(
            raw.contains("## Layers {#layers}"),
            "inherited section kept: {raw}"
        );
        assert!(raw.contains("layers body"));
        assert!(
            raw.contains("StateFlow + reducer"),
            "child override wins: {raw}"
        );
        assert!(
            !raw.contains("LiveData wiring"),
            "parent's data-flow body replaced: {raw}"
        );
        // order: Layers before Data Flow (spine preserved)
        assert!(raw.find("{#layers}").unwrap() < raw.find("{#data-flow}").unwrap());
    }

    #[test]
    fn child_without_description_inherits_parents() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvvm", None);
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        // parent defines the topic WITH a description
        conv(kn.path(), "android-mvvm", "architecture",
            "---\nid: architecture\ntitle: Architecture\ndescription: How layers wire together\n---\n\n# Architecture\n\n## Layers {#layers}\nlayers body\n\n## Data Flow {#data-flow}\nLiveData wiring\n");
        // child overrides one section but omits description: in its front-matter
        conv(kn.path(), "android-mvi", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nStateFlow + reducer\n");
        let raw = resolve_convention_raw(kn.path(), "android-mvi", "architecture")
            .unwrap()
            .unwrap();
        // merged front-matter still carries the parent's description
        assert_eq!(
            crate::knowledge::frontmatter_field(&raw, "description").as_deref(),
            Some("How layers wire together"),
            "child without description inherits parent's: {raw}"
        );
        // and it appears before the first section heading (in the front-matter block)
        assert!(raw.find("How layers wire together").unwrap() < raw.find("## ").unwrap());
    }

    #[test]
    fn inherited_only_topic_is_verbatim_from_parent() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvvm", None);
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        let md = "---\nid: testing\ntitle: Testing\n---\n\n# Testing\n## Unit {#unit}\nx\n";
        conv(kn.path(), "android-mvvm", "testing", md);
        assert_eq!(
            resolve_convention_raw(kn.path(), "android-mvi", "testing")
                .unwrap()
                .as_deref(),
            Some(md)
        );
    }

    #[test]
    fn absent_topic_is_none() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        assert_eq!(
            resolve_convention_raw(kn.path(), "p", "nope").unwrap(),
            None
        );
    }

    // ── Task 4 tests: merged_conventions / merged_recipes ────────────────────

    fn conv_indexed(kn: &Path, profile: &str, topic: &str, sections: &[(&str, &str)]) {
        // Writes <topic>.md AND a matching _index.json entry via knowledge writers.
        let dir = kn.join("profiles").join(profile).join("conventions");
        let specs: Vec<crate::knowledge::SectionSpec> = sections
            .iter()
            .map(|(_, title)| crate::knowledge::SectionSpec {
                title: (*title).into(),
                body: "b".into(),
                code: false,
            })
            .collect();
        crate::knowledge::add_convention_in(
            &dir,
            &crate::knowledge::ConventionSpec {
                id: topic.into(),
                title: topic.into(),
                description: format!("{topic} desc"),
                tags: vec![],
                sections: specs,
            },
        )
        .unwrap();
    }

    #[test]
    fn merged_conventions_union_child_wins_and_merges_sections() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        conv_indexed(
            kn.path(),
            "base",
            "architecture",
            &[("layers", "Layers"), ("data-flow", "Data Flow")],
        );
        conv_indexed(kn.path(), "base", "testing", &[("unit", "Unit")]);
        conv_indexed(
            kn.path(),
            "child",
            "architecture",
            &[("data-flow", "Data Flow"), ("reducer", "Reducer")],
        );

        let merged = merged_conventions(kn.path(), "child").unwrap();
        let ids: Vec<&str> = merged.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"architecture") && ids.contains(&"testing"));
        let arch = merged.iter().find(|t| t.id == "architecture").unwrap();
        let secs: Vec<&str> = arch.sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(secs, vec!["layers", "data-flow", "reducer"]); // spine + appended override-set
    }

    // ── Task 5 tests: resolve_recipe_raw ────────────────────────────────────

    #[test]
    fn recipe_inherited_then_overridden() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        let bdir = kn.path().join("profiles/base/recipes");
        std::fs::create_dir_all(&bdir).unwrap();
        std::fs::write(
            bdir.join("feature.md"),
            "---\nid: feature\n---\n# F\nbase steps\n",
        )
        .unwrap();
        // inherited
        assert!(resolve_recipe_raw(kn.path(), "child", "feature")
            .unwrap()
            .unwrap()
            .contains("base steps"));
        // overridden by child
        let cdir = kn.path().join("profiles/child/recipes");
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::write(
            cdir.join("feature.md"),
            "---\nid: feature\n---\n# F\nchild steps\n",
        )
        .unwrap();
        assert!(resolve_recipe_raw(kn.path(), "child", "feature")
            .unwrap()
            .unwrap()
            .contains("child steps"));
        // absent
        assert_eq!(
            resolve_recipe_raw(kn.path(), "child", "nope").unwrap(),
            None
        );
    }

    #[test]
    fn end_to_end_three_level_mvvm_to_mvi() {
        let kn = tempfile::tempdir().unwrap();
        // android-base: architecture (layers) + testing
        profile(kn.path(), "android-base", None);
        conv(kn.path(), "android-base", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Layers {#layers}\nlayer rules\n");
        conv(kn.path(), "android-base", "testing",
            "---\nid: testing\ntitle: Testing\n---\n\n# Testing\n\n## Unit {#unit}\nunit rules\n");
        // android-mvvm extends base: adds data-flow (LiveData)
        profile(kn.path(), "android-mvvm", Some("android-base"));
        conv(kn.path(), "android-mvvm", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nLiveData wiring\n");
        // android-mvi extends mvvm: overrides data-flow (StateFlow)
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        conv(kn.path(), "android-mvi", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nStateFlow + reducer\n");

        // chain
        assert_eq!(
            resolve_chain(kn.path(), "android-mvi").unwrap(),
            vec!["android-mvi".to_string(), "android-mvvm".to_string(), "android-base".to_string()]
        );
        // merged architecture: layers (grandparent) + data-flow (child override)
        let arch = resolve_convention_raw(kn.path(), "android-mvi", "architecture").unwrap().unwrap();
        assert!(arch.contains("## Layers {#layers}") && arch.contains("layer rules"));
        assert!(arch.contains("StateFlow + reducer"));
        assert!(!arch.contains("LiveData wiring"));
        assert!(arch.find("{#layers}").unwrap() < arch.find("{#data-flow}").unwrap());
        // testing inherited verbatim from grandparent
        let testing = resolve_convention_raw(kn.path(), "android-mvi", "testing").unwrap().unwrap();
        assert!(testing.contains("unit rules"));
    }

    #[test]
    fn merged_recipes_union_child_overrides_whole() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        let bdir = kn.path().join("profiles/base/recipes");
        let cdir = kn.path().join("profiles/child/recipes");
        crate::knowledge::add_recipe_from_markdown(
            &bdir,
            "---\nid: feature\ntitle: Base Feature\n---\n# F\nbase\n",
        )
        .unwrap();
        crate::knowledge::add_recipe_from_markdown(
            &bdir,
            "---\nid: refactor\ntitle: Refactor\n---\n# R\nr\n",
        )
        .unwrap();
        crate::knowledge::add_recipe_from_markdown(
            &cdir,
            "---\nid: feature\ntitle: Child Feature\n---\n# F\nchild\n",
        )
        .unwrap();

        let merged = merged_recipes(kn.path(), "child").unwrap();
        assert_eq!(merged.len(), 2);
        let feature = merged.iter().find(|r| r.id == "feature").unwrap();
        assert_eq!(feature.title, "Child Feature"); // child wins
    }

    #[test]
    fn provenance_marks_own_overridden_inherited() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        // base: architecture(layers, data-flow) + testing(unit)
        conv_indexed(kn.path(), "base", "architecture", &[("layers", "Layers"), ("data-flow", "Data Flow")]);
        conv_indexed(kn.path(), "base", "testing", &[("unit", "Unit")]);
        // child: overrides architecture's data-flow + adds reducer; adds own `style`
        conv_indexed(kn.path(), "child", "architecture", &[("data-flow", "Data Flow"), ("reducer", "Reducer")]);
        conv_indexed(kn.path(), "child", "style", &[("naming", "Naming")]);

        let topics = merged_conventions_provenance(kn.path(), "child").unwrap();
        let arch = topics.iter().find(|t| t.id == "architecture").unwrap();
        assert_eq!(arch.origin, "overridden"); // in both child and base
        let testing = topics.iter().find(|t| t.id == "testing").unwrap();
        assert_eq!(testing.origin, "inherited");
        assert_eq!(testing.from, "base");
        let style = topics.iter().find(|t| t.id == "style").unwrap();
        assert_eq!(style.origin, "own");
        // section-level provenance within architecture
        let layers = arch.sections.iter().find(|s| s.id == "layers").unwrap();
        assert_eq!(layers.origin, "inherited");
        assert_eq!(layers.from, "base");
        let df = arch.sections.iter().find(|s| s.id == "data-flow").unwrap();
        assert_eq!(df.origin, "overridden");
        let reducer = arch.sections.iter().find(|s| s.id == "reducer").unwrap();
        assert_eq!(reducer.origin, "own");
    }

    #[test]
    fn provenance_recipe_whole_doc() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        let bdir = kn.path().join("profiles/base/recipes");
        std::fs::create_dir_all(&bdir).unwrap();
        crate::knowledge::add_recipe_from_markdown(&bdir, "---\nid: feature\ntitle: Base F\n---\n# F\nb\n").unwrap();
        crate::knowledge::add_recipe_from_markdown(&bdir, "---\nid: refactor\ntitle: R\n---\n# R\nr\n").unwrap();
        let cdir = kn.path().join("profiles/child/recipes");
        std::fs::create_dir_all(&cdir).unwrap();
        crate::knowledge::add_recipe_from_markdown(&cdir, "---\nid: feature\ntitle: Child F\n---\n# F\nc\n").unwrap();

        let recipes = merged_recipes_provenance(kn.path(), "child").unwrap();
        assert_eq!(recipes.iter().find(|r| r.id == "feature").unwrap().origin, "overridden");
        let refac = recipes.iter().find(|r| r.id == "refactor").unwrap();
        assert_eq!(refac.origin, "inherited");
        assert_eq!(refac.from, "base");
    }

    #[test]
    fn resolve_convention_propagates_non_notfound_io_error() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        // Make <topic>.md a DIRECTORY → read_to_string fails with a non-NotFound error.
        let cdir = kn.path().join("profiles/p/conventions");
        std::fs::create_dir_all(cdir.join("arch.md")).unwrap();
        let r = resolve_convention_raw(kn.path(), "p", "arch");
        assert!(r.is_err(), "a non-NotFound read error must propagate, got {r:?}");
    }

    #[test]
    fn merged_convention_carries_tags_from_nearest() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        // base defines arch with tags + a section; child overrides one section, omits tags
        conv(kn.path(), "base", "architecture",
            "---\nid: architecture\ntitle: Architecture\ntags: [kt, mvvm]\n---\n\n# Architecture\n\n## Layers {#layers}\nL\n");
        conv(kn.path(), "child", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nDF\n");
        let raw = resolve_convention_raw(kn.path(), "child", "architecture").unwrap().unwrap();
        assert!(raw.contains("tags: [kt, mvvm]"), "merged front-matter carries parent tags: {raw}");
    }
}
