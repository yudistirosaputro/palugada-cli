# Per-project Convention Overlay + Effective-Rules Merge — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a project add/override/remap conventions for itself only (stored in its repo `.palugada/`, committed & shared via git), with `brief`, the web detail page, and a CLI inspector all resolving against the merged "effective rules" (profile + overlay).

**Architecture:** Mirror the profile's on-disk convention layout inside the repo so the existing reader/writer is reused via new `*_in(dir, …)` accessors. A new pure module `effective.rs` owns the merge (conventions by id; `review_map` replace-per-family) and an I/O `effective_rules` resolver. `brief` reads the overlay dir + the project `review_map` override; the web page edits the overlay; `palugada project rules` prints it.

**Tech Stack:** Rust (single binary), `serde`/`serde_yaml`/`serde_json`, `tempfile` for tests, vanilla JS for the web console.

## Global Constraints

- Overlay convention files use the **same format** as profile conventions: `<id>.md` (front-matter `id`/`title`/`description`/`sections`/`tags` + markdown body) + a `_index.json` sibling.
- Overlay lives at `<repo>/.palugada/conventions/`; the `review_map` override lives in `<repo>/.palugada/config.yaml`. Both are committed with the repo. **No secrets** are touched by this feature.
- `review_map` merge is **replace-per-family**: a family present in the overlay replaces that family's profile list entirely; absent families keep the profile's list.
- Convention id validation reuses the existing `[a-z0-9_-]` rule (`validate_doc_id`).
- Every commit message ends with the trailer `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Run `cargo test` (whole suite) at each task's verify step — existing 84 tests must stay green.

---

### Task 1: Dir-parameterized convention accessors in `knowledge.rs`

Extract the conventions-directory core so both profile and overlay reuse it. The `_in` functions take the **conventions directory itself** (e.g. `kn/profiles/<id>/conventions` or `<repo>/.palugada/conventions`).

**Files:**
- Modify: `src/knowledge.rs`
- Test: `src/knowledge.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub fn conventions_in(conv_dir: &Path) -> Result<Vec<TopicMeta>, String>` — reads `conv_dir/_index.json`; **missing dir/file → `Ok(vec![])`**.
  - `pub fn convention_md_in(conv_dir: &Path, id: &str) -> Result<String, String>`
  - `pub fn convention_outline_in(conv_dir: &Path, id: &str) -> Result<String, String>`
  - `pub fn add_convention_in(conv_dir: &Path, spec: &ConventionSpec) -> Result<(), String>`
  - `pub fn set_convention_body_in(conv_dir: &Path, id: &str, markdown: &str) -> Result<(), String>`
- Existing `conventions`, `convention_md`, `convention_outline`, `add_convention`, `set_convention_body` become shims that compute `kn.join("profiles").join(profile).join("conventions")` and delegate.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/knowledge.rs`:

```rust
#[test]
fn conventions_in_missing_dir_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("conventions"); // does not exist
    assert!(conventions_in(&dir).unwrap().is_empty());
}

#[test]
fn add_convention_in_then_read_and_override_body() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("conventions");
    let spec = ConventionSpec {
        id: "ours".into(),
        title: "Ours".into(),
        description: "team rule".into(),
        tags: vec!["kt".into()],
        sections: vec![SectionSpec { title: "Rule".into(), body: "do X".into(), code: false }],
    };
    add_convention_in(&dir, &spec).unwrap();
    let metas = conventions_in(&dir).unwrap();
    assert_eq!(metas.len(), 1);
    assert_eq!(metas[0].id, "ours");
    assert!(convention_md_in(&dir, "ours").unwrap().contains("do X"));

    set_convention_body_in(&dir, "ours", "---\nid: ours\n---\n# Ours\nnew body\n").unwrap();
    assert!(convention_md_in(&dir, "ours").unwrap().contains("new body"));

    // edit-only: unknown id errors
    assert!(set_convention_body_in(&dir, "nope", "x").is_err());
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --lib conventions_in_missing_dir_is_empty add_convention_in_then`
Expected: FAIL (functions not found).

- [ ] **Step 3: Implement the `_in` accessors**

In `src/knowledge.rs`, add a dir-taking index reader and the `_in` functions, then refactor the existing functions to delegate. Replace `read_conv_index` usage where it builds the path.

