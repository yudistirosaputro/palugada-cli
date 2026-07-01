# "Puzzle/Pickup" Create Composer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a single "Create new" button on the Profile-detail view that opens a two-pane composer where the user picks reusable pieces (a palette of sections/conventions/recipes across the profile, its `extends` chain, and other profiles) and assembles a new convention or recipe on a canvas.

**Architecture:** A new backend `palette` resolver flattens pickable convention sections (with bodies + origin provenance) across the active profile + chain, plus lists other profiles for lazy expansion. Convention save **reuses the existing `AddConvention` writer** (the canvas is exactly a `ConventionSpec`). Recipe save **extends** `RecipeSpec`/`write_recipe_files`/`add_recipe` to carry `convention_refs` + `related_recipes` (this also fixes a latent data-loss bug where re-saving a recipe wiped its refs). Clone reuses the existing raw-body routes. Frontend is a new vanilla-JS `renderCreate` view.

**Tech Stack:** Rust (`serde`, `tiny_http`), vanilla JS/HTML/CSS. Interaction reference: the approved composer mockup (`https://claude.ai/code/artifact/691d4a53-b23a-4468-845c-0ee62f1c8d1c`).

## Global Constraints

- **Do NOT run `cargo fmt`** (repo hand-formats compact wide-style). `cargo build` → 0 warnings; `cargo test` → pass.
- Frontend verified via `node --check src/web/app.js` + curl against an isolated `~/.palugada` + visual smoke. No JS test runner.
- Commit per task with trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- Escape every interpolated string in JS via `esc()`. All HTTP goes through the `api()` wrapper (app.js:8). Save handlers full-re-render via `renderProfileDetail(id)`.
- Picked sections are **copied, not linked** — the composed doc owns its section bodies.
- Refs are stored in `recipes/_index.json` (canonical). The nested `references:` `.md` front-matter stays decorative (not parsed) — out of scope.
- Loopback-only trust model (no auth); the palette intentionally exposes all local profiles.

---

### Task 1: `palette` resolver (backend, new module)

