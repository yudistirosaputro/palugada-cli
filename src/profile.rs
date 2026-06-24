//! Profile authoring tooling — list, validate, and scaffold stack profiles.
//! All helpers take the knowledge dir `kn` so they're testable against a temp
//! directory; the engine's own readers (`indexer::load_families`,
//! `indexer::fact_families`) are reused so validation matches runtime behaviour.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Default)]
struct ProfileId {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
}

/// (id, title) for every `kn/profiles/<dir>/` that has a `profile.yaml`, sorted.
pub fn list(kn: &Path) -> Result<Vec<(String, String)>, String> {
    let root = kn.join("profiles");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for entry in fs::read_dir(&root).map_err(|e| format!("read {}: {e}", root.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let pf = entry.path().join("profile.yaml");
        if !pf.is_file() {
            continue;
        }
        let dir_id = entry.file_name().to_string_lossy().to_string();
        let title = fs::read_to_string(&pf)
            .ok()
            .and_then(|raw| serde_yaml::from_str::<ProfileId>(&raw).ok())
            .map(|p| if p.title.is_empty() { dir_id.clone() } else { p.title })
            .unwrap_or_else(|| dir_id.clone());
        out.push((dir_id, title));
    }
    out.sort();
    Ok(out)
}

/// One validation check result.
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}

fn check(name: &str, r: Result<String, String>) -> Check {
    match r {
        Ok(detail) => Check { name: name.into(), ok: true, detail },
        Err(detail) => Check { name: name.into(), ok: false, detail },
    }
}

/// Validate a profile against the schema the engine expects. Returns one Check
/// per rule; the caller prints them and decides the exit code.
pub fn validate(kn: &Path, id: &str) -> Vec<Check> {
    let dir = kn.join("profiles").join(id);
    let mut checks = Vec::new();

    if !dir.is_dir() {
        checks.push(Check {
            name: "profile dir".into(),
            ok: false,
            detail: format!("{} does not exist", dir.display()),
        });
        return checks;
    }
    checks.push(Check { name: "profile dir".into(), ok: true, detail: dir.display().to_string() });

    // profile.yaml parses + has an id
    let pf_path = dir.join("profile.yaml");
    let pf_check = fs::read_to_string(&pf_path)
        .map_err(|e| format!("read {}: {e}", pf_path.display()))
        .and_then(|raw| serde_yaml::from_str::<ProfileId>(&raw).map_err(|e| format!("parse: {e}")))
        .and_then(|p| {
            if p.id.is_empty() {
                Err("profile.yaml has no `id`".into())
            } else {
                Ok(format!("id = {}", p.id))
            }
        });
    checks.push(check("profile.yaml", pf_check));

    // extractors compile (regex + tree-sitter queries + .scm files + ids)
    checks.push(check(
        "extractors.yaml",
        crate::indexer::load_families(kn, id).map(|(_, fams)| format!("{} families compile", fams.len())),
    ));

    // fact_families declared
    checks.push(check(
        "fact_families",
        crate::indexer::fact_families(kn, id).and_then(|f| {
            if f.is_empty() {
                Err("profile.yaml declares no fact_families".into())
            } else {
                Ok(format!("{} declared: {}", f.len(), f.join(", ")))
            }
        }),
    ));

    // indexes (if present) are valid JSON
    for (label, rel) in [("conventions index", "conventions/_index.json"), ("recipes index", "recipes/_index.json")] {
        let p = dir.join(rel);
        if p.exists() {
            let r = fs::read_to_string(&p)
                .map_err(|e| format!("read: {e}"))
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).map(|_| "valid JSON".to_string()).map_err(|e| format!("invalid JSON: {e}")));
            checks.push(check(label, r));
        }
    }

    checks.extend(web_render_checks(kn, id));
    checks
}

