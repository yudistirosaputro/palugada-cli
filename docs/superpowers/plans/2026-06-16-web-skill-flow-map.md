# Per-project Skill Flow map — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a per-project detail page to `palugada web` that resolves and visualizes each generated skill and the exact steps/conventions/recipes it routes to, with in-place editing of those conventions/recipes.

**Architecture:** A new Rust resolver (`src/skillmap.rs`) parses the bound profile's `flows`/`review_map`, classifies each step, checks existence, and computes tool-skill gating — exposed via `GET /api/project/{name}/skillmap`. The vanilla-JS console renders it as a per-project page; `[edit]` writes convention/recipe `.md` files verbatim via two new body endpoints. Knowledge lives centrally in `~/.palugada`, so editing never touches a project repo.

**Tech Stack:** Rust (serde, serde_yaml, tiny_http), vanilla JS, `node:test`-free (Rust tests only).

Spec: `docs/superpowers/specs/2026-06-16-web-skill-flow-map-design.md`

---

## File structure

| File | Action | Responsibility |
|---|---|---|
| `src/knowledge.rs` | Modify | `set_convention_body` / `set_recipe_body` (verbatim, edit-only) + test |
| `src/skillmap.rs` | Create | step classification + tool gating + skillmap resolver (+ tests) |
| `src/main.rs` | Modify | `mod skillmap;` |
| `src/scaffold.rs` | Modify | make `FLOW_SKILLS` `pub` |
| `src/web.rs` | Modify | 3 new routes (skillmap GET, 2 body POSTs) + handlers + route test |
| `src/web/app.js` | Modify | per-project detail view, skill-flow render, view + edit |
| `src/web/style.css` | Modify | step/badge styling |

---

## Task 1: `knowledge::set_convention_body` / `set_recipe_body`