**Files:**
- Create: `src/palette.rs`
- Modify: `src/main.rs` (add `mod palette;` beside the other module declarations)
- Test: unit tests in `src/palette.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `inherit::resolve_chain`, `inherit::merged_conventions_provenance` (returns `Vec<knowledge::TopicMeta>` with per-section `SectionMeta{id,title,tokens,origin,from}`), `inherit::resolve_convention_raw(kn,profile,topic) -> Result<Option<String>,String>`, `inherit::parse_sections(body) -> Vec<MergedSection{anchor,title,body}>`, `profile::list(kn) -> Vec<(String,String)>`, `knowledge::conventions(kn,profile)` + `knowledge::convention_md(kn,profile,id)`.
- Produces:
  - `pub struct PaletteSection { source_profile, topic_id, section_id, title, tokens, origin, from, body }` (all `String` except `tokens: usize`; derives `Serialize, Clone, Default`).
  - `pub fn palette(kn: &Path, active: &str) -> Result<Palette, String>` where `pub struct Palette { active_profile: String, chain: Vec<String>, sections: Vec<PaletteSection>, other_profiles: Vec<String> }` (derives `Serialize, Default`).
  - `pub fn profile_sections(kn: &Path, profile: &str) -> Result<Vec<PaletteSection>, String>` (one profile's OWN sections, origin `"other-profile"`, for lazy expansion).

- [ ] **Step 1: Confirm the exact upstream signatures**

Read `src/inherit.rs` around `parse_sections`/`MergedSection` (~:74-93) and `resolve_convention_raw` (~:175) and confirm: `MergedSection` field for the id is `anchor`; whether `resolve_convention_raw`'s returned markdown still contains front-matter (if so, strip it before `parse_sections`). Read `src/knowledge.rs:~818` for `strip_frontmatter`; if it is private, make it `pub(crate)` (add `pub(crate)` to `fn strip_frontmatter`). Note the confirmed facts as comments in `palette.rs`.

- [ ] **Step 2: Write the failing test**

Create `src/palette.rs` with the struct + a stub `palette()`/`profile_sections()` returning `Ok(Default::default())`/`Ok(vec![])`, then:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // Build a throwaway knowledge dir with one profile + one convention.
    fn seed(kn: &std::path::Path, profile: &str, conv_id: &str) {
        let cdir = kn.join("profiles").join(profile).join("conventions");
        fs::create_dir_all(&cdir).unwrap();
        fs::write(kn.join("profiles").join(profile).join("profile.yaml"),
            format!("id: {profile}\ntitle: {profile}\nlanguages: [rust]\n")).unwrap();
        fs::write(cdir.join(format!("{conv_id}.md")),
            "---\nid: errh\ntitle: Error handling\ndescription: d\ntags: [e]\n---\n\n# Error handling\n\n## Modeling failures {#modeling-failures}\nModel errors explicitly.\n").unwrap();
        fs::write(cdir.join("_index.json"),
            "{\"schema_version\":\"1.0\",\"topics\":[{\"id\":\"errh\",\"title\":\"Error handling\",\"description\":\"d\",\"file\":\"errh.md\",\"tags\":[\"e\"],\"sections\":[{\"id\":\"modeling-failures\",\"title\":\"Modeling failures\",\"tokens\":40}]}]}").unwrap();
    }

    #[test]
    fn palette_returns_own_section_with_body_and_origin() {
        let tmp = std::env::temp_dir().join(format!("pal-{}", std::process::id()));
        let kn = tmp.join("kn");
        seed(&kn, "rust-cli", "errh");
        let p = palette(&kn, "rust-cli").unwrap();
        let s = p.sections.iter().find(|s| s.section_id == "modeling-failures").expect("section present");
        assert_eq!(s.topic_id, "errh");
        assert_eq!(s.origin, "own");
        assert_eq!(s.from, "rust-cli");
        assert!(s.body.contains("Model errors explicitly"), "body copied from .md");
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn palette_lists_other_profiles_not_in_chain() {
        let tmp = std::env::temp_dir().join(format!("pal2-{}", std::process::id()));
        let kn = tmp.join("kn");
        seed(&kn, "rust-cli", "errh");
        seed(&kn, "flutter-bloc", "errh");
        let p = palette(&kn, "rust-cli").unwrap();
        assert!(p.other_profiles.contains(&"flutter-bloc".to_string()));
        assert!(!p.other_profiles.contains(&"rust-cli".to_string()));
        let _ = fs::remove_dir_all(&tmp);
    }
}
```

- [ ] **Step 3: Run the tests to verify they FAIL**

Run: `cargo test palette::tests`
Expected: FAIL (stub returns empty — `expect("section present")` panics).

- [ ] **Step 4: Implement `palette()` + `profile_sections()`**

```rust
//! Flattens pickable convention sections (with bodies + provenance) across the
//! active profile + its `extends` chain, and lists other profiles for lazy expand.
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Default)]
pub struct PaletteSection {
    pub source_profile: String,
    pub topic_id: String,
    pub section_id: String,
    pub title: String,
    pub tokens: usize,
    pub origin: String, // own | overridden | inherited | other-profile
    pub from: String,
    pub body: String,
}

#[derive(Serialize, Default)]
pub struct Palette {
    pub active_profile: String,
    pub chain: Vec<String>,
    pub sections: Vec<PaletteSection>,
    pub other_profiles: Vec<String>,
}

pub fn palette(kn: &Path, active: &str) -> Result<Palette, String> {
    let chain = crate::inherit::resolve_chain(kn, active)?;
    let topics = crate::inherit::merged_conventions_provenance(kn, active)?;
    let mut sections = Vec::new();
    for t in &topics {
        // Merged body across the chain (front-matter + preamble + ## sections).
        let raw = crate::inherit::resolve_convention_raw(kn, active, &t.id)?;
        let parsed = match raw {
            Some(md) => crate::inherit::parse_sections(crate::knowledge::strip_frontmatter(&md)),
            None => Vec::new(),
        };
        for sm in &t.sections {
            match parsed.iter().find(|ps| ps.anchor == sm.id) {
                Some(ps) => sections.push(PaletteSection {
                    source_profile: active.to_string(),
                    topic_id: t.id.clone(),
                    section_id: sm.id.clone(),
                    title: sm.title.clone(),
                    tokens: sm.tokens,
                    origin: if sm.origin.is_empty() { "own".into() } else { sm.origin.clone() },
                    from: if sm.from.is_empty() { active.to_string() } else { sm.from.clone() },
                    body: ps.body.clone(),
                }),
                // md<->index id drift: skip-with-warning, never crash.
                None => eprintln!("palette: no body for {}#{} (md/index drift)", t.id, sm.id),
            }
        }
    }
    let all = crate::profile::list(kn)?;
    let other_profiles = all.into_iter().map(|(id, _)| id).filter(|id| !chain.contains(id)).collect();
    Ok(Palette { active_profile: active.to_string(), chain, sections, other_profiles })
}

/// One profile's OWN convention sections (with bodies), for lazy "other profile" expansion.
pub fn profile_sections(kn: &Path, profile: &str) -> Result<Vec<PaletteSection>, String> {
    let mut out = Vec::new();
    for t in crate::knowledge::conventions(kn, profile)? {
        let md = crate::knowledge::convention_md(kn, profile, &t.id)?;
        for ps in crate::inherit::parse_sections(crate::knowledge::strip_frontmatter(&md)) {
            out.push(PaletteSection {
                source_profile: profile.to_string(),
                topic_id: t.id.clone(),
                section_id: ps.anchor.clone(),
                title: ps.title.clone(),
                tokens: ps.body.len() / 4 + 8,
                origin: "other-profile".into(),
                from: profile.to_string(),
                body: ps.body.clone(),
            });
        }
    }
    Ok(out)
}
```
(Adjust `parse_sections` arg if it takes `&str` vs owned, and `MergedSection` field names, per Step 1. `conventions()` returns `Vec<TopicMeta>` — confirm the id field is `.id`.)

