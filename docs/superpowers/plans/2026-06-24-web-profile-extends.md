# Web Profile Inheritance + Editable Browse Docs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the `extends` profile-inheritance feature into the `palugada web` console — create-with-base, merged conventions/recipes with per-section provenance, and inheritance-aware editing — AND fix the standing bug that browse-list docs have no edit affordance.

**Architecture:** The CLI already resolves inheritance via `src/inherit.rs`; the web console does not. This plan routes the web reads through `inherit::*`, adds **provenance** (own / overridden / inherited-from-`<parent>`) to the merge output, threads `extends` through profile creation, and reworks the Doc Reader + browse rows so OWN docs are editable inline and INHERITED docs offer "Override in this profile" (per-section override that preserves live inheritance — the user's chosen semantics).

**Tech Stack:** Rust 2021 (`tiny_http` web backend, `serde_json`), vanilla JS frontend (`src/web/app.js`, no framework/build/test-runner), `serde`/`serde_yaml`.

**Scope note:** This is **Plan B** of the profile-inheritance work (spec `docs/superpowers/specs/2026-06-24-profile-inheritance-design.md` §5, web parts), merged with the edit-affordance bug fix. Two decisions locked with the user (2026-06-24):
- **Edit bug + Plan B = one branch** (`feat/web-profile-extends`) — same files, and surfacing merged docs activates the inherited-edit path.
- **Editing an INHERITED doc = local override that preserves live inheritance** — inherited docs are read-only-with-badge; "Override in this profile" opens the existing Add-convention/Add-recipe form (for conventions: author only the changed sections; the read-time merge keeps the rest live). NOT whole-body copy.

## Global Constraints

- Language: **Rust 2021**; every fallible fn returns **`Result<T, String>`**; no `unwrap()/expect()/panic!` outside `#[cfg(test)]`.
- **No new dependencies.**
- **DO NOT run `cargo fmt`** — this repo has no rustfmt.toml and CI does not enforce formatting; the maintainer uses a compact wide hand-style. Match the surrounding style by hand; touch only the lines a change requires.
- **BIN crate — no lib target.** Build/test with `cargo build` / `cargo test` (NOT `--lib`). Focused: `cargo test inherit::tests`, `cargo test web::tests`.
- Frontend (`app.js`) has **no JS test runner** — frontend changes are verified by a documented **manual smoke test** (launch `palugada web`, exercise the UI). Backend changes get Rust unit tests.
- Provenance vocabulary (exact strings): a doc/section `origin` is one of **`"own"`** (defined only in the active profile), **`"overridden"`** (defined in the active profile AND an ancestor), **`"inherited"`** (defined only in an ancestor). `from` = the profile id that owns the winning version (for `inherited`, the ancestor; otherwise the active id).
- The active profile is `chain[0]` from `inherit::resolve_chain(kn, id)` (chain is most-derived-first).
- Web error mapping: `read()` → 200/500, `write_op()` → 200/400 (so a returned `Err` surfaces as HTTP 400/500 with `{"error": ...}`).

---

### Task 1: Provenance-aware merge (`inherit.rs` + struct fields)

**Files:**
- Modify: `src/knowledge.rs` — add `origin`/`from` fields to `SectionMeta`, `TopicMeta`, `RecipeMeta`.
- Modify: `src/inherit.rs` — add `merged_conventions_provenance` + `merged_recipes_provenance`.
- Test: inline `#[cfg(test)] mod tests` in `src/inherit.rs`.

**Interfaces:**
- Produces: `pub fn crate::inherit::merged_conventions_provenance(kn, profile) -> Result<Vec<TopicMeta>, String>` — like `merged_conventions` but each topic + each section carries `origin`/`from`.
- Produces: `pub fn crate::inherit::merged_recipes_provenance(kn, profile) -> Result<Vec<RecipeMeta>, String>` — each recipe carries `origin`/`from` (whole-doc).
- Existing `merged_conventions`/`merged_recipes` (CLI) are unchanged; their output has empty `origin`/`from` (serde skips empty).

- [ ] **Step 1: Add `origin`/`from` to the three structs in `src/knowledge.rs`**

`SectionMeta` (currently `{id,title,tokens}`, derives Serialize+Deserialize+Default+Clone) — add two fields:

```rust
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct SectionMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tokens: usize,
    /// Provenance (filled only by inherit::*_provenance; empty for plain reads).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub origin: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
}
```

`TopicMeta` (derives Serialize only) — add the same two fields with `#[serde(skip_serializing_if = "String::is_empty")]` and `#[serde(default)]` (default needed so the existing constructors that don't set them still compile — but TopicMeta is built struct-literal in `knowledge::conventions`; see Step 2):

```rust
#[derive(serde::Serialize, Default)]
pub struct TopicMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub sections: Vec<SectionMeta>,
    pub related: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub origin: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub from: String,
}
```

`RecipeMeta` (derives Serialize only) — same:

```rust
#[derive(serde::Serialize, Default)]
pub struct RecipeMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub convention_refs: Vec<ConvRef>,
    pub related_recipes: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub origin: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub from: String,
}
```

Then update the struct-literal builders in `knowledge.rs` (`conventions_in` builds `TopicMeta { ... }`, `recipes` builds `RecipeMeta { ... }`) to set the two new fields to `String::new()` (add `origin: String::new(), from: String::new(),` to each literal). Adding `Default` derive lets you instead write `..Default::default()` at the end of each literal — use whichever keeps the existing field lines unchanged; prefer appending `origin: String::new(), from: String::new(),`.

- [ ] **Step 2: Add the provenance merges to `src/inherit.rs`** (after `merged_recipes`)

```rust
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
```

- [ ] **Step 3: Add provenance tests to `src/inherit.rs` tests**

```rust
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
```

(The `conv_indexed` helper added in Plan A's Task 4 tests already exists in `inherit.rs` tests; reuse it.)

- [ ] **Step 4: Build + test + commit**

Run: `cargo build 2>&1 | tail -3 && cargo test inherit::tests 2>&1 | tail -5`
Expected: builds clean (no fmt run); new provenance tests pass; existing tests still pass.

```bash
git add src/knowledge.rs src/inherit.rs
git commit -m "feat(inherit): provenance-aware merge (own/overridden/inherited + from)"
```

---

### Task 2: Backend web reads use merged docs + provenance + extends

**Files:**
- Modify: `src/web.rs` — `profile_json` (merged + provenance + `extends`/`chain`); the `Convention`/`Recipe` GET body handlers (chain-resolved body).
- Test: `src/web.rs` tests (focused, where the global `knowledge_dir()` can be redirected via the `PALUGADA_KNOWLEDGE` env var) + a documented manual smoke.

**Interfaces:**
- Consumes: `crate::inherit::merged_conventions_provenance`, `merged_recipes_provenance`, `resolve_chain`, `read_extends`, `resolve_convention_raw`, `resolve_recipe_raw`.

- [ ] **Step 1: Rewrite `profile_json` in `src/web.rs`** (replace the existing fn body, web.rs:544-553)

```rust
fn profile_json(id: &str) -> Result<serde_json::Value, String> {
    let kn = knowledge_dir()?;
    let chain = crate::inherit::resolve_chain(&kn, id)?;
    Ok(json!({
        "id": id,
        "extends": crate::inherit::read_extends(&kn, id),
        "chain": chain,
        "conventions": jv(&crate::inherit::merged_conventions_provenance(&kn, id)?),
        "recipes": jv(&crate::inherit::merged_recipes_provenance(&kn, id)?),
        "fact_families": crate::indexer::fact_families(&kn, id).unwrap_or_default(),
        "flows": jv(&flows(&kn, id).unwrap_or_default()),
    }))
}
```

`extends` serializes as `null` when `read_extends` returns `None` (fine for the frontend). `chain` is `[id]` for a flat profile (frontend treats length-1 as "no inheritance").

- [ ] **Step 2: Route the Doc-Reader body GETs through the chain** in `src/web.rs`'s `api()` dispatch

Find the `Route::Convention(id, cid)` and `Route::Recipe(id, rid)` arms (web.rs ~185-192) that currently return `{ "markdown": convention_md(...) }` / `recipe_md(...)`. Change them to the chain-aware resolvers so an INHERITED doc's body renders in the reader:

```rust
Route::Convention(id, cid) => read(|| {
    let kn = knowledge_dir()?;
    let md = crate::inherit::resolve_convention_raw(&kn, &id, &cid)?
        .ok_or_else(|| format!("no convention '{cid}' in profile '{id}' or its parents"))?;
    Ok(json!({ "markdown": md }))
}),
Route::Recipe(id, rid) => read(|| {
    let kn = knowledge_dir()?;
    let md = crate::inherit::resolve_recipe_raw(&kn, &id, &rid)?
        .ok_or_else(|| format!("no recipe '{rid}' in profile '{id}' or its parents"))?;
    Ok(json!({ "markdown": md }))
}),
```

(Match the exact closure/`read()` shape already used in `api()`; the key change is swapping `knowledge::convention_md`/`recipe_md` for `inherit::resolve_convention_raw`/`resolve_recipe_raw`, which return `Result<Option<String>, String>`.)

- [ ] **Step 3: Add a focused web test** (`src/web.rs` tests) using `PALUGADA_KNOWLEDGE`

`knowledge_dir()` honors the `PALUGADA_KNOWLEDGE` env var first (per `knowledge::knowledge_dir`). A test can point it at a temp chain and assert `profile_json` shape. Tests run single-threaded for env safety — gate with a mutex or set/remove in one test. Minimal:

```rust
    #[test]
    fn profile_json_exposes_extends_chain_and_provenance() {
        let kn = tempfile::tempdir().unwrap();
        // base + child(extends base), child overrides a section
        for (p, ext) in [("base", None), ("kid", Some("base"))] {
            let d = kn.path().join("profiles").join(p);
            std::fs::create_dir_all(d.join("conventions")).unwrap();
            std::fs::create_dir_all(d.join("recipes")).unwrap();
            let mut y = format!("id: {p}\nfact_families:\n  - {{ id: symbol, symbol: true }}\n");
            if let Some(e) = ext { y.push_str(&format!("extends: {e}\n")); }
            std::fs::write(d.join("profile.yaml"), y).unwrap();
            std::fs::write(d.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
            std::fs::write(d.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();
        }
        crate::knowledge::add_convention_in(&kn.path().join("profiles/base/conventions"),
            &crate::knowledge::ConventionSpec { id: "arch".into(), title: "Arch".into(), description: "d".into(), tags: vec![],
                sections: vec![crate::knowledge::SectionSpec { title: "Layers".into(), body: "L".into(), code: false }] }).unwrap();
        std::fs::write(kn.path().join("profiles/kid/conventions/_index.json"), r#"{"topics":[]}"#).unwrap();

        std::env::set_var("PALUGADA_KNOWLEDGE", kn.path());
        let v = profile_json("kid").unwrap();
        std::env::remove_var("PALUGADA_KNOWLEDGE");

        assert_eq!(v["extends"], "base");
        assert_eq!(v["chain"][0], "kid");
        let arch = v["conventions"].as_array().unwrap().iter().find(|c| c["id"] == "arch").unwrap();
        assert_eq!(arch["origin"], "inherited");
        assert_eq!(arch["from"], "base");
    }
```

(If the existing web tests don't set env vars, keep this test self-contained and serialize via `#[serial]` only if a serial-test dep exists — it does NOT here, so set/remove within the single test and accept the small global-env risk, matching how `knowledge_dir` is already env-driven. If flakiness is a concern, note it and rely on the Task 1 provenance unit tests as the real gate.)

- [ ] **Step 4: Build + test + commit**

Run: `cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -5`
Expected: clean; all tests pass.

```bash
git add src/web.rs
git commit -m "feat(web): GET /api/profile/<id> serves merged docs + provenance + extends/chain"
```

---

### Task 3: Create profile with `--extends` (backend + Extends selector)

**Files:**
- Modify: `src/web.rs` — `create_profile` `NewProfile.extends` + pass-through + robust languages handling.
- Modify: `src/web/app.js` — `renderProfiles` New-profile form gains an "Extends (base profile)" `<select>`.

**Interfaces:**
- Consumes: `crate::profile::scaffold_new(kn, id, extends)` (already 3-arg from Plan A).

- [ ] **Step 1: Add `extends` to `create_profile` in `src/web.rs`** (replace web.rs:399-427)

```rust
fn create_profile(body: &str) -> Result<serde_json::Value, String> {
    #[derive(serde::Deserialize)]
    struct NewProfile {
        id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        languages: Vec<String>,
        #[serde(default)]
        extends: Option<String>,
    }
    let np: NewProfile = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
    let kn = knowledge_dir()?;
    let extends = np.extends.as_deref().filter(|s| !s.is_empty());
    crate::profile::scaffold_new(&kn, &np.id, extends)?;
    // Apply the chosen title / languages over the generated profile.yaml.
    if !np.title.is_empty() || !np.languages.is_empty() {
        let pf = kn.join("profiles").join(&np.id).join("profile.yaml");
        let mut raw = std::fs::read_to_string(&pf).map_err(|e| e.to_string())?;
        if !np.title.is_empty() {
            raw = raw.replace(
                &format!("title: \"{} profile\"", np.id),
                &format!("title: \"{}\"", np.title.replace('"', "'")),
            );
        }
        if !np.languages.is_empty() {
            // Flat profiles scaffold `languages: []`; an extends-child copies the
            // parent's `languages: [...]`. Replace whichever line is present.
            let langs = format!("languages: [{}]", np.languages.join(", "));
            if let Some(start) = raw.find("\nlanguages:") {
                let line_start = start + 1;
                let line_end = raw[line_start..].find('\n').map(|i| line_start + i).unwrap_or(raw.len());
                raw.replace_range(line_start..line_end, &langs);
            }
        }
        std::fs::write(&pf, raw).map_err(|e| e.to_string())?;
    }
    Ok(json!({ "ok": true, "id": np.id }))
}
```

(The languages change makes the replace robust for both `languages: []` and a copied `languages: [kotlin]`; if the user leaves languages blank on an extends-child, the parent's languages are kept.)

- [ ] **Step 2: Add the Extends selector to `renderProfiles` in `src/web/app.js`**

Read the current `renderProfiles` (app.js ~638-665): it builds a form with `#np-id`, `#np-title`, `#np-langs`, a `#np-create` button, and already fetches `/api/profiles` (for the list). Make these edits:
1. In the form HTML, after the title input and before/after languages, insert:
   ```html
   <label>Extends (optional base profile)
     <select id="np-extends"><option value="">(none — standalone)</option></select>
   </label>
   ```
2. After the `/api/profiles` fetch resolves (the call that populates the list), also populate the dropdown — for each `{id,title}` in `profiles`, append `<option value="${id}">${esc(id)} — ${esc(title)}</option>` to `#np-extends`.
3. In the `#np-create` click handler, read `const extends = form.querySelector('#np-extends').value;` and add `extends` to the POST body: `api('/api/profile','POST',{ id, title, languages, extends: extends || undefined })`.

Use the existing `esc()` helper for option text. Keep the existing toast + re-render behavior.

- [ ] **Step 3: Build + manual smoke + commit**

Run: `cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -3`
Manual smoke (documented; run if a browser is available):
```bash
KP="$(pwd)/knowledge"
# start the server in the background, create a child via the UI or curl:
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada web --port 7799 &   # then:
curl -s -XPOST localhost:7799/api/profile -d '{"id":"mvi-web","title":"MVI","extends":"android-mvvm"}'
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada profile validate mvi-web   # expect OK, extends: android-mvvm
curl -s localhost:7799/api/profile/mvi-web | head -c 400                     # expect "extends":"android-mvvm", merged conventions w/ origin
rm -rf "$KP/profiles/mvi-web"; kill %1
```
Expected: the created child has `extends: android-mvvm` in its profile.yaml, validates, and `/api/profile/mvi-web` shows inherited conventions with `origin:"inherited"`. Clean up the demo profile.

```bash
git add src/web.rs src/web/app.js
git commit -m "feat(web): create profile with --extends (base selector + manifest seed)"
```

---

### Task 4: Doc-Reader provenance labels + browse-row origin badge

**Files:**
- Modify: `src/web/app.js` — `renderDoc` (sections panel + recipe header), `docRow`, `renderProfileDetail` header.
- Verification: manual smoke (no JS test runner).

- [ ] **Step 1: Show a profile-level `extends` line** in `renderProfileDetail`

Read `renderProfileDetail` (app.js ~684). After the view header, if `d.extends` is truthy, render a small muted line: `extends <chip>${esc(d.extends)}</chip>` (reuse the `id-chip` class). This tells the user the profile inherits from a base.

- [ ] **Step 2: Add an origin badge to `docRow`** (app.js ~675)

Read `docRow(meta, kind, profileId, ...)`. `meta.origin` is now `"own"`/`"overridden"`/`"inherited"`/`""`. Add a small badge after the title when `meta.origin === 'inherited'` (label `inherited · ${meta.from}`) or `meta.origin === 'overridden'` (label `overridden`). Reuse an existing badge/pill class (e.g. the `sticker`/`id-chip` styles already in `style.css`); do not introduce new colors. `own`/empty → no badge.

- [ ] **Step 3: Per-section provenance in the `renderDoc` Sections panel** (app.js ~202-216)

Read the Sections-panel loop in `renderDoc` (it renders one row per `meta.sections[]` with `#<sid>` chip + title + `~tok`). For each section `s`, append a provenance badge when `s.origin === 'inherited'` (`inherited · ${esc(s.from)}`) or `s.origin === 'overridden'` (`overridden`) — `own`/empty → none. This makes "which sections are mine vs the parent's" visible at a glance. Use `esc()` and existing classes.

- [ ] **Step 4: Build + manual smoke + commit**

Run: `cargo build 2>&1 | tail -3` (frontend is embedded via `include_str!`, so it ships in the binary — rebuild to serve the new app.js).
Manual smoke (with the `mvi-web`/`android-mvvm` chain or any extends pair): open `palugada web`, open the child profile detail — confirm the `extends` line, inherited docs show an `inherited · <parent>` badge in the list, and opening a merged convention shows per-section `inherited`/`overridden`/own labels in the Sections panel. Clean up any demo profile.

```bash
git add src/web/app.js
git commit -m "feat(web): Doc Reader + browse rows show own/overridden/inherited provenance"
```

---

### Task 5: Editable browse docs (THE BUG FIX) + inherited "Override" affordance

**Files:**
- Modify: `src/web/app.js` — `docRow` gains an edit/override action; `editDoc` confirmed for own/overridden docs; inherited → opens the Add/override form.
- Verification: manual smoke + the existing Rust `set_*_body` edit-only tests (already green).

**Context:** Today `docRow` (browse list) renders only a "View" link; `editDoc` is wired ONLY from flow-step rows (`stepRow`). So a created doc not referenced by a flow has no edit entry point — this is the user's "tidak bisa ubah convention/recipe yang udah dibikin" bug. `editDoc` itself works for docs whose `.md` exists locally (own/overridden). Inherited docs have no local `.md`, so they get an "Override in this profile" action instead (opens the existing Add form; authoring a same-id convention with the changed sections creates the child-local override, which the read-time merge layers in).

- [ ] **Step 1: Add edit/override actions to `docRow`** (app.js ~675)

Read `docRow(meta, kind, profileId, ...)`. Alongside the existing "View" link, add a second action based on `meta.origin`:
- `own` or `overridden` (or empty, i.e. a flat-profile local doc) → an **"edit"** link wired exactly like `stepRow`'s `.doc-edit` (call `editDoc(profileId, kind, meta.id)` — read `stepRow` ~585-605 for the exact `editDoc` signature/argument order and reuse it verbatim).
- `inherited` → an **"override"** link that, for `kind==='convention'`, opens the Add-convention form (`addConventionForm`, app.js ~879) pre-seeded with `meta.id` as the id (so authoring writes a child-local override of that topic); for `kind==='recipe'`, opens the Add-recipe form similarly. (If the Add forms don't accept a preset id, pass it through — read the form fn signature and add an optional `presetId` arg that pre-fills and locks the id field.)

Keep using existing classes; do not change the View behavior.

- [ ] **Step 2: Confirm `editDoc` handles the browse entry point** (app.js ~610)

Read `editDoc(profileId, kind, docId)`. It GETs `/api/profile/<id>/<kind>/<docId>` then POSTs to `.../body`. With Task 2, the GET now resolves via the chain — but for OWN/OVERRIDDEN docs the local `.md` exists and `set_*_body` (the POST target) writes it fine. Ensure `editDoc` is callable from `docRow` (same args as from `stepRow`). No backend change is needed for own/overridden edits. Add a guard: if a caller somehow invokes `editDoc` on an inherited doc, the backend `set_*_body` returns `"... does not exist in profile"` (HTTP 400) which `editDoc` already renders inline as `✗ <msg>` — acceptable fallback, but the `docRow` routing in Step 1 should prevent reaching it.

- [ ] **Step 3: Build + manual smoke (reproduce + verify the fix) + commit**

Run: `cargo build 2>&1 | tail -3`
Manual smoke — **reproduce the user's bug then verify the fix**:
```
1. palugada web → open profile `rust-cli` (flat, no extends) → Conventions list.
2. BEFORE: (verified from code) a row had only "View". AFTER: each row now has "edit".
3. Click "edit" on `architecture` → editor opens with the body → change a line → Save → ✓ saved.
4. Re-open → the edit persisted (q architecture from CLI shows it too).
5. (inherited path) On an extends-child (e.g. a temp `mvi-web`): an inherited convention row shows "override" (not "edit"); clicking opens the Add-convention form seeded with that id; authoring a section writes a child-local override; the merged reader then shows that section as `overridden`.
```
Expected: own docs are now editable directly from the browse list (bug fixed); inherited docs offer override-via-add. Clean up any demo profiles.

```bash
git add src/web/app.js
git commit -m "fix(web): browse-list docs are editable (own); inherited docs offer local override"
```

---

## Self-Review

**1. Spec coverage (spec §5 web parts + the 2 locked decisions):**

| Requirement | Task |
|---|---|
| `create_profile --extends` (web) | Task 3 |
| Extends selector in New-profile form | Task 3 |
| Merged `/api/profile` (inherited + own) | Task 2 |
| Per-section provenance (own/overridden/inherited) | Task 1 (compute) + Task 4 (render) |
| Doc-Reader inherited/overridden labels | Task 4 |
| Edit bug fix (browse docs editable) | Task 5 |
| Inherited-doc edit = local override (preserve live inheritance) | Task 5 (override-via-add) |
| Inherited doc body renders in reader | Task 2 (Step 2) |

**2. Placeholder scan:** Backend tasks (1-3) carry complete Rust. Frontend tasks (3-5) specify exact edits against named functions with the snippet to insert + "read the current function first" because `app.js` is a large interlocking file with no test runner — each frontend step names the function, the line region, the exact new markup/logic, and a manual smoke that reproduces+verifies. No "TBD"/"handle later".

**3. Type consistency:** `merged_conventions_provenance`/`merged_recipes_provenance`/`classify` are named identically across Task 1 (def) and Task 2 (use). `origin`/`from` are `String` on `SectionMeta`/`TopicMeta`/`RecipeMeta`, serialized via `skip_serializing_if = "String::is_empty"`, so CLI reads (empty) omit them and web reads include them. `editDoc`/`addConventionForm` signatures are to be read from `app.js` and reused verbatim (Task 5 flags the possible `presetId` arg addition).

**4. Risk notes:**
- The `profile_json` env-var web test (Task 2 Step 3) mutates global `PALUGADA_KNOWLEDGE`; the Task 1 provenance unit tests are the real gate if that test proves flaky.
- Frontend has no automated tests — Tasks 4-5 lean on manual smoke; the implementer MUST run the documented smoke and paste output in the report.
- `addConventionForm` may need a small `presetId` parameter (Task 5 Step 1) — if so, that is a real code change, not a placeholder; read the fn and add it.