```rust
fn read_conv_index_in(conv_dir: &Path) -> Result<ConvIndex, String> {
    let p = conv_dir.join("_index.json");
    if !p.exists() {
        return Ok(ConvIndex::default());
    }
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
}

/// Conventions in an arbitrary conventions dir (profile or per-project overlay).
/// A missing dir/index yields an empty list (a project with no overlay).
pub fn conventions_in(conv_dir: &Path) -> Result<Vec<TopicMeta>, String> {
    Ok(read_conv_index_in(conv_dir)?
        .topics
        .into_iter()
        .map(|t| TopicMeta {
            id: t.id,
            title: t.title,
            description: t.description,
            tags: t.tags,
            sections: t.sections.into_iter().map(|s| s.title).collect(),
        })
        .collect())
}

pub fn convention_md_in(conv_dir: &Path, id: &str) -> Result<String, String> {
    let p = conv_dir.join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

pub fn convention_outline_in(conv_dir: &Path, id: &str) -> Result<String, String> {
    let raw = convention_md_in(conv_dir, id)
        .map_err(|_| format!("no convention '{id}' in {}", conv_dir.display()))?;
    Ok(convention_outline_str(&raw, id))
}

pub fn add_convention_in(conv_dir: &Path, spec: &ConventionSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    fs::create_dir_all(conv_dir).map_err(|e| format!("create {}: {e}", conv_dir.display()))?;

    let mut fm = format!(
        "---\nid: {}\ntitle: {}\ndescription: {}\nsections:\n",
        spec.id,
        yaml_scalar(&spec.title),
        yaml_scalar(&spec.description)
    );
    let mut body = format!("# {}\n", spec.title);
    let mut sec_meta: Vec<serde_json::Value> = Vec::new();
    for s in &spec.sections {
        let sid = slug(&s.title);
        let tokens = s.body.len() / 4 + 8;
        fm.push_str(&format!(
            "  - {{ id: {}, title: {}, tokens: {}, code: {} }}\n",
            sid,
            yaml_scalar(&s.title),
            tokens,
            s.code
        ));
        body.push_str(&format!("\n## {} {{#{}}}\n{}\n", s.title, sid, s.body.trim()));
        sec_meta.push(serde_json::json!({ "id": sid, "title": s.title, "tokens": tokens }));
    }
    fm.push_str(&format!("tags: [{}]\n---\n\n", spec.tags.join(", ")));
    fs::write(conv_dir.join(format!("{}.md", spec.id)), format!("{fm}{body}"))
        .map_err(|e| format!("write convention: {e}"))?;

    let entry = serde_json::json!({
        "id": spec.id, "title": spec.title, "file": format!("{}.md", spec.id),
        "description": spec.description, "tags": spec.tags, "sections": sec_meta,
    });
    upsert_index(&conv_dir.join("_index.json"), "topics", &spec.id, entry)
}

pub fn set_convention_body_in(conv_dir: &Path, id: &str, markdown: &str) -> Result<(), String> {
    validate_doc_id(id)?;
    let p = conv_dir.join(format!("{id}.md"));
    if !p.exists() {
        return Err(format!("convention '{id}' does not exist in {}", conv_dir.display()));
    }
    fs::write(&p, markdown).map_err(|e| format!("write {}: {e}", p.display()))
}
```

Refactor the existing profile-based functions to delegate:

```rust
pub fn conventions(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    conventions_in(&kn.join("profiles").join(profile).join("conventions"))
}

pub fn convention_md(kn: &Path, profile: &str, id: &str) -> Result<String, String> {
    convention_md_in(&kn.join("profiles").join(profile).join("conventions"), id)
}

pub fn convention_outline(kn: &Path, profile: &str, name: &str) -> Result<String, String> {
    convention_outline_in(&kn.join("profiles").join(profile).join("conventions"), name)
}

pub fn add_convention(kn: &Path, profile: &str, spec: &ConventionSpec) -> Result<(), String> {
    add_convention_in(&kn.join("profiles").join(profile).join("conventions"), spec)
}

pub fn set_convention_body(kn: &Path, profile: &str, id: &str, markdown: &str) -> Result<(), String> {
    set_convention_body_in(&kn.join("profiles").join(profile).join("conventions"), id, markdown)
}
```

Notes: `read_conv_index(kn, profile)` is still used by `list_topics`/`search`/`query` — leave those callers; either keep `read_conv_index` delegating to `read_conv_index_in`, or leave it as-is. Keep `set_doc_body` for the recipe path (`set_recipe_body` still uses it); `set_convention_body` no longer routes through it.

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --lib`
Expected: new tests PASS, all existing knowledge tests still PASS.

- [ ] **Step 5: Commit**

```bash
git add src/knowledge.rs
git commit -m "$(printf 'feat(knowledge): dir-parameterized convention accessors (profile/overlay reuse)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 2: `effective.rs` — types + pure merge functions

**Files:**
- Create: `src/effective.rs`
- Modify: `src/main.rs` (add `mod effective;` near the other `mod` lines)
- Test: `src/effective.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::knowledge::TopicMeta { id, title, description, tags, sections }`.
- Produces:
  - `pub enum Origin { Profile, Project, Overridden }` (serializes lowercase).
  - `pub struct EffectiveConvention { id, title, description, tags: Vec<String>, origin: Origin }`
  - `pub struct EffectiveReviewEntry { family: String, conventions: Vec<String>, origin: Origin }`
  - `pub fn merge_conventions(profile: &[TopicMeta], overlay: &[TopicMeta]) -> Vec<EffectiveConvention>`
  - `pub fn apply_review_overlay(profile: &BTreeMap<String,Vec<String>>, overlay: &BTreeMap<String,Vec<String>>) -> BTreeMap<String,Vec<String>>` — replace-per-family merge (used by `brief` and by `merge_review_map`).
  - `pub fn merge_review_map(profile: &BTreeMap<String,Vec<String>>, overlay: &BTreeMap<String,Vec<String>>) -> Vec<EffectiveReviewEntry>`
  - `pub fn dangling_review_refs(review: &[EffectiveReviewEntry], known_ids: &BTreeSet<String>) -> Vec<String>`