- [ ] **Step 5: Register the module**

Add `mod palette;` in `src/main.rs` alongside the other `mod ...;` lines.

- [ ] **Step 6: Run tests to verify they PASS**

Run: `cargo test palette::tests`
Expected: PASS (both tests).

- [ ] **Step 7: Commit**

```bash
git add src/palette.rs src/main.rs src/knowledge.rs
git commit -m "feat(palette): resolver for pickable sections across profile+chain+others

palette() joins _index metadata + .md bodies + provenance for the active
profile and its extends chain; profile_sections() serves other profiles lazily.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Palette HTTP routes

**Files:**
- Modify: `src/web.rs` — `enum Route` (~:22-70), `route()` (~:73-162), `api()` (~:233-493), tests (~:777)

**Interfaces:**
- Consumes: `palette::palette`, `palette::profile_sections`, `web::knowledge_dir()`.
- Produces: `GET /api/profile/{id}/palette` → `Palette` JSON; `GET /api/profile/{id}/palette/{other}` → `{ sections: Vec<PaletteSection> }` for one other profile (lazy).

- [ ] **Step 1: Add the failing route test**

In `route_parses_paths()` (~:777) add:
```rust
        assert_eq!(route("GET", "/api/profile/p/palette"), Route::Palette("p".into()));
        assert_eq!(route("GET", "/api/profile/p/palette/other"),
            Route::PaletteProfile("p".into(), "other".into()));
```

- [ ] **Step 2: Run to verify FAIL**

Run: `cargo test web::tests::route_parses_paths` (bin crate — no `--lib`)
Expected: FAIL — `Route::Palette` variant does not exist (compile error).

- [ ] **Step 3: Add the variants + route arms + api handlers**

- `enum Route` (~:70, before `NotFound`): add
```rust
    Palette(String),
    PaletteProfile(String, String),
```
- `route()` — add near the profile GET arms (after the `.../convention/{cid}/raw` style arms):
```rust
        ("GET", ["api", "profile", id, "palette"]) => Route::Palette((*id).to_string()),
        ("GET", ["api", "profile", id, "palette", other]) => {
            Route::PaletteProfile((*id).to_string(), (*other).to_string())
        }
```
- `api()` — add read arms:
```rust
        Route::Palette(id) => read(|| {
            let kn = knowledge_dir()?;
            Ok(serde_json::to_value(crate::palette::palette(&kn, &id)?).map_err(|e| e.to_string())?)
        }),
        Route::PaletteProfile(_id, other) => read(|| {
            let kn = knowledge_dir()?;
            Ok(json!({ "sections": crate::palette::profile_sections(&kn, &other)? }))
        }),