/// Hard-fail checks that guarantee a profile renders consistently in the web
/// console: every topic/section has an id+title, recipe cross-refs and `related`
/// ids resolve against the MERGED (inherited + own) sets, every LOCALLY-declared
/// doc has its `<id>.md` on disk, and the extends chain is valid. Any failure
/// makes `palugada profile validate <id>` exit non-zero (main.rs:991).
fn web_render_checks(kn: &Path, id: &str) -> Vec<Check> {
    let dir = kn.join("profiles").join(id);

    // 0. extends chain resolves (cycle / depth / missing parent).
    let chain = match crate::inherit::resolve_chain(kn, id) {
        Ok(c) => c,
        Err(e) => return vec![Check { name: "extends chain".into(), ok: false, detail: e }],
    };
    let chain_detail = if chain.len() > 1 {
        format!("extends: {}", chain[1..].join(" \u{2192} "))
    } else {
        "no extends".into()
    };

    // Merged (inherited + own) sets — cross-refs resolve across the chain.
    let topics = match crate::inherit::merged_conventions(kn, id) {
        Ok(t) => t,
        Err(e) => return vec![Check { name: "conventions render-shape".into(), ok: false, detail: e }],
    };
    let recipes = match crate::inherit::merged_recipes(kn, id) {
        Ok(r) => r,
        Err(e) => return vec![Check { name: "recipes render-shape".into(), ok: false, detail: e }],
    };
    let topic_ids: BTreeSet<&str> = topics.iter().map(|t| t.id.as_str()).collect();
    let recipe_ids: BTreeSet<&str> = recipes.iter().map(|r| r.id.as_str()).collect();
    let mut out: Vec<Check> = Vec::new();

    // 1. render-shape: every topic and section has a non-empty id + title.
    let mut shape: Result<String, String> =
        Ok(format!("{} topics, {} recipes render-ready", topics.len(), recipes.len()));
    'shape: for t in &topics {
        if t.id.trim().is_empty() {
            shape = Err("a topic has an empty id".into());
            break;
        }
        if t.title.trim().is_empty() {
            shape = Err(format!("topic '{}' has an empty title", t.id));
            break;
        }
        for s in &t.sections {
            if s.id.trim().is_empty() {
                shape = Err(format!("topic '{}' has a section with an empty id", t.id));
                break 'shape;
            }
            if s.title.trim().is_empty() {
                shape = Err(format!("topic '{}' section '{}' has an empty title", t.id, s.id));
                break 'shape;
            }
        }
    }
    out.push(check("conventions render-shape", shape));

    // 2. recipe convention_refs resolve to a real topic and (if given) section.
    let mut refs: Result<String, String> =
        Ok(format!("{} recipes: all convention_refs resolve", recipes.len()));
    'refs: for r in &recipes {
        for cr in &r.convention_refs {
            if !topic_ids.contains(cr.topic.as_str()) {
                refs = Err(format!("recipe '{}' references unknown convention '{}'", r.id, cr.topic));
                break 'refs;
            }
            if !cr.section.trim().is_empty() {
                let section_ok = topics
                    .iter()
                    .find(|t| t.id == cr.topic)
                    .map(|t| t.sections.iter().any(|s| s.id == cr.section))
                    .unwrap_or(false);
                if !section_ok {
                    refs = Err(format!(
                        "recipe '{}' references '{}#{}' but that section does not exist",
                        r.id, cr.topic, cr.section
                    ));
                    break 'refs;
                }
            }
        }
    }
    out.push(check("recipe cross-refs resolve", refs));

    // 3. `related` (conventions) and `related_recipes` ids resolve (against merged sets).
    let mut rel: Result<String, String> = Ok("all related ids resolve".into());
    'rel: {
        for t in &topics {
            for rid in &t.related {
                if !topic_ids.contains(rid.as_str()) {
                    rel = Err(format!("convention '{}' lists related '{}' which is not a convention", t.id, rid));
                    break 'rel;
                }
            }
        }
        for r in &recipes {
            for rid in &r.related_recipes {
                if !recipe_ids.contains(rid.as_str()) {
                    rel = Err(format!("recipe '{}' lists related_recipes '{}' which is not a recipe", r.id, rid));
                    break 'rel;
                }
            }
        }
    }
    out.push(check("related ids resolve", rel));

    // 4. every LOCALLY-declared topic/recipe has its `<id>.md` on disk in THIS
    //    profile. Inherited-only docs need not exist locally.
    let local_topics = crate::knowledge::conventions(kn, id).unwrap_or_default();
    let local_recipes = crate::knowledge::recipes(kn, id).unwrap_or_default();
    let mut files: Result<String, String> = Ok("all local convention/recipe files present".into());
    'files: {
        for t in &local_topics {
            if !dir.join("conventions").join(format!("{}.md", t.id)).exists() {
                files = Err(format!("convention '{}' has no conventions/{}.md", t.id, t.id));
                break 'files;
            }
        }
        for r in &local_recipes {
            if !dir.join("recipes").join(format!("{}.md", r.id)).exists() {
                files = Err(format!("recipe '{}' has no recipes/{}.md", r.id, r.id));
                break 'files;
            }
        }
    }
    out.push(check("doc files present", files));

    out.push(Check { name: "extends chain".into(), ok: true, detail: chain_detail });
    out
}