- [ ] **Step 1: Write the failing tests**

Create `src/effective.rs` with:

```rust
//! Per-project convention overlay + effective-rules merge (web cycle C).

use crate::knowledge::TopicMeta;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str) -> TopicMeta {
        TopicMeta { id: id.into(), title: id.into(), description: String::new(), tags: vec![], sections: vec![] }
    }

    #[test]
    fn merge_conventions_classifies_origin() {
        let profile = vec![meta("architecture"), meta("errorhandling")];
        let overlay = vec![meta("architecture"), meta("ours")];
        let eff = merge_conventions(&profile, &overlay);
        let by: BTreeMap<_, _> = eff.iter().map(|c| (c.id.clone(), &c.origin)).collect();
        assert_eq!(by["architecture"], &Origin::Overridden);
        assert_eq!(by["errorhandling"], &Origin::Profile);
        assert_eq!(by["ours"], &Origin::Project);
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
        let by: BTreeMap<_, _> = eff.iter().map(|e| (e.family.clone(), &e.origin)).collect();
        assert_eq!(by["viewmodel"], &Origin::Project);
        assert_eq!(by["repository"], &Origin::Profile);
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
}
```

- [ ] **Step 2: Run tests, verify they fail**

First add `mod effective;` to `src/main.rs` (so it compiles). Then:
Run: `cargo test --lib effective::`
Expected: FAIL (types/functions not defined).

- [ ] **Step 3: Implement the pure layer**

Add above the test module in `src/effective.rs`:

```rust
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

/// Merge conventions by id: overlay-over-profile id → Overridden, overlay-only →
/// Project, profile-only → Profile. Profile order first, then overlay-only ids.
pub fn merge_conventions(profile: &[TopicMeta], overlay: &[TopicMeta]) -> Vec<EffectiveConvention> {
    let overlay_ids: BTreeSet<&str> = overlay.iter().map(|t| t.id.as_str()).collect();
    let profile_ids: BTreeSet<&str> = profile.iter().map(|t| t.id.as_str()).collect();
    let mut out: Vec<EffectiveConvention> = Vec::new();
    for t in profile {
        let origin = if overlay_ids.contains(t.id.as_str()) { Origin::Overridden } else { Origin::Profile };
        // when overridden, prefer the overlay's metadata
        let src = if origin == Origin::Overridden {
            overlay.iter().find(|o| o.id == t.id).unwrap()
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
/// family; absent families keep the profile's list.
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
pub fn dangling_review_refs(
    review: &[EffectiveReviewEntry],
    known_ids: &BTreeSet<String>,
) -> Vec<String> {
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
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --lib effective::`
Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/effective.rs src/main.rs
git commit -m "$(printf 'feat(effective): pure convention/review_map merge with origin tracking\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 3: `review_map` override on `ProjectConfig` + `set_review_map`

**Files:**
- Modify: `src/config.rs`
- Test: `src/config.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `ProjectConfig.review_map: BTreeMap<String, Vec<String>>` (default, skipped when empty).
  - `pub fn set_review_map(repo_path: &str, map: BTreeMap<String, Vec<String>>) -> Result<(), String>`

- [ ] **Step 1: Write the failing test**

Add to `src/config.rs` tests:

```rust
#[test]
fn project_config_review_map_roundtrips_and_sets() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().to_string_lossy().to_string();
    // baseline config without review_map parses, serializes without the key
    let pc = ProjectConfig { project: "p".into(), profile: "android-mvvm".into(), ..Default::default() };
    pc.save_to(&repo).unwrap();
    let loaded = ProjectConfig::load_from(&repo).unwrap();
    assert!(loaded.review_map.is_empty());

    let mut map = std::collections::BTreeMap::new();
    map.insert("viewmodel".to_string(), vec!["ours".to_string()]);
    set_review_map(&repo, map).unwrap();
    let loaded = ProjectConfig::load_from(&repo).unwrap();
    assert_eq!(loaded.review_map["viewmodel"], vec!["ours".to_string()]);
    assert_eq!(loaded.profile, "android-mvvm"); // other fields preserved
}
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --lib project_config_review_map_roundtrips_and_sets`
Expected: FAIL (no `review_map` field / `set_review_map`).

- [ ] **Step 3: Implement**

Add the field to `ProjectConfig` (after `exec`):

```rust
    /// Per-project review_map override (family → convention ids); replaces the
    /// profile's entry for each listed family. Empty → follow the profile.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub review_map: BTreeMap<String, Vec<String>>,