```

- [ ] **Step 4: Run tests to verify PASS**

Run: `cargo test web:: && cargo build 2>&1 | tail -3` (bin crate — no `--lib`)
Expected: route test passes; build 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/web.rs
git commit -m "feat(web): GET /api/profile/{id}/palette (+ /{other} lazy)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Recipe convention_refs + related_recipes (extend writer, fix data-loss bug)

**Files:**
- Modify: `src/knowledge.rs` — `RecipeSpec` (:313-323), `add_recipe` (:521-532), `write_recipe_files` (:536-560), `add_recipe_from_markdown` (:565-578)
- Test: `src/knowledge.rs` `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: existing `ConvRef { topic: String, section: String }` (:187-193, already `Serialize+Deserialize+Clone+Default`), `recipes(kn,profile) -> Vec<RecipeMeta>` (RecipeMeta already has `convention_refs`/`related_recipes`).
- Produces: `RecipeSpec` gains `convention_refs: Vec<ConvRef>` + `related_recipes: Vec<String>`; `write_recipe_files` gains those two params and writes them into `_index.json`.

- [ ] **Step 1: Write the failing test**

In `knowledge.rs` tests:
```rust
    #[test]
    fn recipe_refs_round_trip_and_survive_resave() {
        let tmp = std::env::temp_dir().join(format!("rr-{}", std::process::id()));
        let kn = tmp.join("kn");
        std::fs::create_dir_all(kn.join("profiles").join("p").join("recipes")).unwrap();
        let spec = RecipeSpec {
            id: "feature".into(), title: "Add a feature".into(),
            description: "d".into(), tags: vec!["r".into()], body: "## Steps\n1.".into(),
            convention_refs: vec![ConvRef { topic: "architecture".into(), section: String::new() },
                                  ConvRef { topic: "testing".into(), section: "fixtures".into() }],
            related_recipes: vec!["refactor".into()],
        };
        add_recipe(&kn, "p", &spec).unwrap();
        let r = recipes(&kn, "p").unwrap().into_iter().find(|r| r.id == "feature").unwrap();
        assert_eq!(r.convention_refs.len(), 2);
        assert_eq!(r.convention_refs[0].topic, "architecture");
        assert_eq!(r.related_recipes, vec!["refactor".to_string()]);
        // Re-save (the old bug wiped refs here) — refs must persist.
        add_recipe(&kn, "p", &spec).unwrap();
        let r2 = recipes(&kn, "p").unwrap().into_iter().find(|r| r.id == "feature").unwrap();
        assert_eq!(r2.convention_refs.len(), 2, "re-save must not drop refs (regression)");
        let _ = std::fs::remove_dir_all(&tmp);
    }
```

- [ ] **Step 2: Run to verify FAIL**

Run: `cargo test recipe_refs_round_trip_and_survive_resave`
Expected: FAIL — `RecipeSpec` has no `convention_refs` field (compile error).

- [ ] **Step 3: Extend `RecipeSpec`**

In `RecipeSpec` (:313-323) add after `body`:
```rust
    #[serde(default)]
    pub convention_refs: Vec<ConvRef>,
    #[serde(default)]
    pub related_recipes: Vec<String>,
```

- [ ] **Step 4: Extend `write_recipe_files` + `add_recipe`**

`write_recipe_files` (:536): add two params and include them in the entry:
```rust
fn write_recipe_files(
    dir: &Path, id: &str, title: &str, description: &str, tags: &[String], body: &str,
    convention_refs: &[ConvRef], related_recipes: &[String],
) -> Result<(), String> {
    validate_doc_id(id)?;
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let md = format!(
        "---\nid: {}\ntitle: {}\ndescription: {}\ntags: [{}]\n---\n\n{}",
        id, yaml_scalar(title), yaml_scalar(description), tags.join(", "), body
    );
    fs::write(dir.join(format!("{id}.md")), md).map_err(|e| format!("write recipe: {e}"))?;
    let entry = serde_json::json!({
        "id": id, "title": title, "description": description,
        "file": format!("{id}.md"), "tags": tags,
        "convention_refs": convention_refs, "related_recipes": related_recipes,
    });
    upsert_index(&dir.join("_index.json"), "recipes", id, entry)
}
```
`add_recipe` (:521): pass the spec's new fields:
```rust
pub fn add_recipe(kn: &Path, profile: &str, spec: &RecipeSpec) -> Result<(), String> {
    let dir = kn.join("profiles").join(profile).join("recipes");
    let body = format!("# {}\n\n{}\n", spec.title, spec.body.trim());
    write_recipe_files(&dir, &spec.id, &spec.title, &spec.description, &spec.tags, &body,
        &spec.convention_refs, &spec.related_recipes)
}
```
`add_recipe_from_markdown` (:576): the import path has no refs — pass empty slices:
```rust
    write_recipe_files(dir, &id, &title, &meta.description, &meta.tags, body, &[], &[])?;
```