**Files:**
- Modify: `src/knowledge.rs` (add writers after `add_recipe`, ~line 353; add test in the `tests` module)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src/knowledge.rs`:

```rust
#[test]
fn set_body_overwrites_verbatim_and_guards() {
    let kn = tempfile::tempdir().unwrap();
    let kp = kn.path();
    add_convention(kp, "p", &ConventionSpec {
        id: "arch".into(), title: "Arch".into(), description: "d".into(),
        tags: vec!["t".into()],
        sections: vec![SectionSpec { title: "S".into(), body: "old".into(), code: false }],
    }).unwrap();
    set_convention_body(kp, "p", "arch", "# brand new body\n").unwrap();
    assert!(convention_md(kp, "p", "arch").unwrap().contains("brand new body"));
    // _index.json metadata preserved
    let cs = conventions(kp, "p").unwrap();
    let arch = cs.iter().find(|c| c.id == "arch").unwrap();
    assert_eq!(arch.title, "Arch");
    assert_eq!(arch.tags, vec!["t".to_string()]);
    // edit-only guard
    assert!(set_convention_body(kp, "p", "missing", "x").is_err());

    add_recipe(kp, "p", &RecipeSpec {
        id: "pag".into(), title: "Pag".into(), description: "".into(), tags: vec![], body: "steps".into(),
    }).unwrap();
    set_recipe_body(kp, "p", "pag", "# r\nfresh\n").unwrap();
    assert!(recipe_md(kp, "p", "pag").unwrap().contains("fresh"));
    assert!(set_recipe_body(kp, "p", "nope", "x").is_err());
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test set_body_overwrites_verbatim_and_guards 2>&1 | tail -15`
Expected: FAIL — `cannot find function `set_convention_body``.

- [ ] **Step 3: Implement the writers**

Add after `add_recipe` (after line ~353) in `src/knowledge.rs`:

```rust
/// Overwrite an existing convention's markdown verbatim (edit-only).
pub fn set_convention_body(kn: &Path, profile: &str, id: &str, markdown: &str) -> Result<(), String> {
    set_doc_body(kn, profile, "conventions", id, markdown)
}

/// Overwrite an existing recipe's markdown verbatim (edit-only).
pub fn set_recipe_body(kn: &Path, profile: &str, id: &str, markdown: &str) -> Result<(), String> {
    set_doc_body(kn, profile, "recipes", id, markdown)
}

/// Write `<dir>/<id>.md` verbatim; errors if it doesn't already exist (edit-only),
/// leaving `_index.json` metadata untouched.
fn set_doc_body(kn: &Path, profile: &str, dir: &str, id: &str, markdown: &str) -> Result<(), String> {
    validate_doc_id(id)?;
    let p = kn.join("profiles").join(profile).join(dir).join(format!("{id}.md"));
    if !p.exists() {
        let what = dir.strip_suffix('s').unwrap_or(dir);
        return Err(format!("{what} '{id}' does not exist in profile '{profile}'"));
    }
    fs::write(&p, markdown).map_err(|e| format!("write {}: {e}", p.display()))
}
```

- [ ] **Step 4: Run it to verify it passes**

Run: `cargo test set_body_overwrites_verbatim_and_guards 2>&1 | tail -8`
Expected: PASS (1 test).

- [ ] **Step 5: Commit**

```bash
git add src/knowledge.rs
git commit -m "feat(knowledge): verbatim edit-only body writers for conventions/recipes

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `src/skillmap.rs` — step classifier + tool gating (pure)

**Files:**
- Create: `src/skillmap.rs`
- Modify: `src/main.rs` (add `mod skillmap;`)

- [ ] **Step 1: Create `src/skillmap.rs` with the pure functions + tests**

```rust
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
```

- [ ] **Step 2: Register the module**

In `src/main.rs`, add `mod skillmap;` alongside the other `mod` declarations (e.g. next to `mod scaffold;`).

- [ ] **Step 3: Run the tests**

Run: `cargo test skillmap:: 2>&1 | tail -10`
Expected: PASS (3 tests). (A `dead_code` warning for the not-yet-used `skillmap()` is fine — there is none yet.)

- [ ] **Step 4: Commit**

```bash
git add src/skillmap.rs src/main.rs
git commit -m "feat(skillmap): step classifier + tool-skill gating (pure, tested)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `skillmap()` resolver

**Files:**
- Modify: `src/skillmap.rs` (add resolver + integration test)
- Modify: `src/scaffold.rs` (make `FLOW_SKILLS` public)

- [ ] **Step 1: Make `FLOW_SKILLS` public**

In `src/scaffold.rs`, change `const FLOW_SKILLS:` to `pub const FLOW_SKILLS:` (~line 191).

- [ ] **Step 2: Write the failing integration test**

Add to `mod tests` in `src/skillmap.rs`:

```rust
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
```

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test skillmap_resolves 2>&1 | tail -12`
Expected: FAIL — `cannot find function `skillmap``.

- [ ] **Step 4: Implement the resolver**

Add to `src/skillmap.rs` (above the `#[cfg(test)]` module):

```rust
#[derive(Serialize, Debug, PartialEq)]
pub struct SkillMap {
    pub project: String,
    pub profile: String,
    pub skills: Vec<serde_json::Value>,
    pub warnings: Vec<String>,
}

#[derive(Deserialize, Default)]
struct ProfileFlows {
    #[serde(default)]
    flows: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    review_map: BTreeMap<String, Vec<String>>,
}

fn load_flows(kn: &Path, profile: &str) -> Result<ProfileFlows, String> {
    let p = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))
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
```

- [ ] **Step 5: Run the test**

Run: `cargo test skillmap:: 2>&1 | tail -10`
Expected: PASS (4 tests).

- [ ] **Step 6: Commit**

```bash
git add src/skillmap.rs src/scaffold.rs
git commit -m "feat(skillmap): per-project resolver (flows + review_map + gating + warnings)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: web routes + handlers

**Files:**
- Modify: `src/web.rs` (Route enum, `route()`, `api()` dispatch, route test ~line 375)

- [ ] **Step 1: Add the failing route test**

In `src/web.rs` `mod tests`, add to `route_parses_paths` (after the existing asserts):

```rust
        assert_eq!(route("GET", "/api/project/app/skillmap"), Route::SkillMap("app".into()));
        assert_eq!(
            route("POST", "/api/profile/p/convention/c/body"),
            Route::SetConventionBody("p".into(), "c".into())
        );
        assert_eq!(
            route("POST", "/api/profile/p/recipe/r/body"),
            Route::SetRecipeBody("p".into(), "r".into())
        );
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test route_parses_paths 2>&1 | tail -10`
Expected: FAIL — `no variant ... SkillMap`.

- [ ] **Step 3: Add the Route variants**

In the `enum Route` (after `Init,`):

```rust
    SkillMap(String),
    SetConventionBody(String, String),
    SetRecipeBody(String, String),
```

- [ ] **Step 4: Add the route matches**

In `route()`, add before the final `_ => Route::NotFound,` (note: place the 6-segment body routes BEFORE the 5-segment convention/recipe GETs are unaffected — match arms are by slice length so order is safe, but keep them grouped):

```rust
        ("GET", ["api", "project", name, "skillmap"]) => Route::SkillMap((*name).to_string()),
        ("POST", ["api", "profile", id, "convention", cid, "body"]) => {
            Route::SetConventionBody((*id).to_string(), (*cid).to_string())
        }
        ("POST", ["api", "profile", id, "recipe", rid, "body"]) => {
            Route::SetRecipeBody((*id).to_string(), (*rid).to_string())
        }
```

- [ ] **Step 5: Add the dispatch handlers**

In `api()`, add before the final `_ => (501, ...)`:

```rust
        Route::SkillMap(name) => read(|| {
            let global = crate::config::GlobalConfig::load_or_default()?;
            let name = crate::http::decode_segment(&name);
            Ok(jv(&crate::skillmap::skillmap(&global, &name)?))
        }),
        Route::SetConventionBody(id, cid) => write_op(|| set_doc_body(&id, "convention", &cid, body)),
        Route::SetRecipeBody(id, rid) => write_op(|| set_doc_body(&id, "recipe", &rid, body)),
```

And add this helper near the other write handlers (e.g. after `set_project_profile`):

```rust
fn set_doc_body(profile: &str, kind: &str, id: &str, body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct Req {
        markdown: String,
    }
    let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let kn = knowledge_dir()?;
    match kind {
        "convention" => crate::knowledge::set_convention_body(&kn, profile, id, &req.markdown)?,
        "recipe" => crate::knowledge::set_recipe_body(&kn, profile, id, &req.markdown)?,
        other => return Err(format!("unknown doc kind '{other}'")),
    }
    Ok(json!({ "ok": true, "id": id }))
}
```

- [ ] **Step 6: Run the route test + full build**

Run: `cargo test route_parses_paths 2>&1 | tail -8 && cargo build 2>&1 | tail -3`
Expected: route test PASS; build OK.

- [ ] **Step 7: Commit**

```bash
git add src/web.rs
git commit -m "feat(web): skillmap GET + convention/recipe body POST routes

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: web UI — per-project detail + skill-flow render + view

**Files:**
- Modify: `src/web/app.js`

- [ ] **Step 1: Make project names link to a detail view**

In `renderProjects()`, replace the `const card = h(...)` line and add a click handler. Replace:

```js
    const card = h(`<div class="card"><strong>${esc(p.name)}</strong>${p.active ? ' <span class="pill">active</span>' : ""}
      <div class="muted">${esc(p.repo_path)}</div>
      <div class="row" style="margin-top:6px"><label style="margin:0">profile</label>
        <select class="proj-profile" style="max-width:240px">${opts}</select></div></div>`);
```

with:

```js
    const card = h(`<div class="card"><a class="link projlink"><strong>${esc(p.name)}</strong></a>${p.active ? ' <span class="pill">active</span>' : ""}
      <div class="muted">${esc(p.repo_path)}</div>
      <div class="row" style="margin-top:6px"><label style="margin:0">profile</label>
        <select class="proj-profile" style="max-width:240px">${opts}</select></div></div>`);
    card.querySelector(".projlink").onclick = () => renderProjectDetail(p.name);
```

- [ ] **Step 2: Add the detail view + render helpers**

Add these functions to `src/web/app.js` (after `renderProjects`):

```js
async function renderProjectDetail(name) {
  view.innerHTML = `<h2>${esc(name)}</h2><p><a class="link" id="back">← projects</a></p>`;
  document.getElementById("back").onclick = renderProjects;
  let m;
  try { m = await api("/api/project/" + encodeURIComponent(name) + "/skillmap"); }
  catch (e) { toast(e.message, true); return; }
  view.appendChild(h(`<div class="card"><span class="muted">profile:</span> <strong>${esc(m.profile)}</strong></div>`));
  if (m.warnings && m.warnings.length) {
    view.appendChild(h(`<div class="card warn"><strong>⚠ warnings</strong>${
      m.warnings.map(w => `<div class="muted">• ${esc(w)}</div>`).join("")
    }</div>`));
  }
  m.skills.forEach(s => view.appendChild(skillCard(m.profile, s)));
}

function skillCard(profile, s) {
  const card = h(`<div class="card"></div>`);
  const head = h(`<div class="row"><strong>${esc(s.name)}</strong> <span class="pill">${esc(s.kind)}</span><span class="spacer"></span></div>`);
  card.appendChild(head);
  if (s.command) card.appendChild(h(`<div class="muted"><code>${esc(s.command)}</code></div>`));
  if (s.kind === "flow") {
    const steps = h(`<div class="steps"></div>`);
    (s.steps || []).forEach(st => steps.appendChild(stepRow(profile, st)));
    if (!s.steps || !s.steps.length) steps.appendChild(h(`<div class="muted">no steps defined for this flow</div>`));
    card.appendChild(steps);
  } else if (s.kind === "tool") {
    head.appendChild(s.enabled
      ? h(`<span class="ok-pill">active</span>`)
      : h(`<span class="warn-pill">⚠ needs ${esc((s.needs || []).join(" or "))}</span>`));
  }
  return card;
}

function stepRow(profile, st) {
  if (st.kind === "engine")
    return h(`<div class="step"><span class="step-tag engine">engine</span> <span class="muted">${esc(st.token)} — ${esc(st.label)}</span></div>`);
  if (st.kind === "review_map") {
    const rows = (st.expand || []).map(e =>
      `<div class="muted" style="margin-left:18px">${esc(e.family)} → ${e.conventions.map(esc).join(", ")}</div>`).join("");
    return h(`<div class="step"><span class="step-tag review">review_map</span> <span class="muted">by changed file kind</span>${rows}</div>`);
  }
  const missing = st.exists === false;
  const row = h(`<div class="step"><span class="step-tag ${esc(st.kind)}">${esc(st.kind)}</span> <code>${esc(st.id)}</code>${
    missing ? ' <span class="warn-pill">⚠ missing</span>'
            : ' <a class="link doc-view">view</a> <a class="link doc-edit">edit</a>'
  }</div>`);
  if (!missing) {
    row.querySelector(".doc-view").onclick = () => viewDoc(profile, st.kind, st.id);
    row.querySelector(".doc-edit").onclick = () => editDoc(profile, st.kind, st.id);
  }
  return row;
}

async function viewDoc(profile, kind, id) {
  try {
    const b = await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}`);
    showBody(`${kind}: ${id}`, b.markdown);
  } catch (e) { toast(e.message, true); }
}
```

(`editDoc` is added in Task 6 — clicking edit before then will throw a ReferenceError; Task 6 follows immediately.)

- [ ] **Step 3: Manual smoke (read path)**

Run: `cargo run -- web --port 7799` (Ctrl-C after), open http://127.0.0.1:7799, go to Projects → click a project → confirm the SKILL FLOW renders bugfix/feature/refactor/review with steps, review_map expands, and tool skills show active / needs-…. (If no project is registered, run `palugada init <repo>` first or test after Task 7.)

- [ ] **Step 4: Commit**

```bash
git add src/web/app.js
git commit -m "feat(web): per-project skill-flow detail view + step rendering + view

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: web UI — inline edit + styling