```

Add the setter near `set_profile`:

```rust
/// Set a project's review_map override by editing its `.palugada/config.yaml`.
pub fn set_review_map(repo_path: &str, map: BTreeMap<String, Vec<String>>) -> Result<(), String> {
    let mut pc = ProjectConfig::load_from(repo_path)?;
    pc.review_map = map;
    pc.save_to(repo_path)
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --lib project_config_review_map_roundtrips_and_sets && cargo test --lib`
Expected: new test PASS, full suite green.

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "$(printf 'feat(config): per-project review_map override + set_review_map\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 4: `effective_rules` I/O resolver + overlay-preferring outline

**Files:**
- Modify: `src/effective.rs`
- Test: `src/effective.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::config::{GlobalConfig, ProjectConfig}`, `crate::knowledge::{knowledge_dir, conventions, conventions_in, convention_outline, convention_outline_in}`.
- Produces:
  - `pub struct EffectiveRules { project, profile, conventions: Vec<EffectiveConvention>, review_map: Vec<EffectiveReviewEntry>, warnings: Vec<String> }`
  - `pub fn overlay_dir(repo_path: &str) -> PathBuf` → `<repo>/.palugada/conventions`
  - `pub fn profile_review_map(kn: &Path, profile: &str) -> Result<BTreeMap<String,Vec<String>>, String>` (reads `profile.yaml`).
  - `pub fn effective_rules(global: &GlobalConfig, name: &str) -> Result<EffectiveRules, String>`
  - `pub fn convention_outline_overlaid(kn: &Path, profile: &str, overlay: &Path, id: &str) -> String` — overlay body if present, else profile, else `(error)`.

- [ ] **Step 1: Write the failing test**

Add to `src/effective.rs` tests (helper builds a tempdir knowledge profile + a registered project repo with an overlay):

```rust
use crate::config::{GlobalConfig, ProjectConfig, ProjectEntry};

fn write(p: &std::path::Path, s: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, s).unwrap();
}

#[test]
fn effective_rules_merges_profile_and_overlay() {
    let home = tempfile::tempdir().unwrap();
    let kn = home.path().join("kn");
    // profile.yaml with review_map + flows
    write(&kn.join("profiles/p/profile.yaml"),
        "flows:\n  review: [diff.scan, convention(by-file-kind)]\nreview_map:\n  viewmodel: [architecture]\n");
    // one profile convention
    let arch = ConventionSpec { id: "architecture".into(), title: "Arch".into(), description: "d".into(), tags: vec![], sections: vec![] };
    crate::knowledge::add_convention_in(&kn.join("profiles/p/conventions"), &arch).unwrap();

    // project repo with overlay: new convention + review_map override
    let repo = home.path().join("repo");
    let ours = ConventionSpec { id: "ours".into(), title: "Ours".into(), description: "team".into(), tags: vec![], sections: vec![] };
    crate::knowledge::add_convention_in(&overlay_dir(repo.to_str().unwrap()), &ours).unwrap();
    let mut rm = BTreeMap::new();
    rm.insert("viewmodel".to_string(), vec!["ours".to_string()]);
    let pc = ProjectConfig { project: "app".into(), profile: "p".into(), review_map: rm, ..Default::default() };
    pc.save_to(repo.to_str().unwrap()).unwrap();

    let mut global = GlobalConfig::default();
    global.engine.knowledge_path = kn.to_string_lossy().to_string();
    global.projects.registered.insert("app".into(),
        ProjectEntry { repo_path: repo.to_string_lossy().to_string(), workspace: String::new() });

    let eff = effective_rules(&global, "app").unwrap();
    assert_eq!(eff.profile, "p");
    assert!(eff.conventions.iter().any(|c| c.id == "ours" && c.origin == Origin::Project));
    assert!(eff.conventions.iter().any(|c| c.id == "architecture" && c.origin == Origin::Profile));
    let vm = eff.review_map.iter().find(|e| e.family == "viewmodel").unwrap();
    assert_eq!(vm.conventions, vec!["ours".to_string()]);
    assert_eq!(vm.origin, Origin::Project);
}
```

> Confirmed against `skillmap`'s test: the knowledge dir comes from `global.engine.knowledge_path` (a `String`), and `ProjectEntry { repo_path, workspace }`. `knowledge::knowledge_dir` checks `PALUGADA_KNOWLEDGE` env first, but `cargo test` runs without it set (skillmap's resolver test relies on this), so the fixture above is sufficient.

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --lib effective_rules_merges_profile_and_overlay`
Expected: FAIL (resolver not defined).

- [ ] **Step 3: Implement the resolver**

Add to `src/effective.rs` (imports at top: `use std::path::{Path, PathBuf};`):