- [ ] **Step 5: Run tests to verify PASS**

Run: `cargo test recipe_refs_round_trip_and_survive_resave && cargo test && cargo build 2>&1 | tail -3` (bin crate — no `--lib`)
Expected: new test passes, no other test regresses, build 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add src/knowledge.rs
git commit -m "fix(knowledge): persist recipe convention_refs/related_recipes (+ stop wiping on re-save)

RecipeSpec + write_recipe_files now carry refs into recipes/_index.json, so the
web composer can set them AND re-saving a recipe no longer silently drops refs.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Composer view — scaffold, entry point, palette pane

**Files:**
- Modify: `src/web/app.js` — `VIEWS` map (~:272), `renderProfileDetail` (~:1008); add `renderCreate` (new)
- Modify: `src/web/style.css` — add composer layout classes (`.composer`, `.pane`, `.pitem`, `.seccard`, `.origin`, etc.), reusing Direction A tokens

**Interfaces:**
- Consumes: `api()` (app.js:8), `h()`/`esc()`/`toast()`/`splitCsv()` (app.js:21-31,145), `GET /api/profile/{id}/palette`, `renderProfileDetail(id)`.
- Produces: `renderCreate(profileId, opts)` that renders into the main view; a `.pc-create` button in the Profile-detail header.

- [ ] **Step 1: Add a "Create new" button to Profile detail**

In `renderProfileDetail(id)` (~:1008), add a primary button in the view header that opens the composer:
```js
head.appendChild(h(`<button class="btn primary" id="pc-create">+ Create new</button>`));
head.querySelector("#pc-create").onclick = () => renderCreate(id, {});
```
(Match how the existing header buttons are appended — reuse the same `h()`/onclick idiom used by `cv-add`/`rc-add`.)

- [ ] **Step 2: Implement `renderCreate` scaffold + palette fetch/render**

Add (patterned after `renderProfiles`/`renderKnowledge`), using the approved mockup's palette structure. Key logic:
```js
async function renderCreate(profileId, opts) {
  const view = document.querySelector(".view"); // or the app's main-view ref used elsewhere
  view.innerHTML = "";
  view.appendChild(h(`<div class="crumb">Profiles / <b>${esc(profileId)}</b> / Create new</div>`));
  const kind = opts.kind || "convention";
  // ... header: kind segmented toggle (convention/recipe) + "Start blank"
  let pal;
  try { pal = await api(`/api/profile/${encodeURIComponent(profileId)}/palette`); }
  catch (e) { toast(e.message, true); return; }
  // group pal.sections by (origin,from): "own"/"overridden"/"inherited" under this profile+chain
  // render collapsible groups with checkboxes; list pal.other_profiles collapsed (lazy).
  // canvas state:
  const canvas = []; // {section_id,title,body,origin,from}
  // checkbox toggles push/remove into canvas -> renderCanvas()
  // ... (see mockup for exact grouping + search)
}
```
Add composer CSS to `style.css` (two-pane grid, panes, palette rows, section cards, origin badges) using the Direction A tokens — port the mockup's `.composer/.pane/.panehead/.pitem/.seccard/.origin` rules (they already use the same token names).

- [ ] **Step 3: Register the view (optional nav)**

Add `create: (arg) => renderCreate(arg, {})` to the `VIEWS` map if a top-level entry is wanted; otherwise the profile-detail button is the sole entry (preferred). Do NOT add a sidebar nav-item unless a top-level Create is desired.

- [ ] **Step 4: Verify JS + palette render**

Run: `node --check src/web/app.js`
Then `cargo run -- web`, open the URL, go to a Profile detail (e.g. rust-cli), click "Create new": the palette lists this profile's sections grouped by origin, plus other profiles collapsed. Search filters. Stop the server.