**Files:**
- Modify: `src/web/app.js` (add `editDoc`)
- Modify: `src/web/style.css`

- [ ] **Step 1: Add `editDoc`**

Append to `src/web/app.js` (after `viewDoc`):

```js
async function editDoc(profile, kind, id) {
  let b;
  try { b = await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}`); }
  catch (e) { toast(e.message, true); return; }
  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview">
    <div class="row"><strong>edit ${esc(kind)}: ${esc(id)}</strong><span class="spacer"></span><a class="link" id="ed-close">close</a></div>
    <div class="muted">Edits the profile's knowledge in <code>~/.palugada</code> (shared by all projects on '${esc(profile)}'); your project repo is not touched.</div>
    <textarea id="ed-body" style="min-height:320px;width:100%"></textarea>
    <div class="row" style="margin-top:6px"><span class="spacer"></span><button id="ed-save">Save</button></div></div>`);
  card.querySelector("#ed-body").value = b.markdown;
  view.insertBefore(card, view.children[1] || null);
  card.querySelector("#ed-close").onclick = () => card.remove();
  card.querySelector("#ed-save").onclick = async () => {
    const markdown = card.querySelector("#ed-body").value;
    try {
      await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}/body`, "POST", { markdown });
      toast(`saved ${kind} ${id}`);
      card.remove();
    } catch (e) { toast(e.message, true); }
  };
}
```

(Note: the textarea value is set via `.value` rather than inline HTML so markdown with `</textarea>` or `&` can't break out.)

- [ ] **Step 2: Add styling**

Append to `src/web/style.css`:

```css
.steps { margin-top: 8px; }
.step { padding: 3px 0; }
.step-tag {
  display: inline-block; min-width: 78px; text-align: center;
  font-size: 11px; padding: 1px 6px; border-radius: 4px; margin-right: 6px;
  background: #2b313c; color: #9aa4b2;
}
.step-tag.convention { background: #1f3a2e; color: #7fd1a6; }
.step-tag.recipe { background: #312a1f; color: #d1b07f; }
.step-tag.review { background: #2a2f3c; color: #9ab0d1; }
.warn-pill { color: #e0a458; font-size: 12px; margin-left: 6px; }
.ok-pill { color: #7fd1a6; font-size: 12px; margin-left: 6px; }
.card.warn { border-left: 3px solid #e0a458; }
```

- [ ] **Step 3: Manual smoke (edit path)**

`cargo run -- web --port 7799`, open a project, click `edit` on a convention, change a line, Save, reopen `view` → confirm the change persisted. Verify the file changed: `git -C ~/.palugada ... ` n/a (knowledge dir is the repo's `knowledge/` in dev) — instead check the convention file under the active knowledge dir was updated. Then `git checkout` any dev-knowledge edits you made while testing.

- [ ] **Step 4: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "feat(web): inline edit of convention/recipe bodies from the flow map

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Full verification

**Files:** none.

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | tail -6`
Expected: all pass (72 prior + new skillmap/knowledge/web tests).

- [ ] **Step 2: Clippy/build clean**

Run: `cargo build 2>&1 | tail -4`
Expected: builds; no new warnings beyond the pre-existing one.

- [ ] **Step 3: End-to-end manual via the live console**

`cargo run -- web --port 7799`. With at least one registered project (else `palugada init <some repo>` first), open Projects → a project: verify (a) bugfix/feature/refactor/review show concrete conventions/recipes, (b) `review` expands via review_map, (c) a not-configured tool skill shows `⚠ needs …`, (d) editing a convention persists. Revert any test edits to the dev `knowledge/` with `git checkout knowledge/`.

- [ ] **Step 4: Confirm no stray edits to tracked knowledge**

Run: `git status --porcelain`
Expected: empty (any test edits to `knowledge/` reverted).

---

## Self-Review

**Spec coverage:**
- Backend resolver endpoint → Task 3 + 4. ✓
- `skillmap.rs` step classifier + tool gating → Task 2. ✓
- JSON shape (skills + steps + warnings) → Task 3. ✓
- Per-project detail page from Projects → Task 5. ✓
- View + edit (verbatim body, both kinds) → Tasks 5, 6 + Task 1 writers + Task 4 routes. ✓
- Missing-convention/recipe + needs-integration flags → Task 3 (warnings + tool `enabled`), rendered Task 5. ✓
- Generic flow rendering + custom skills → Task 3. ✓
- `set_convention_body`/`set_recipe_body` round-trip + guard → Task 1. ✓
- Tests (unit classify/tool, resolver integration, route, knowledge) → Tasks 1–4. ✓
- Edits never touch the project repo (central knowledge + UI note) → Task 6 note. ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code.

**Type consistency:** `Step` variants (`engine`/`convention`/`recipe`/`review_map` via snake_case tag) match the JS `stepRow` switch and the resolver. `set_convention_body`/`set_recipe_body` signatures match Task 1 ↔ Task 4 handler. `skillmap(global, name) -> SkillMap` matches the web handler. `tool_skills`/`classify_step` signatures match their callers. Route variants `SkillMap`/`SetConventionBody`/`SetRecipeBody` consistent across enum, `route()`, `api()`, and tests.