```rust
#[derive(serde::Serialize, Debug)]
pub struct EffectiveRules {
    pub project: String,
    pub profile: String,
    pub conventions: Vec<EffectiveConvention>,
    pub review_map: Vec<EffectiveReviewEntry>,
    pub warnings: Vec<String>,
}

pub fn overlay_dir(repo_path: &str) -> PathBuf {
    crate::config::expand_home(repo_path).join(".palugada").join("conventions")
}

#[derive(serde::Deserialize, Default)]
struct ProfileReview {
    #[serde(default)]
    review_map: BTreeMap<String, Vec<String>>,
}

pub fn profile_review_map(kn: &Path, profile: &str) -> Result<BTreeMap<String, Vec<String>>, String> {
    let p = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let pr: ProfileReview =
        serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))?;
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
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --lib effective::`
Expected: all `effective` tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/effective.rs
git commit -m "$(printf 'feat(effective): effective_rules resolver + overlay-preferring outline\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 5: `brief` resolves against the overlay

**Files:**
- Modify: `src/brief.rs`
- Test: `src/brief.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::effective::{overlay_dir, apply_review_overlay, convention_outline_overlaid}`, `crate::config::ProjectConfig`.
- Behavior change inside `brief::run`: the effective review_map drives `mapped_topics`, and convention rendering prefers the overlay body.

- [ ] **Step 1: Write the failing test**

`brief::run` writes to stdout, so test the seam, not the print. Add a test that the overlay review_map override changes `mapped_topics` output via `apply_review_overlay`, and that overlay outline wins. (These exercise the exact functions `run` will call.)

```rust
#[test]
fn overlay_review_map_overrides_profile_for_mapped_topics() {
    use std::collections::{BTreeMap, BTreeSet};
    let mut profile = BTreeMap::new();
    profile.insert("viewmodel".to_string(), vec!["architecture".to_string()]);
    let mut overlay = BTreeMap::new();
    overlay.insert("viewmodel".to_string(), vec!["ours".to_string()]);
    let merged = crate::effective::apply_review_overlay(&profile, &overlay);
    let touched: BTreeSet<String> = ["viewmodel".to_string()].into_iter().collect();
    let topics = mapped_topics(&merged, &touched);
    assert_eq!(topics, vec!["ours".to_string()]);
}

#[test]
fn overlay_outline_prefers_overlay_then_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let kn = tmp.path().join("kn");
    let prof_dir = kn.join("profiles/p/conventions");
    let spec = crate::knowledge::ConventionSpec {
        id: "architecture".into(), title: "Arch".into(), description: "profile body".into(),
        tags: vec![], sections: vec![],
    };
    crate::knowledge::add_convention_in(&prof_dir, &spec).unwrap();
    let overlay = tmp.path().join("repo/.palugada/conventions");
    let ov = crate::knowledge::ConventionSpec {
        id: "architecture".into(), title: "Arch".into(), description: "OVERLAY body".into(),
        tags: vec![], sections: vec![],
    };
    crate::knowledge::add_convention_in(&overlay, &ov).unwrap();
    let out = crate::effective::convention_outline_overlaid(&kn, "p", &overlay, "architecture");
    assert!(out.contains("OVERLAY"));
}
```

- [ ] **Step 2: Run tests, verify they fail/compile-fail**

Run: `cargo test --lib overlay_review_map_overrides_profile_for_mapped_topics overlay_outline_prefers`
Expected: FAIL until `run` is wired and imports compile (the second test only needs Task 1/4 code — should pass already; the first needs nothing new and should pass — if both pass, proceed to wire `run` and rely on e2e). The point of this task is the `run` wiring below.

- [ ] **Step 3: Wire `brief::run` to the overlay**

In `src/brief.rs`, near the top of `run` after `pf` is parsed, compute the overlay context:

```rust
    // Per-project overlay: review_map override + overlay convention bodies.
    let repo_str = repo.to_string_lossy().to_string();
    let overlay = crate::effective::overlay_dir(&repo_str);
    let review_map = match crate::config::ProjectConfig::load_from(&repo_str) {
        Ok(pc) => crate::effective::apply_review_overlay(&pf.review_map, &pc.review_map),
        Err(_) => pf.review_map.clone(), // brief works without a project config
    };
    let outline = |id: &str| crate::effective::convention_outline_overlaid(kn, profile, &overlay, id);
```

Then replace the two convention render sites:
- `let topics = mapped_topics(&pf.review_map, &ctx.touched_families);` → `let topics = mapped_topics(&review_map, &ctx.touched_families);`
- inside the `by-file-kind` map: `knowledge::convention_outline(kn, profile, t)...` → `outline(t)` (drop the surrounding `format!("### {t}\n{}", ...)` wrapper unchanged: `format!("### {t}\n{}", outline(t))`).
- the plain `"convention" => (... knowledge::convention_outline(kn, profile, &arg)...)` → use `outline(&arg)`.