- [ ] **Step 5: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "feat(web): Create composer scaffold + palette pane

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Composer canvas — assemble + save convention + clone

**Files:**
- Modify: `src/web/app.js` — `renderCreate` (canvas half), reuse `POST /api/profile/{id}/convention`

**Interfaces:**
- Consumes: `POST /api/profile/{id}/convention` with `ConventionSpec { id, title, description, tags, sections:[{title,body,code}] }`; `GET /api/profile/{id}/convention/{cid}/raw` (clone).
- Produces: a working convention save from picked+blank sections.

- [ ] **Step 1: Render the canvas + fields + assemble behavior**

- Canvas cards from `canvas[]`: each shows title (editable), origin badge (`own`/`base·<from>`/`<from>`), a body preview, remove (splice + uncheck palette), ↑/↓ reorder (swap array indices), and "edit" (inline textarea to edit the copied body). "+ add blank section" pushes `{title:"New section", body:"", origin:"own"}`.
- Fields: id (`valid` check `[a-z0-9_-]`), title, tags (`splitCsv`).
- "Clone whole convention" select: on change, fetch `/api/profile/{id}/convention/{cid}/raw`, strip front-matter, split into `## ` sections, push each into canvas.

- [ ] **Step 2: Wire Save (convention)**

```js
saveBtn.onclick = async () => {
  const id = idInput.value.trim();
  if (!/^[a-z0-9_-]+$/.test(id)) return toast("id: use [a-z0-9_-] only", true);
  if (!canvas.length) return toast("add at least one section", true);
  const titles = canvas.map(c => slugify(c.title));
  if (new Set(titles).size !== titles.length) return toast("two sections share a title — rename one", true);
  const spec = {
    id, title: titleInput.value.trim(), description: descInput.value.trim(),
    tags: splitCsv(tagsInput.value),
    sections: canvas.map(c => ({ title: c.title, body: c.body, code: /```/.test(c.body) })),
  };
  try {
    await api(`/api/profile/${encodeURIComponent(profileId)}/convention`, "POST", spec);
    toast(`Saved convention · ${id}`);
    renderProfileDetail(profileId);
  } catch (e) { toast(e.message, true); }
};
```
Add a local `slugify` mirroring the Rust `slug()` (lowercase, non-alnum → `-`, trim) for the dup-title guard.

- [ ] **Step 3: Verify (curl e2e, isolated HOME)**

```bash
HOME=$(mktemp -d) sh -c '
  set -e
  cargo run -q -- profile new demo --languages rust >/dev/null 2>&1 || true
  # start web in background on a fixed port if supported, else drive the writer via the CLI-equivalent:
  echo "manual: click Create new, pick 2 sections + 1 blank, Save; then:"
