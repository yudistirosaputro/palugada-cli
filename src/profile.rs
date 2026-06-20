//! Profile authoring tooling — list, validate, and scaffold stack profiles.
//! All helpers take the knowledge dir `kn` so they're testable against a temp
//! directory; the engine's own readers (`indexer::load_families`,
//! `indexer::fact_families`) are reused so validation matches runtime behaviour.

use serde::Deserialize;
use std::collections::BTreeMap;
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

    checks
}

/// Scaffold a minimal but valid profile under `kn/profiles/<id>/`. Refuses if it
/// already exists. Returns the written file paths.
pub fn scaffold_new(kn: &Path, id: &str) -> Result<Vec<PathBuf>, String> {
    let ok_id = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok_id {
        return Err(format!("invalid profile id '{id}' — use only [a-z0-9_-]"));
    }
    let dir = kn.join("profiles").join(id);
    if dir.exists() {
        return Err(format!("profile '{id}' already exists at {}", dir.display()));
    }
    fs::create_dir_all(dir.join("conventions")).map_err(|e| format!("create dirs: {e}"))?;
    fs::create_dir_all(dir.join("recipes")).map_err(|e| format!("create dirs: {e}"))?;

    let profile_yaml = format!(
        "schema_version: \"1.0\"\nid: {id}\ntitle: \"{id} profile\"\nlanguages: []\n\nfact_families:\n  - {{ id: symbol, symbol: true }}\n\nflows:\n  bugfix:   [code.recent, symbol.find]\n  feature:  [recipe(feature)]\n  refactor: [convention(architecture)]\n  review:   [diff.scan, convention(by-file-kind)]\n\nreview_map:\n  symbol: [architecture]\n"
    );
    let extractors_yaml =
        "schema_version: \"1.0\"\nignore_dirs: [\".git\", \".palugada\", \"target\", \"node_modules\", \"build\"]\n\nfamilies:\n  - id: symbol\n    regex: 'class\\s+(?P<name>\\w+)'\n";
    let conv_index = "{\n  \"schema_version\": \"1.0\",\n  \"topics\": []\n}\n";
    let recipe_index = "{\n  \"schema_version\": \"1.0\",\n  \"recipes\": []\n}\n";

    let files = [
        (dir.join("profile.yaml"), profile_yaml.as_str()),
        (dir.join("extractors.yaml"), extractors_yaml),
        (dir.join("conventions").join("_index.json"), conv_index),
        (dir.join("recipes").join("_index.json"), recipe_index),
    ];
    let mut written = Vec::new();
    for (path, contents) in files {
        fs::write(&path, contents).map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(path);
    }
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
    fn new_then_validate_round_trips() {
        let kn = tempfile::tempdir().unwrap();
        scaffold_new(kn.path(), "fresh").unwrap();
        let checks = validate(kn.path(), "fresh");
        for c in &checks {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
        // scaffolding again refuses
        assert!(scaffold_new(kn.path(), "fresh").is_err());
        // and the new profile shows up in list
        assert!(list(kn.path()).unwrap().iter().any(|(id, _)| id == "fresh"));
    }
}