(The closure borrows `kn`/`profile`/`overlay` immutably; if the borrow checker complains about `outline` capturing while `ctx` mutates, inline the call instead of the closure.)

- [ ] **Step 4: Run tests + build, verify pass**

Run: `cargo test --lib && cargo build`
Expected: green build, tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/brief.rs
git commit -m "$(printf 'feat(brief): resolve conventions/review_map against per-project overlay\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 6: web routes + handlers

**Files:**
- Modify: `src/web.rs`
- Test: `src/web.rs` (inline route test)

**Interfaces:**
- New `Route` variants: `ProjectRules(String)`, `AddOverlayConvention(String)`, `SetOverlayConventionBody(String, String)`, `SetOverlayReviewMap(String)`.

- [ ] **Step 1: Write the failing route test**

Add asserts to `route_parses_paths()` in `src/web.rs`:

```rust
    assert_eq!(route("GET", "/api/project/app/rules"), Route::ProjectRules("app".into()));
    assert_eq!(route("POST", "/api/project/app/convention"), Route::AddOverlayConvention("app".into()));
    assert_eq!(
        route("POST", "/api/project/app/convention/architecture/body"),
        Route::SetOverlayConventionBody("app".into(), "architecture".into())
    );
    assert_eq!(route("POST", "/api/project/app/review-map"), Route::SetOverlayReviewMap("app".into()));
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --lib route_parses_paths`
Expected: FAIL (variants/patterns missing).

- [ ] **Step 3: Add variants, route patterns, handlers**

In the `Route` enum (`src/web.rs`):

```rust
    ProjectRules(String),
    AddOverlayConvention(String),
    SetOverlayConventionBody(String, String),
    SetOverlayReviewMap(String),
```

In `fn route`, before the `_ => Route::NotFound` arm:

```rust
        ("GET", ["api", "project", name, "rules"]) => Route::ProjectRules((*name).to_string()),
        ("POST", ["api", "project", name, "convention"]) => Route::AddOverlayConvention((*name).to_string()),
        ("POST", ["api", "project", name, "convention", id, "body"]) => {
            Route::SetOverlayConventionBody((*name).to_string(), (*id).to_string())
        }
        ("POST", ["api", "project", name, "review-map"]) => Route::SetOverlayReviewMap((*name).to_string()),
```

In the dispatch `match` (near the other project routes), add handlers. Add a small repo-resolver helper near the other web helpers:

```rust
fn project_repo(name: &str) -> Result<(crate::config::GlobalConfig, String), String> {
    let global = crate::config::GlobalConfig::load_or_default()?;
    let name = crate::http::decode_segment(name);
    let repo = global
        .projects
        .registered
        .get(&name)
        .ok_or_else(|| format!("project '{name}' is not registered"))?
        .repo_path
        .clone();
    Ok((global, repo))
}
```

Handlers:

```rust
        Route::ProjectRules(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            Ok(jv(&crate::effective::effective_rules(&global, &name)?))
        }),
        Route::AddOverlayConvention(name) => write_op(|| {
            let (_g, repo) = project_repo(&name)?;
            let spec: crate::knowledge::ConventionSpec =
                serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::add_convention_in(&crate::effective::overlay_dir(&repo), &spec)?;
            Ok(json!({ "ok": true, "id": spec.id }))
        }),
        Route::SetOverlayConventionBody(name, id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req { markdown: String }
            let (_g, repo) = project_repo(&name)?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::knowledge::set_convention_body_in(&crate::effective::overlay_dir(&repo), &id, &req.markdown)?;
            Ok(json!({ "ok": true, "id": id }))
        }),
        Route::SetOverlayReviewMap(name) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                #[serde(default)]
                review_map: std::collections::BTreeMap<String, Vec<String>>,
            }
            let (_g, repo) = project_repo(&name)?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::config::set_review_map(&repo, req.review_map)?;
            Ok(json!({ "ok": true }))
        }),
```

- [ ] **Step 4: Run tests + build, verify pass**

Run: `cargo test --lib route_parses_paths && cargo build`
Expected: route test PASS; build green.

- [ ] **Step 5: Commit**

```bash
git add src/web.rs
git commit -m "$(printf 'feat(web): per-project rules read + overlay convention/review-map write routes\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 7: CLI `palugada project rules <name>`

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- New `ProjectCmd::Rules { name: String }`.

- [ ] **Step 1: Add the subcommand variant**

In `enum ProjectCmd` (`src/main.rs:247`), add:

```rust
    /// Show the effective rules (profile + per-project overlay) for a project.
    Rules { name: String },