'
# After a manual save in the browser against a scratch profile, confirm the file exists:
ls ~/.palugada/profiles/*/conventions/ 2>/dev/null
```
(Simplest reliable check: in the running `cargo run -- web`, create a convention from picked sections into a scratch profile, then `cat` the produced `<id>.md` + `_index.json` and confirm the picked section bodies were copied. Clean up the scratch profile after.)

- [ ] **Step 4: Commit**

```bash
git add src/web/app.js
git commit -m "feat(web): composer canvas — assemble + clone + save convention

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Composer recipe mode (body + refs pickers) + absorb scattered add buttons

**Files:**
- Modify: `src/web/app.js` — `renderCreate` (recipe mode), `renderProfileDetail` (retire/relabel old add buttons)

**Interfaces:**
- Consumes: `POST /api/profile/{id}/recipe` with extended `RecipeSpec { id, title, description, tags, body, convention_refs:[{topic,section}], related_recipes:[] }`; profile-detail data (existing conventions/recipes ids for the pickers); `GET /api/profile/{id}/recipe/{rid}/raw` (clone).

- [ ] **Step 1: Recipe canvas — body + chip pickers**

When kind === "recipe": show a body textarea + two chip rows:
- "Convention refs": chips = existing convention ids (from the profile-detail data already fetched, or `pal` topic ids). Toggling adds `{topic:id, section:""}` to `convention_refs`.
- "Related recipes": chips = existing recipe ids; toggling adds to `related_recipes`.
Clone: `GET .../recipe/{rid}/raw` → strip front-matter → prefill body; prefill picker selections from that recipe's `convention_refs`/`related_recipes` (available in the profile-detail payload).

- [ ] **Step 2: Wire Save (recipe)**

```js
const spec = {
  id, title: titleInput.value.trim(), description: descInput.value.trim(),
  tags: splitCsv(tagsInput.value), body: bodyArea.value,
  convention_refs: pickedRefs.map(t => ({ topic: t, section: "" })),
  related_recipes: pickedRelated,
};
await api(`/api/profile/${encodeURIComponent(profileId)}/recipe`, "POST", spec);
```

- [ ] **Step 3: Absorb the scattered add buttons**

In `renderProfileDetail`, remove the standalone "Add convention"/"Add recipe" inline-form triggers (the `cv-add`/`rc-add` `.ac-host`/`.ar-host` handlers, app.js ~:1027-1048) — the composer supersedes them. Keep "Import markdown" as-is (out of scope). Inherited-doc "Override" links now route to `renderCreate(id, {kind, presetId})` with the id locked (overwrite that id).

- [ ] **Step 4: Verify (recipe e2e)**

`cargo run -- web` → Create new → Recipe → type body, pick 2 convention refs + 1 related recipe → Save. Then:
```bash
cat ~/.palugada/profiles/<scratch>/recipes/<id>.md
python3 -c "import json;d=json.load(open('$HOME/.palugada/profiles/<scratch>/recipes/_index.json'));print([r for r in d['recipes'] if r['id']=='<id>'])"
```
Expected: `_index.json` entry contains `convention_refs` (2) + `related_recipes` (1). Clean up scratch profile.

- [ ] **Step 5: Commit**

```bash
git add src/web/app.js
git commit -m "feat(web): composer recipe mode (refs pickers) + retire scattered add buttons

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Whole-feature verification

**Files:** none (verification only)

- [ ] **Step 1: Build + full test + JS check**

Run: `cargo build 2>&1 | tail -3 && cargo test 2>&1 | tail -8 && node --check src/web/app.js`
Expected: 0 warnings, all tests pass (incl. `palette::tests` + `recipe_refs_round_trip_and_survive_resave` + route tests), JS valid.

- [ ] **Step 2: Full manual e2e in an isolated HOME**

`HOME=$(mktemp -d)`, `cargo run -- web`; create a scratch profile (or use a copied one), then exercise: assemble a convention from own + inherited + other-profile sections (confirm bodies copied), clone a convention, override an inherited convention (locked id), create a recipe with refs (confirm `_index.json`), start-blank. Clean up.

- [ ] **Step 3: Confirm no regression to existing authoring**

Confirm Profile detail still renders, Import markdown still works, existing conventions/recipes still view/edit. Confirm the reskin (if Plan A merged first) still looks right with the new composer.

---

## Self-Review notes

- **Spec coverage:** B1 palette resolver → Task 1; palette route → Task 2; B2 convention compose (reuse AddConvention) → Task 5; B3 recipe refs + bug fix → Task 3; B4 clone → Tasks 5/6; B5 frontend (renderCreate, entry, absorb buttons) → Tasks 4-6; primitives 1-4 all covered (sections=Task1/4/5, clone=Task5/6, other-profiles=Task1/2/4, recipe-refs=Task3/6).
- **Type consistency:** `PaletteSection` fields identical across Tasks 1-2-4; `ConventionSpec`/`RecipeSpec` field names match `knowledge.rs`; `ConvRef{topic,section}` consistent Task 3 ↔ Task 6.
- **Flagged confirmations (Task 1 Step 1):** `MergedSection.anchor` field name, whether `resolve_convention_raw` output includes front-matter, `strip_frontmatter` visibility — verify against `src/inherit.rs`/`src/knowledge.rs` before implementing; tests pin the behavior regardless.
- **Frontend has no unit tests** — deliverables verified by `node --check` + curl/browser e2e against an isolated HOME, per repo norms.
- **Ordering:** Tasks 1→2 (backend palette) and 3 (recipe writer) are independent and can be done in any order; 4→5→6 (frontend) depend on 2 (palette route) and 3 (recipe route). Run Plan A (reskin) first so the composer inherits the professional theme.
```