/// Rewrite a parent's `profile.yaml` text for a child: set `id:` to the child,
/// insert `extends: <parent>` right after it, and retitle. Other lines (flows,
/// review_map, fact_families, exec) are copied verbatim.
fn reid_with_extends(parent_yaml: &str, id: &str, parent: &str) -> String {
    let mut out = String::new();
    let mut did_id = false;
    for line in parent_yaml.lines() {
        let trimmed = line.trim_start();
        if !did_id && trimmed.starts_with("id:") {
            out.push_str(&format!("id: {id}\n"));
            out.push_str(&format!("extends: {parent}\n"));
            did_id = true;
            continue;
        }
        if trimmed.starts_with("title:") {
            out.push_str(&format!("title: \"{id} profile\"\n"));
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !did_id {
        out = format!("id: {id}\nextends: {parent}\n{out}");
    }
    out
}

/// Scaffold a profile under `kn/profiles/<id>/`. With `extends`, the manifest
/// (profile.yaml minus knowledge) and `extractors.yaml` (+ `extractors/`) are
/// copy-seeded from the parent so the child is immediately valid; conventions
/// and recipes are left empty (they are live-inherited). Refuses if `id` exists.
pub fn scaffold_new(kn: &Path, id: &str, extends: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let ok_id = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok_id {
        return Err(format!("invalid profile id '{id}' — use only [a-z0-9_-]"));
    }
    let dir = kn.join("profiles").join(id);
    if dir.exists() {
        return Err(format!("profile '{id}' already exists at {}", dir.display()));
    }

    let default_profile = format!(
        "schema_version: \"1.0\"\nid: {id}\ntitle: \"{id} profile\"\nlanguages: []\n\nfact_families:\n  - {{ id: symbol, symbol: true }}\n\nflows:\n  bugfix:   [code.recent, symbol.find]\n  feature:  [recipe(feature)]\n  refactor: [convention(architecture)]\n  review:   [diff.scan, convention(by-file-kind)]\n\nreview_map:\n  symbol: [architecture]\n"
    );
    let default_extractors =
        "schema_version: \"1.0\"\nignore_dirs: [\".git\", \".palugada\", \"target\", \"node_modules\", \"build\"]\n\nfamilies:\n  - id: symbol\n    regex: 'class\\s+(?P<name>\\w+)'\n".to_string();

    fs::create_dir_all(dir.join("conventions")).map_err(|e| format!("create dirs: {e}"))?;
    fs::create_dir_all(dir.join("recipes")).map_err(|e| format!("create dirs: {e}"))?;

    let mut written: Vec<PathBuf> = Vec::new();

    let (profile_yaml, extractors_yaml) = match extends {
        Some(parent) => {
            let pdir = kn.join("profiles").join(parent);
            if !pdir.join("profile.yaml").is_file() {
                return Err(format!("base profile '{parent}' does not exist at {}", pdir.display()));
            }
            let praw = fs::read_to_string(pdir.join("profile.yaml"))
                .map_err(|e| format!("read parent profile.yaml: {e}"))?;
            let child_yaml = reid_with_extends(&praw, id, parent);
            let pextr = fs::read_to_string(pdir.join("extractors.yaml")).unwrap_or(default_extractors);
            // copy parent extractors/*.scm if present
            let scm_dir = pdir.join("extractors");
            if scm_dir.is_dir() {
                fs::create_dir_all(dir.join("extractors")).map_err(|e| format!("create extractors dir: {e}"))?;
                for e in fs::read_dir(&scm_dir).map_err(|e| format!("read parent extractors/: {e}"))? {
                    let e = e.map_err(|e| e.to_string())?;
                    let from = e.path();
                    if from.is_file() {
                        let to = dir.join("extractors").join(e.file_name());
                        fs::copy(&from, &to).map_err(|e| format!("copy scm: {e}"))?;
                        written.push(to);
                    }
                }
            }
            (child_yaml, pextr)
        }
        None => (default_profile, default_extractors),
    };

    let conv_index = "{\n  \"schema_version\": \"1.0\",\n  \"topics\": []\n}\n";
    let recipe_index = "{\n  \"schema_version\": \"1.0\",\n  \"recipes\": []\n}\n";
    let files = [
        (dir.join("profile.yaml"), profile_yaml),
        (dir.join("extractors.yaml"), extractors_yaml),
        (dir.join("conventions").join("_index.json"), conv_index.to_string()),
        (dir.join("recipes").join("_index.json"), recipe_index.to_string()),
    ];
    for (path, contents) in files {
        fs::write(&path, &contents).map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(path);
    }
    written.sort();
    Ok(written)
}

fn valid_flow_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Replace the `flows:` block of a profile's `profile.yaml` with `flows`,
/// preserving every other line (comments, description, fact_families,
/// review_map). Flow names must be `[a-z0-9_-]`; steps are written verbatim.
pub fn set_flows(kn: &Path, id: &str, flows: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    for name in flows.keys() {
        if !valid_flow_name(name) {
            return Err(format!("invalid flow name '{name}' — use only [a-z0-9_-]"));
        }
    }
    let path = kn.join("profiles").join(id).join("profile.yaml");
    let raw = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;

    let mut block = String::from("flows:\n");
    for (name, steps) in flows {
        block.push_str(&format!("  {}: [{}]\n", name, steps.join(", ")));
    }

    let lines: Vec<&str> = raw.lines().collect();
    let new_content = if let Some(start) = lines.iter().position(|l| l.trim_end() == "flows:") {
        // Replace `flows:` + the contiguous indented lines that follow it.
        let mut end = start + 1;
        while end < lines.len() && lines[end].starts_with([' ', '\t']) {
            end += 1;
        }
        let mut out = String::new();
        for l in &lines[..start] {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&block);
        for l in &lines[end..] {
            out.push_str(l);
            out.push('\n');
        }
        out
    } else if let Some(rm) = lines.iter().position(|l| l.trim_end() == "review_map:") {
        // No flows block yet: insert before review_map.
        let mut out = String::new();
        for l in &lines[..rm] {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&block);
        out.push('\n');
        for l in &lines[rm..] {
            out.push_str(l);
            out.push('\n');
        }
        out
    } else {
        // Append at end.
        let mut out = raw.clone();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&block);
        out
    };
    fs::write(&path, new_content).map_err(|e| format!("write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_flows_replaces_block_and_preserves_rest() {
        let kn = tempfile::tempdir().unwrap();
        let dir = kn.path().join("profiles").join("p");
        std::fs::create_dir_all(&dir).unwrap();
        let original = "schema_version: \"1.0\"\nid: p\ntitle: \"P\"\nlanguages: [kotlin]\n\nfact_families:\n  - { id: vm, symbol: true }\n\n# retrieval flows comment\nflows:\n  bugfix:   [code.recent, convention(errorhandling)]\n  review:   [diff.scan, convention(by-file-kind)]\n\n# review map comment\nreview_map:\n  vm: [architecture]\n";
        std::fs::write(dir.join("profile.yaml"), original).unwrap();

        let mut flows = BTreeMap::new();
        flows.insert("bugfix".to_string(), vec!["code.recent".to_string(), "convention(errorhandling)".to_string(), "convention(r8-analyzer)".to_string()]);
        flows.insert("optimize".to_string(), vec!["convention(r8-analyzer)".to_string()]);
        set_flows(kn.path(), "p", &flows).unwrap();

        let out = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
        assert!(out.contains("bugfix: [code.recent, convention(errorhandling), convention(r8-analyzer)]"), "{out}");
        assert!(out.contains("optimize: [convention(r8-analyzer)]"));
        assert!(!out.contains("review:   [diff.scan"), "old review flow line removed");
        assert!(out.contains("# retrieval flows comment"));
        assert!(out.contains("# review map comment"));
        assert!(out.contains("review_map:\n  vm: [architecture]"));
        assert!(out.contains("fact_families:\n  - { id: vm, symbol: true }"));
        assert!(out.contains("languages: [kotlin]"));
    }

    #[test]
    fn set_flows_rejects_bad_flow_name() {
        let kn = tempfile::tempdir().unwrap();
        let dir = kn.path().join("profiles").join("p");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("profile.yaml"), "id: p\nflows:\n  bugfix: [diff.scan]\n").unwrap();
        let mut flows = BTreeMap::new();
        flows.insert("Bad Name".to_string(), vec!["diff.scan".to_string()]);
        assert!(set_flows(kn.path(), "p", &flows).is_err());
    }

    #[test]
    fn set_flows_inserts_block_when_absent() {
        let kn = tempfile::tempdir().unwrap();
        let dir = kn.path().join("profiles").join("p");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("profile.yaml"), "id: p\nlanguages: [kotlin]\n\nreview_map:\n  vm: [a]\n").unwrap();
        let mut flows = BTreeMap::new();
        flows.insert("bugfix".to_string(), vec!["diff.scan".to_string()]);
        set_flows(kn.path(), "p", &flows).unwrap();
        let out = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
        assert!(out.contains("flows:\n  bugfix: [diff.scan]"), "{out}");
        assert!(out.contains("review_map:"));
    }

    #[test]
    fn list_finds_profiles_with_titles() {
        let kn = tempfile::tempdir().unwrap();
        let p = kn.path().join("profiles").join("demo");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("profile.yaml"), "id: demo\ntitle: \"Demo Stack\"\n").unwrap();
        // a dir without profile.yaml is ignored
        fs::create_dir_all(kn.path().join("profiles").join("notaprofile")).unwrap();
        assert_eq!(list(kn.path()).unwrap(), vec![("demo".to_string(), "Demo Stack".to_string())]);
    }

    #[test]
    fn validate_flags_broken_extractors() {
        let kn = tempfile::tempdir().unwrap();
        let p = kn.path().join("profiles").join("broken");
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("profile.yaml"), "id: broken\nfact_families:\n  - { id: symbol, symbol: true }\n").unwrap();
        fs::write(p.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: '([unclosed'\n").unwrap();
        let checks = validate(kn.path(), "broken");
        let ex = checks.iter().find(|c| c.name == "extractors.yaml").unwrap();
        assert!(!ex.ok, "broken regex should fail validation: {}", ex.detail);
    }

    #[test]
    fn validate_flags_dangling_recipe_section_ref() {
        let kn = tempfile::tempdir().unwrap();
        let p = kn.path().join("profiles").join("d");
        fs::create_dir_all(p.join("conventions")).unwrap();
        fs::create_dir_all(p.join("recipes")).unwrap();
        fs::write(p.join("profile.yaml"), "id: d\nfact_families:\n  - { id: symbol, symbol: true }\n").unwrap();
        fs::write(p.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
        fs::write(
            p.join("conventions/_index.json"),
            r#"{"topics":[{"id":"arch","title":"Arch","sections":[{"id":"o","title":"Overview","tokens":10}]}]}"#,
        ).unwrap();
        fs::write(p.join("conventions/arch.md"), "# Arch\n").unwrap();
        // recipe points at a section id that does not exist on `arch`
        fs::write(
            p.join("recipes/_index.json"),
            r#"{"recipes":[{"id":"feat","title":"Feat","convention_refs":[{"topic":"arch","section":"nope"}]}]}"#,
        ).unwrap();
        fs::write(p.join("recipes/feat.md"), "# Feat\n").unwrap();

        let checks = validate(kn.path(), "d");
        let c = checks.iter().find(|c| c.name == "recipe cross-refs resolve").unwrap();
        assert!(!c.ok, "a dangling section ref must fail validation: {}", c.detail);
    }

    #[test]
    fn new_then_validate_round_trips() {
        let kn = tempfile::tempdir().unwrap();
        scaffold_new(kn.path(), "fresh", None).unwrap();
        let checks = validate(kn.path(), "fresh");
        for c in &checks {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
        // scaffolding again refuses
        assert!(scaffold_new(kn.path(), "fresh", None).is_err());
        // and the new profile shows up in list
        assert!(list(kn.path()).unwrap().iter().any(|(id, _)| id == "fresh"));
    }

    #[test]
    fn scaffold_with_extends_seeds_manifest_and_inherits_knowledge() {
        let kn = tempfile::tempdir().unwrap();
        // parent with a real manifest + extractors + a convention
        let p = kn.path().join("profiles").join("android-mvvm");
        fs::create_dir_all(p.join("conventions")).unwrap();
        fs::create_dir_all(p.join("recipes")).unwrap();
        fs::write(p.join("profile.yaml"),
            "schema_version: \"1.0\"\nid: android-mvvm\ntitle: \"MVVM\"\nlanguages: [kotlin]\nfact_families:\n  - { id: viewmodel, symbol: true }\nflows:\n  feature: [recipe(feature)]\nreview_map:\n  viewmodel: [architecture]\n").unwrap();
        fs::write(p.join("extractors.yaml"), "families:\n  - id: viewmodel\n    regex: 'x'\n").unwrap();
        fs::write(p.join("conventions/_index.json"),
            r#"{"topics":[{"id":"architecture","title":"Arch","sections":[{"id":"layers","title":"Layers","tokens":10}]}]}"#).unwrap();
        fs::write(p.join("conventions/architecture.md"), "# Arch\n## Layers {#layers}\nx\n").unwrap();
        fs::write(p.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();

        scaffold_new(kn.path(), "android-mvi", Some("android-mvvm")).unwrap();

        let child_yaml = fs::read_to_string(kn.path().join("profiles/android-mvi/profile.yaml")).unwrap();
        assert!(child_yaml.contains("id: android-mvi"));
        assert!(child_yaml.contains("extends: android-mvvm"));
        assert!(child_yaml.contains("viewmodel"), "manifest fact_families copy-seeded");
        // knowledge left empty (live inheritance)
        let conv_idx = fs::read_to_string(kn.path().join("profiles/android-mvi/conventions/_index.json")).unwrap();
        assert!(conv_idx.contains("\"topics\": []"));
        // child validates AND sees the inherited `architecture` topic via merge
        for c in validate(kn.path(), "android-mvi") {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
        let merged = crate::inherit::merged_conventions(kn.path(), "android-mvi").unwrap();
        assert!(merged.iter().any(|t| t.id == "architecture"), "inherited topic visible");
    }

    #[test]
    fn scaffold_with_missing_base_errors() {
        let kn = tempfile::tempdir().unwrap();
        assert!(scaffold_new(kn.path(), "child", Some("ghost")).is_err());
    }

    /// Minimal valid profile dir (profile.yaml + extractors.yaml + empty indexes).
    fn base_profile(kn: &Path, id: &str, extends: Option<&str>) {
        let dir = kn.join("profiles").join(id);
        fs::create_dir_all(dir.join("conventions")).unwrap();
        fs::create_dir_all(dir.join("recipes")).unwrap();
        let mut y = format!("id: {id}\nfact_families:\n  - {{ id: symbol, symbol: true }}\n");
        if let Some(e) = extends {
            y.push_str(&format!("extends: {e}\n"));
        }
        fs::write(dir.join("profile.yaml"), y).unwrap();
        fs::write(dir.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
        fs::write(dir.join("conventions/_index.json"), r#"{"topics":[]}"#).unwrap();
        fs::write(dir.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();
    }

    #[test]
    fn validate_child_resolves_inherited_cross_refs() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "parent", None);
        // parent owns `architecture` with section `data-flow`
        fs::write(
            kn.path().join("profiles/parent/conventions/_index.json"),
            r#"{"topics":[{"id":"architecture","title":"Arch","sections":[{"id":"data-flow","title":"Data Flow","tokens":10}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/parent/conventions/architecture.md"),
            "# Arch\n## Data Flow {#data-flow}\nx\n").unwrap();

        base_profile(kn.path(), "child", Some("parent"));
        // child owns ONLY a recipe that references the INHERITED architecture#data-flow
        fs::write(
            kn.path().join("profiles/child/recipes/_index.json"),
            r#"{"recipes":[{"id":"feature","title":"Feature","convention_refs":[{"topic":"architecture","section":"data-flow"}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/child/recipes/feature.md"), "# Feature\n").unwrap();

        let checks = validate(kn.path(), "child");
        for c in &checks {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
    }

    #[test]
    fn validate_fails_on_cycle() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "a", Some("b"));
        base_profile(kn.path(), "b", Some("a"));
        let checks = validate(kn.path(), "a");
        let c = checks.iter().find(|c| c.name == "extends chain").unwrap();
        assert!(!c.ok && c.detail.contains("cycle"), "{}", c.detail);
    }

    #[test]
    fn validate_fails_on_missing_parent() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "child", Some("ghost"));
        let checks = validate(kn.path(), "child");
        let c = checks.iter().find(|c| c.name == "extends chain").unwrap();
        assert!(!c.ok && c.detail.contains("ghost"), "{}", c.detail);
    }
}