```

- [ ] **Step 2: Implement the handler**

In `cmd_project`, add an arm:

```rust
        ProjectCmd::Rules { name } => {
            let global = GlobalConfig::load_or_default()?;
            let eff = effective::effective_rules(&global, &name)?;
            println!("Effective rules for '{}' (profile: {})\n", eff.project, eff.profile);
            println!("Conventions:");
            for c in &eff.conventions {
                let tag = match c.origin {
                    effective::Origin::Profile => "[profile]",
                    effective::Origin::Project => "[project]",
                    effective::Origin::Overridden => "[overridden]",
                };
                println!("  {:<12} {:<16} {}", tag, c.id, c.description);
            }
            println!("\nreview_map:");
            for e in &eff.review_map {
                let tag = match e.origin {
                    effective::Origin::Project => "[project]",
                    _ => "[profile]",
                };
                println!("  {:<10} {} -> {}", tag, e.family, e.conventions.join(", "));
            }
            for w in &eff.warnings {
                eprintln!("warning: {w}");
            }
            Ok(())
        }
```

- [ ] **Step 3: Build + smoke**

Run: `cargo build && cargo run -- project rules NOPE`
Expected: build green; clear error "project 'NOPE' is not registered" (exit non-zero).

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "$(printf 'feat(project): `project rules <name>` prints effective rules with origins\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 8: web console — Effective Rules card

**Files:**
- Modify: `src/web/app.js`
- Modify: `src/web/style.css`

**Interfaces:**
- Consumes `GET /api/project/<name>/rules` and posts to the three write routes from Task 6.
- Hooks into the existing `renderProjectDetail(name)` (added in cycle B).

- [ ] **Step 1: Add the Effective Rules section to project detail**

In `src/web/app.js`, inside `renderProjectDetail(name)` (after the skill-flow section), fetch and render the rules card:

```javascript
async function renderRulesCard(name) {
  const data = await api(`/api/project/${encodeURIComponent(name)}/rules`);
  const badge = (o) => `<span class="origin origin-${o}">${o}</span>`;
  const convRows = data.conventions.map((c) => {
    const editable = c.origin === 'project' || c.origin === 'overridden';
    const actions = editable
      ? `<a href="#" data-rule-edit="${c.id}" data-proj="${name}">[edit]</a>`
      : `<a href="#" data-rule-view="${c.id}" data-proj="${name}" data-profile="${data.profile}">[view]</a>`;
    return `<tr><td>${badge(c.origin)}</td><td>${c.id}</td><td>${c.title}</td><td>${actions}</td></tr>`;
  }).join('');
  const rmRows = data.review_map.map((e) =>
    `<tr><td>${badge(e.origin)}</td><td>${e.family}</td><td>${e.conventions.join(', ')}</td></tr>`
  ).join('');
  const warns = data.warnings.length
    ? `<div class="warn">${data.warnings.map((w) => `⚠ ${w}`).join('<br>')}</div>` : '';
  return `
    <section class="card">
      <h3>Effective Rules</h3>
      <p class="muted">Edits here touch THIS project's overlay in its repo (.palugada/),
        committed with the project — not the shared profile.</p>
      ${warns}
      <table class="rules"><thead><tr><th>origin</th><th>id</th><th>title</th><th></th></tr></thead>
        <tbody>${convRows}</tbody></table>
      <button data-add-rule="${name}">+ Add project rule</button>
      <h4>review_map</h4>
      <table class="rules"><thead><tr><th>origin</th><th>family</th><th>conventions</th></tr></thead>
        <tbody>${rmRows}</tbody></table>
      <button data-edit-rm="${name}" data-rm='${JSON.stringify(reviewMapObject(data.review_map))}'>Edit review_map override</button>
    </section>`;
}

function reviewMapObject(entries) {
  const o = {};
  entries.filter((e) => e.origin === 'project').forEach((e) => { o[e.family] = e.conventions; });
  return o;
}
```

Append `await renderRulesCard(name)` output into the detail page's HTML and wire the buttons in the page's delegated click handler.

- [ ] **Step 2: Wire the edit/add/view/remap actions**

Add handlers (reuse the existing `showBody`/editor modal pattern from cycle B; the profile-body GET endpoint is `/api/profile/<profile>/convention/<id>` for `[view]` on profile-only rows):

```javascript
// [view] profile-only convention (read-only, from the shared profile)
on('[data-rule-view]', 'click', async (el) => {
  const id = el.dataset.ruleView, profile = el.dataset.profile;
  const md = await api(`/api/profile/${encodeURIComponent(profile)}/convention/${encodeURIComponent(id)}`);
  showBody(`${id} (profile)`, md.markdown ?? md);
});

// [edit] overlay convention body
on('[data-rule-edit]', 'click', async (el) => {
  const id = el.dataset.ruleEdit, name = el.dataset.proj;
  const cur = await api(`/api/project/${encodeURIComponent(name)}/rules`); // or a body GET if added
  openEditor(`Edit overlay convention '${id}'`, '', async (markdown) => {
    await api(`/api/project/${encodeURIComponent(name)}/convention/${encodeURIComponent(id)}/body`,
      'POST', { markdown });
    renderProjectDetail(name);
  });
});

// + Add project rule (reuse the profile add-convention form shape)
on('[data-add-rule]', 'click', (el) => {
  const name = el.dataset.addRule;
  openConventionForm(async (spec) => {
    await api(`/api/project/${encodeURIComponent(name)}/convention`, 'POST', spec);
    renderProjectDetail(name);
  });
});

// Edit review_map override (textarea of JSON object family -> [ids])
on('[data-edit-rm]', 'click', (el) => {
  const name = el.dataset.editRm;
  openEditor('Edit review_map override (JSON: { family: [ids] })', el.dataset.rm, async (txt) => {
    const review_map = JSON.parse(txt);
    await api(`/api/project/${encodeURIComponent(name)}/review-map`, 'POST', { review_map });
    renderProjectDetail(name);
  });
});
```

> Adapt `api`/`on`/`showBody`/`openEditor`/`openConventionForm` to the actual helper names already in `app.js` (cycle A/B). If a body GET for overlay conventions is wanted for pre-filling `[edit]`, add `GET /api/project/<name>/convention/<id>` returning `{markdown}` in Task 6 — otherwise pre-fill empty and instruct the user to paste full markdown. **Decision: pre-fill empty (verbatim overwrite) to keep Task 6 to 4 routes; note this in the editor.**

- [ ] **Step 3: Style origin badges**

In `src/web/style.css`:

```css
.origin { font-size: 11px; padding: 1px 6px; border-radius: 3px; text-transform: uppercase; }
.origin-profile { background: #2b3a4a; color: #9bb; }
.origin-project { background: #2b4a32; color: #9d9; }
.origin-overridden { background: #4a3a2b; color: #db9; }
table.rules { width: 100%; border-collapse: collapse; margin: 8px 0; }
table.rules th, table.rules td { text-align: left; padding: 4px 8px; border-bottom: 1px solid #2a2a2a; }
.warn { color: #d99; margin: 6px 0; }
```

- [ ] **Step 4: Manual render check**

Run: `cargo run -- web` (open the printed loopback URL), navigate to a project's detail page.
Expected: the Effective Rules card renders conventions with origin badges, the review_map table, and the Add/Edit buttons.

- [ ] **Step 5: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "$(printf 'feat(web): Effective Rules card — origin badges, add/edit overlay, remap\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 9: End-to-end verification

**Files:** none (verification only).

- [ ] **Step 1: Full build + test + clippy**

Run: `cargo test && cargo clippy --all-targets -- -D warnings && cargo build --release`
Expected: all tests pass (≥84 + new), no clippy errors, release builds.

- [ ] **Step 2: Live e2e against a real project**

Install the local build and exercise the overlay against `status-saver` (profile android-mvvm):

```bash
cargo install --path . --force
# from a scratch copy or the real repo, with the project registered:
palugada project rules status-saver           # baseline: all [profile]
```

Then in the project's repo, add an overlay (via `palugada web` → project → Effective Rules, or by hand): add a `ours` convention, override `architecture`, remap a family to include `ours`. Re-run:

```bash
palugada project rules status-saver           # shows [project]/[overridden] rows + remapped family
palugada brief review HEAD --project status-saver   # review pulls overlay body + remapped conventions
```

Expected: `project rules` reflects the overlay with correct origin tags; `brief review` renders the overlay `architecture` body and the remapped family's conventions.

- [ ] **Step 3: Confirm old configs still parse**

Run `palugada project rules <some-other-project>` that has **no** `review_map`/overlay.
Expected: works, all rows `[profile]`, no overlay errors.

- [ ] **Step 4: Final commit (if any verification fixups)**

```bash
git add -A && git commit -m "$(printf 'test: e2e verification fixups for convention overlay\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Self-review

**Spec coverage:**
- Storage (overlay `.palugada/conventions/` + `review_map` in config.yaml) → Tasks 1, 3.
- Merge semantics (by-id origin; replace-per-family) → Task 2.
- `effective_rules` resolver + JSON shape → Task 4 + 6 (`/rules`).
- brief consumption → Task 5.
- Web routes + UI → Tasks 6, 8.
- CLI inspector → Task 7.
- Tests + e2e → each task's TDD steps + Task 9.

**Placeholder scan:** All code steps contain concrete code. The only deliberate adaptation notes are in Task 4 (confirm `GlobalConfig` knowledge-dir field name) and Task 8 (adapt to existing JS helper names) — flagged with a concrete decision, not left open.

**Type consistency:** `Origin`/`EffectiveConvention`/`EffectiveReviewEntry`/`EffectiveRules` used identically across Tasks 2/4/6/7. `conventions_in`/`add_convention_in`/`set_convention_body_in`/`convention_outline_in` defined in Task 1 and consumed in Tasks 4/5/6. `apply_review_overlay`/`overlay_dir`/`convention_outline_overlaid` defined in Tasks 2/4 and consumed in Task 5. `set_review_map` defined in Task 3, consumed in Task 6.
