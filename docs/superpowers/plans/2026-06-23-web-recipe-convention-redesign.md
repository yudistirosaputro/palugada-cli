# Web Console Recipe & Convention Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring tuntun-android-cli's cleaner recipe & convention *web* UX into palugada's "Pop Workbench" console — rendered/structured reading, a Sections panel, clickable cross-references, list/browse separated from authoring — as a repeatable standard enforced across every profile.

**Architecture:** palugada's web console stays vanilla JS (no framework, no build step) on the existing `tiny_http` backend (`src/web.rs`) that serves `src/web/{index.html,app.js,style.css}` via `include_str!`. The cleanliness the user admires in tuntun is *architectural, not React*: (1) rendered markdown + a derived Sections panel instead of a raw `<pre>`, (2) one concern per screen, (3) constrained, navigable cross-references. All three are plain DOM/CSS and reuse palugada's existing comic component system. The backend is already CRUD-complete and profile-scoped (`/api/profile/<id>/...`); the only backend work is to stop *dropping* index data the renderer needs, plus hard-fail `profile validate` checks so the UX is consistent for every profile by construction.

**Tech Stack:** Rust (`tiny_http`, `serde`, `serde_json`, `serde_yaml`), vanilla ES (no framework), the Pop Workbench CSS token system (`src/web/style.css:11-79`). Tests: Rust `#[cfg(test)]` for backend; `node --check` + manual browser verification for frontend (this codebase has no JS test harness, and adding one is out of scope per the zero-build principle).

## Global Constraints

- **Vanilla JS only.** No framework, no bundler, no `dist/`. The server `include_str!`s `app.js`/`style.css` directly (`src/web.rs:12-14`). Do not add Vite/React/TS.
- **Keep the Pop Workbench comic aesthetic.** Reuse the existing CSS variables (`src/web/style.css:11-79`). **Add no new colors and do not change the palette.** New CSS must be expressed with existing tokens (`--ink`, `--surface`, `--surface-2`, `--accent`, `--pow`, `--conv`, `--rec`, `--r`, `--s*`, `--bw`, `--shadow*`).
- **Escape all user/markdown data** through the existing `esc()` helper (`src/web/app.js:31`). Never inject unescaped strings into `innerHTML`. Preserve the no-XSS posture.
- **Recipe reader defaults to FULL** (entire recipe incl. code), with a brief toggle. (User decision.)
- **Per-profile consistency is enforced by HARD-FAIL** checks in `palugada profile validate <id>` — a profile that can't render cleanly must exit non-zero. (User decision.)
- **Ship as ONE milestone** (all six tasks), not phased. (User decision.)
- Server stays loopback-only; do not touch `host_ok` / binding.
- Conventional commits (`feat(web):`, `feat:`, `fix:`, `chore(knowledge):`), matching the repo's history.

---

## File Structure

| File | Responsibility | Change |
|------|----------------|--------|
| `src/knowledge.rs` | Typed knowledge accessors (`conventions`, `recipes`) consumed by the web API. | Enrich `ConvTopic`/`RecipeEntry` deserializers + `TopicMeta`/`RecipeMeta` to carry section `id`+`tokens`, topic `related`, recipe `convention_refs`+`related_recipes`. Update 3 tests. |
| `src/effective.rs` | Profile↔overlay convention merge (consumes `TopicMeta`). | One-line test-helper update (new `related` field). |
| `src/profile.rs` | `profile validate` checks. | Add `web_render_checks()` (4 hard-fail checks) + a test. |
| `knowledge/profiles/android-mvvm/conventions/_index.json` | android-mvvm convention index. | Prune dangling `related` ids. |
| `knowledge/profiles/android-mvvm/recipes/_index.json` | android-mvvm recipe index. | Prune dangling `related_recipes` ids. |
| `src/web/app.js` | The whole vanilla-JS console. | Add `mdToHtml`/`renderDoc`/`docRow`/`openConventionAt`; remove `showBody`; restructure `renderProfileDetail`; dedupe `renderKnowledge`; tokenize stray colors; inline save pill. |
| `src/web/style.css` | Pop Workbench stylesheet. | Add `.prev` (rendered-markdown) + `.doc-sections` rules using existing tokens only. |

No backend route changes: `profile_json` (`src/web.rs:544-553`) already serializes whatever `conventions()`/`recipes()` return, so the enriched `TopicMeta`/`RecipeMeta` flow to the frontend automatically.

---

## Task 1: Enrich the typed knowledge accessors

The web API drops data that already exists in every profile's `_index.json`: section `id`+`tokens` (collapsed to a title string), topic `related`, and recipe `convention_refs`/`related_recipes` (not deserialized at all). The index **writer** (`render_sections`, `src/knowledge.rs:350-364`) already emits `id`/`title`/`tokens` per section, so this is purely a *reader* fix — no writer change, and newly-authored docs already carry the right shape.

**Files:**
- Modify: `src/knowledge.rs:104-138` (deserializers), `src/knowledge.rs:165-210` (meta structs + mappers)
- Modify: `src/effective.rs:188` (test helper)
- Test: `src/knowledge.rs` (inline `#[cfg(test)]`, lines 923-942, 999-1016)

**Interfaces:**
- Produces: `pub struct SectionMeta { id: String, title: String, tokens: usize }` and `pub struct ConvRef { topic: String, section: String }` (both `Serialize + Deserialize`); `TopicMeta.sections: Vec<SectionMeta>`, `TopicMeta.related: Vec<String>`; `RecipeMeta.convention_refs: Vec<ConvRef>`, `RecipeMeta.related_recipes: Vec<String>`.
- Consumes: nothing new.

- [ ] **Step 1: Update the two tests that assert `sections` as `Vec<String>`**

In `src/knowledge.rs`, replace the assertion at line 925 (in `add_convention_writes_md_and_index`):

```rust
        let topics = conventions(kn.path(), "p").unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(
            topics[0].sections.iter().map(|s| s.title.clone()).collect::<Vec<_>>(),
            vec!["Modeling Failures".to_string()]
        );
        assert_eq!(topics[0].sections[0].id, "modeling-failures");
```

Replace the assertion at line 941 (in `conventions_accessor_reads_index`):

```rust
        let v = conventions(kn.path(), "p").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "arch");
        assert_eq!(
            v[0].sections.iter().map(|s| s.title.clone()).collect::<Vec<_>>(),
            vec!["Overview".to_string()]
        );
        assert_eq!(v[0].sections[0].id, "o");
```

Replace the assertion at line 1010 (in `import_convention_derives_sections_and_stores_body_verbatim`):

```rust
        assert_eq!(
            m.sections.iter().map(|s| s.title.clone()).collect::<Vec<_>>(),
            vec!["Result Type".to_string(), "With Code".to_string()]
        );
```

(Leave the `split_markdown_conventions` tests at lines 1025/1036/1048 unchanged — those assert `ConventionDoc.sections`, which stays `Vec<String>`.)

- [ ] **Step 2: Run the tests to verify they FAIL to compile**

Run: `cargo test -p palugada --lib knowledge:: 2>&1 | tail -20`
Expected: compile error — `no method named title found for ... String` / `sections` is `Vec<String>` (the structs aren't enriched yet).

- [ ] **Step 3: Enrich the deserializers**

In `src/knowledge.rs`, replace the `ConvTopic` + `ConvSection` block (lines 104-121) with:

```rust
#[derive(Deserialize, Default)]
struct ConvTopic {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    sections: Vec<SectionMeta>,
    #[serde(default)]
    related: Vec<String>,
}
```

(Delete the old `ConvSection` struct entirely — `SectionMeta`, defined in Step 4, replaces it and carries `id`+`tokens`.)

Replace the `RecipeEntry` block (lines 129-138) with:

```rust
#[derive(Deserialize, Default)]
struct RecipeEntry {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    convention_refs: Vec<ConvRef>,
    #[serde(default)]
    related_recipes: Vec<String>,
}
```

- [ ] **Step 4: Enrich the meta structs (Serialize + Deserialize, shared)**

In `src/knowledge.rs`, replace the `TopicMeta` + `RecipeMeta` block (lines 165-180) with:

```rust
/// One section of a convention, as stored in `_index.json` and surfaced to the
/// web console (its `id` is the `{#anchor}` scroll target; `tokens` is a cost estimate).
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct SectionMeta {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tokens: usize,
}

/// A recipe → convention cross-reference: `topic` (a convention id) and an
/// optional `section` (a section id within it).
#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct ConvRef {
    #[serde(default)]
    pub topic: String,
    #[serde(default)]
    pub section: String,
}

#[derive(serde::Serialize)]
pub struct TopicMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub sections: Vec<SectionMeta>,
    pub related: Vec<String>,
}

#[derive(serde::Serialize)]
pub struct RecipeMeta {
    pub id: String,
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub convention_refs: Vec<ConvRef>,
    pub related_recipes: Vec<String>,
}
```

- [ ] **Step 5: Update the mappers to pass the new fields through**

In `src/knowledge.rs`, replace the `.map(...)` closure inside `conventions_in` (lines 188-194) with:

```rust
        .map(|t| TopicMeta {
            id: t.id,
            title: t.title,
            description: t.description,
            tags: t.tags,
            sections: t.sections,
            related: t.related,
        })
```

Replace the `.map(...)` closure inside `recipes` (lines 208) with:

```rust
        .map(|r| RecipeMeta {
            id: r.id,
            title: r.title,
            description: r.description,
            tags: r.tags,
            convention_refs: r.convention_refs,
            related_recipes: r.related_recipes,
        })
```

(The keyword-search path at `src/knowledge.rs:631` reads `t.sections.iter().map(|s| s.title.as_str())` — `SectionMeta.title` is a `String`, so `.as_str()` still works; no change needed there.)

- [ ] **Step 6: Fix the `effective.rs` test helper for the new `related` field**

In `src/effective.rs`, replace the `meta()` helper (line 188) with:

```rust
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
```

- [ ] **Step 7: Run the full library test suite to verify everything passes**

Run: `cargo test -p palugada --lib 2>&1 | tail -25`
Expected: PASS — all `knowledge::` and `effective::` tests green, no warnings about unused fields.

- [ ] **Step 8: Commit**

```bash
git add src/knowledge.rs src/effective.rs
git commit -m "feat(web): surface section ids/tokens, related, and recipe cross-refs in the knowledge API"
```

---

## Task 2: Hard-fail web-render consistency checks in `profile validate`

The web renderer (Tasks 4-5) reads only the generic shapes returned by `/api/profile/<id>`, so it is identical across every profile **as long as each profile's `_index.json` carries the shape the renderer needs**. Make that a guarantee: add checks to `profile::validate` that fail (non-zero exit, already wired at `src/main.rs:991-992`) when a profile would render broken.

**Files:**
- Modify: `src/profile.rs:6-9` (imports), `src/profile.rs:60-118` (`validate`), add `web_render_checks` fn
- Test: `src/profile.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::knowledge::conventions`, `crate::knowledge::recipes` (Task 1's enriched `TopicMeta`/`RecipeMeta`).
- Produces: four new `Check` entries — `"conventions render-shape"`, `"recipe cross-refs resolve"`, `"related ids resolve"`, `"doc files present"`.

- [ ] **Step 1: Write the failing test**

In `src/profile.rs`, inside `mod tests`, add:

```rust
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p palugada --lib profile::tests::validate_flags_dangling_recipe_section_ref 2>&1 | tail -15`
Expected: FAIL — `no check named "recipe cross-refs resolve"` (panics on `.unwrap()` of `None`).

- [ ] **Step 3: Add `BTreeSet` to the imports**

In `src/profile.rs`, replace line 7:

```rust
use std::collections::{BTreeMap, BTreeSet};
```

- [ ] **Step 4: Add the `web_render_checks` function**

In `src/profile.rs`, add this function immediately after `validate` (after line 118):

```rust
/// Hard-fail checks that guarantee a profile renders consistently in the web
/// console: every topic/section has an id+title, recipe cross-refs and `related`
/// ids resolve, and every referenced markdown file exists on disk. Any failure
/// makes `palugada profile validate <id>` exit non-zero (main.rs:991), so a
/// profile that can't render cleanly cannot pass validation.
fn web_render_checks(kn: &Path, id: &str) -> Vec<Check> {
    let dir = kn.join("profiles").join(id);
    let topics = match crate::knowledge::conventions(kn, id) {
        Ok(t) => t,
        Err(e) => return vec![Check { name: "conventions render-shape".into(), ok: false, detail: e }],
    };
    let recipes = match crate::knowledge::recipes(kn, id) {
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

    // 3. `related` (conventions) and `related_recipes` ids resolve.
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

    // 4. every topic/recipe has its `<id>.md` on disk (the reader fetches it raw).
    let mut files: Result<String, String> = Ok("all convention/recipe files present".into());
    'files: {
        for t in &topics {
            if !dir.join("conventions").join(format!("{}.md", t.id)).exists() {
                files = Err(format!("convention '{}' has no conventions/{}.md", t.id, t.id));
                break 'files;
            }
        }
        for r in &recipes {
            if !dir.join("recipes").join(format!("{}.md", r.id)).exists() {
                files = Err(format!("recipe '{}' has no recipes/{}.md", r.id, r.id));
                break 'files;
            }
        }
    }
    out.push(check("doc files present", files));

    out
}
```

- [ ] **Step 5: Call `web_render_checks` from `validate`**

In `src/profile.rs`, in `validate`, insert this line immediately before the final `checks` return (between line 115 and line 117):

```rust
    checks.extend(web_render_checks(kn, id));
```

- [ ] **Step 6: Run the new test and the existing round-trip test to verify both pass**

Run: `cargo test -p palugada --lib profile:: 2>&1 | tail -20`
Expected: PASS — `validate_flags_dangling_recipe_section_ref` now fails the recipe-ref check as designed, and `new_then_validate_round_trips` still passes (an empty scaffolded profile satisfies all four checks vacuously).

- [ ] **Step 7: Commit**

```bash
git add src/profile.rs
git commit -m "feat: hard-fail profile validate when conventions/recipes can't render cleanly"
```

---

## Task 3: Prune android-mvvm's dangling references and validate all profiles

The new checks surface a real data problem: `android-mvvm` references conventions/recipes that were never authored. Prune them so the profile validates. (`flutter-bloc`, `rust-cli`, and the empty `kmp` are already clean — verified.)

**Files:**
- Modify: `knowledge/profiles/android-mvvm/conventions/_index.json`
- Modify: `knowledge/profiles/android-mvvm/recipes/_index.json`

**Interfaces:** none (data only).

- [ ] **Step 1: Prune dangling `related` ids in the conventions index**

In `knowledge/profiles/android-mvvm/conventions/_index.json`, the topics `architecture`, `errorhandling`, and `testing` list `related` ids that are not conventions (`di`, `networking`, `concurrency` — only `architecture`, `errorhandling`, `testing`, `style`, `r8-analyzer` exist).

Edit `architecture.related` from `["di", "networking", "concurrency", "testing"]` to:

```json
      "related": [
        "testing"
      ],
```

Edit `errorhandling.related` from `["architecture", "testing", "networking"]` to:

```json
      "related": [
        "architecture",
        "testing"
      ],
```

Edit `testing.related` from `["architecture", "errorhandling", "concurrency"]` to:

```json
      "related": [
        "architecture",
        "errorhandling"
      ],
```

(Leave `style.related` = `["architecture"]` — it resolves. `r8-analyzer` has no `related` — fine.)

- [ ] **Step 2: Prune dangling `related_recipes` in the recipes index**

In `knowledge/profiles/android-mvvm/recipes/_index.json`, the `feature` recipe lists `related_recipes: ["viewmodel", "repository"]` — neither recipe exists (only `feature`, `refactor`). Edit it to an empty list:

```json
      "related_recipes": [],
```

(Leave `refactor.related_recipes` = `["feature"]` — it resolves. Optionally, you may set `feature.related_recipes` to `["refactor"]` for symmetry, but the literal prune is `[]`.)

- [ ] **Step 3: Verify the JSON still parses**

Run: `python3 -c "import json,sys; [json.load(open(f)) for f in ['knowledge/profiles/android-mvvm/conventions/_index.json','knowledge/profiles/android-mvvm/recipes/_index.json']]; print('json ok')"`
Expected: `json ok`

- [ ] **Step 4: Build, then validate every profile (expect all green)**

Run:
```bash
cargo build -p palugada 2>&1 | tail -3
for p in android-mvvm flutter-bloc rust-cli kmp; do
  echo "=== $p ==="
  PALUGADA_KNOWLEDGE="$PWD/knowledge" ./target/debug/palugada profile validate "$p"
done
```
Expected: every profile prints all checks as `[ok]` (including the four new `render-shape` / `cross-refs resolve` / `related ids resolve` / `doc files present` checks) and the command exits 0 for each.

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/android-mvvm/conventions/_index.json knowledge/profiles/android-mvvm/recipes/_index.json
git commit -m "chore(knowledge): prune dangling related refs in android-mvvm so it passes profile validate"
```

---

## Task 4: Doc Reader — render markdown, default to FULL, brief toggle

Replace the raw-`<pre>` `showBody` with `renderDoc`: a comic-styled reader that renders markdown to HTML, opens in **FULL** by default, and offers a brief toggle. This is the single biggest readability win.

**Files:**
- Modify: `src/web/app.js` (add helpers + `renderDoc`; remove `showBody`; rewire call sites)
- Modify: `src/web/style.css` (add `.prev` rules)

**Interfaces:**
- Produces: `mdToHtml(md) -> string`, `slugify(s) -> string`, `stripFrontMatter(md) -> string`, `briefMarkdown(md) -> string`, `renderDoc(meta, kind, profileId, anchor) -> Promise<HTMLElement>`. `meta` is a convention/recipe object `{id, title, description?, sections?, convention_refs?, related_recipes?}`; `kind` is `"convention"` or `"recipe"`.
- Consumes: existing `esc`, `h`, `placePanel`, `api`, `toast`.

- [ ] **Step 1: Add the markdown renderer + helpers**

In `src/web/app.js`, immediately after the `esc` function (after line 34), add:

```js
function slugify(s) {
  return String(s == null ? "" : s).toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}
// Strip a leading `---\n...\n---` YAML front-matter block.
function stripFrontMatter(md) {
  const m = String(md || "");
  if (m.startsWith("---")) {
    const end = m.indexOf("\n---", 3);
    if (end !== -1) {
      const nl = m.indexOf("\n", end + 1);
      return (nl === -1 ? "" : m.slice(nl + 1)).replace(/^\s+/, "");
    }
  }
  return m;
}
// Brief view: drop fenced code blocks (keep a marker). FULL view keeps everything.
function briefMarkdown(md) {
  return String(md || "").replace(/```[\s\S]*?```/g, "\n_(code — switch to FULL)_\n");
}
// Inline spans on already-escaped text: `code`, **bold**.
function mdInline(s) {
  let t = esc(s);
  t = t.replace(/`([^`]+)`/g, (_, c) => "<code>" + c + "</code>");
  t = t.replace(/\*\*([^*]+)\*\*/g, (_, b) => "<strong>" + b + "</strong>");
  return t;
}
// Minimal, safe markdown → HTML for the Doc Reader. Escapes all text; supports
// #/##/### headings (slug ids for scroll targets), fenced code, inline code,
// bold, unordered/ordered lists, GFM tables, blockquotes, and paragraphs.
// Not full CommonMark — covers palugada convention/recipe bodies.
function mdToHtml(md) {
  const lines = String(md || "").replace(/\r\n/g, "\n").split("\n");
  let html = "";
  let para = [];
  let i = 0;
  const flush = () => { if (para.length) { html += "<p>" + mdInline(para.join(" ")) + "</p>"; para = []; } };
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^```/);
    if (fence) {
      flush();
      const code = [];
      i++;
      while (i < lines.length && !/^```/.test(lines[i])) { code.push(lines[i]); i++; }
      i++; // closing fence
      html += '<pre class="code"><code>' + esc(code.join("\n")) + "</code></pre>";
      continue;
    }
    const head = line.match(/^(#{1,4})\s+(.*?)(?:\s*\{#([a-z0-9-]+)\})?\s*$/);
    if (head) {
      flush();
      const level = head[1].length;
      const text = head[2].trim();
      const id = head[3] || slugify(text);
      const tag = level <= 2 ? "h2" : level === 3 ? "h3" : "h4";
      html += "<" + tag + ' id="sec-' + id + '">' + mdInline(text) + "</" + tag + ">";
      i++;
      continue;
    }
    const isTableHead = /^\s*\|.*\|\s*$/.test(line)
      && i + 1 < lines.length
      && /^\s*\|?[\s:\-|]+\|?\s*$/.test(lines[i + 1])
      && lines[i + 1].includes("-");
    if (isTableHead) {
      flush();
      const cells = (r) => r.trim().replace(/^\||\|$/g, "").split("|").map(c => c.trim());
      const ths = cells(line).map(c => "<th>" + mdInline(c) + "</th>").join("");
      i += 2;
      let rows = "";
      while (i < lines.length && /^\s*\|.*\|\s*$/.test(lines[i])) {
        rows += "<tr>" + cells(lines[i]).map(c => "<td>" + mdInline(c) + "</td>").join("") + "</tr>";
        i++;
      }
      html += '<div class="table-scroll"><table class="rules"><thead><tr>' + ths + "</tr></thead><tbody>" + rows + "</tbody></table></div>";
      continue;
    }
    if (/^\s*[-*]\s+/.test(line)) {
      flush();
      let items = "";
      while (i < lines.length && /^\s*[-*]\s+/.test(lines[i])) {
        items += "<li>" + mdInline(lines[i].replace(/^\s*[-*]\s+/, "")) + "</li>";
        i++;
      }
      html += "<ul>" + items + "</ul>";
      continue;
    }
    if (/^\s*\d+\.\s+/.test(line)) {
      flush();
      let items = "";
      while (i < lines.length && /^\s*\d+\.\s+/.test(lines[i])) {
        items += "<li>" + mdInline(lines[i].replace(/^\s*\d+\.\s+/, "")) + "</li>";
        i++;
      }
      html += "<ol>" + items + "</ol>";
      continue;
    }
    if (/^\s*>\s?/.test(line)) {
      flush();
      const quote = [];
      while (i < lines.length && /^\s*>\s?/.test(lines[i])) { quote.push(lines[i].replace(/^\s*>\s?/, "")); i++; }
      html += "<blockquote>" + mdInline(quote.join(" ")) + "</blockquote>";
      continue;
    }
    if (/^\s*$/.test(line)) { flush(); i++; continue; }
    para.push(line.trim());
    i++;
  }
  flush();
  return html;
}
```

- [ ] **Step 2: Replace `showBody` with `renderDoc`**

In `src/web/app.js`, replace the entire `showBody` function (lines 60-69) with:

```js
// The comic Doc Reader: rendered markdown for one convention/recipe. Opens FULL
// (entire doc incl. code) by default; a pill toggles a brief (code-stripped) view.
// `meta` carries id/title/description and (for the structured panels in Task 5)
// sections + cross-refs; the markdown body is fetched here.
async function renderDoc(meta, kind, profileId, anchor) {
  let body;
  try {
    const b = await api(`/api/profile/${encodeURIComponent(profileId)}/${kind}/${encodeURIComponent(meta.id)}`);
    body = b.markdown;
  } catch (e) { toast(e.message, true); return null; }

  let card = document.getElementById("bodyview");
  if (card) card.remove();
  card = h(`<div class="card" id="bodyview"></div>`);
  const head = h(`<div class="card-head">
    <span class="id-chip">${esc(meta.id)}</span>
    <h3 style="margin:0">${esc(meta.title || meta.id)}</h3>
    <span class="spacer"></span>
    <span class="pill doc-mode" style="cursor:pointer" title="toggle full / brief">FULL</span>
    <a class="link" id="bodyclose">close</a>
  </div>`);
  card.appendChild(head);
  if (meta.description) card.appendChild(h(`<div class="card-note">${esc(meta.description)}</div>`));

  const prose = h(`<div class="prev"></div>`);
  const stripped = stripFrontMatter(body);
  let full = true; // FULL by default (user decision)
  const renderBody = () => { prose.innerHTML = mdToHtml(full ? stripped : briefMarkdown(stripped)); };
  card.appendChild(prose);
  renderBody();

  head.querySelector(".doc-mode").onclick = (e) => {
    full = !full;
    e.target.textContent = full ? "FULL" : "BRIEF";
    renderBody();
  };
  head.querySelector("#bodyclose").onclick = () => card.remove();
  placePanel(card, anchor);
  return card;
}
```

- [ ] **Step 3: Rewire `viewDoc` to use `renderDoc`**

In `src/web/app.js`, replace the `viewDoc` function (lines 411-416) with:

```js
async function viewDoc(profile, kind, id, anchor) {
  await renderDoc({ id, title: id }, kind, profile, anchor);
}
```

- [ ] **Step 4: Rewire the four list-row `showBody` call sites to `renderDoc`**

In `src/web/app.js`, in `renderProfileDetail`, replace the convention row click handler (lines 489-492):

```js
    row.querySelector("a").onclick = () => renderDoc(c, "convention", id, row);
```

Replace the recipe row click handler (lines 504-507):

```js
    row.querySelector("a").onclick = () => renderDoc(r, "recipe", id, row);
```

In `renderKnowledge`, replace the convention row handler (line 791):

```js
      r.querySelector("a").onclick = () => renderDoc(c, "convention", id, r);
```

Replace the recipe row handler (line 799):

```js
      r.querySelector("a").onclick = () => renderDoc(c, "recipe", id, r);
```

- [ ] **Step 5: Add the `.prev` reader styles (existing tokens only)**

In `src/web/style.css`, add at the end of the file (after line 425):

```css
/* ── rendered markdown reader (Doc Reader) ── */
.prev { font-family: var(--font-ui); line-height: 1.6; color: var(--ink); margin-top: var(--s3); }
.prev h2, .prev h3, .prev h4 {
  font-family: var(--font-display); font-weight: 400; color: var(--ink);
  margin: var(--s5) 0 var(--s2); line-height: 1.1; scroll-margin-top: 76px;
}
.prev h2 { font-size: 26px; border-bottom: var(--bw) solid var(--ink); padding-bottom: 4px; }
.prev h3 { font-size: 20px; }
.prev h4 { font-size: 17px; }
.prev p { margin: var(--s3) 0; }
.prev ul, .prev ol { margin: var(--s3) 0; padding-left: var(--s6); }
.prev li { margin: 4px 0; }
.prev code { font-family: var(--font-mono); font-size: .86em; background: var(--surface-2); border: 1.5px solid var(--ink); border-radius: 5px; padding: 0 5px; color: var(--rec); }
.prev pre.code { background: var(--surface-2); border: var(--bw) solid var(--ink); border-radius: var(--r); padding: var(--s3); overflow-x: auto; box-shadow: var(--shadow-sm); }
.prev pre.code code { background: none; border: 0; padding: 0; color: var(--ink); font-size: 13px; line-height: 1.55; }
.prev blockquote { margin: var(--s3) 0; padding: 6px var(--s4); border-left: 4px solid var(--pow); background: var(--surface-2); border-radius: var(--r-sm); color: var(--ink-soft); }
.prev table.rules { margin-top: 0; }
```

- [ ] **Step 6: Syntax-check the JS and build**

Run: `node --check src/web/app.js && cargo build -p palugada 2>&1 | tail -3`
Expected: no output from `node --check` (syntax OK); build succeeds. (If `node` is unavailable, skip the check and rely on Step 7.)

- [ ] **Step 7: Manually verify the rendered reader**

Run: `PALUGADA_KNOWLEDGE="$PWD/knowledge" ./target/debug/palugada web` (it prints a `http://127.0.0.1:<port>` URL and opens a browser).
Verify, in the browser:
1. Go to **Knowledge** → pick `android-mvvm` → click **View** on the `architecture` convention.
2. The body renders as styled HTML (headings, bullet lists, fenced code blocks) — **not** a raw `<pre>` of markdown.
3. The reader header shows the id chip + title + a **FULL** pill.
4. Click the **FULL** pill → it becomes **BRIEF** and code blocks collapse to `(code — switch to FULL)`; click again → back to FULL.
5. Toggle the top-bar **Dark/Light** theme → the reader stays legible in both.
Stop the server with Ctrl-C.

- [ ] **Step 8: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "feat(web): render conventions/recipes as a comic Doc Reader (FULL default + brief toggle)"
```

---

## Task 5: Sections panel + clickable cross-references

Extend `renderDoc`: conventions get a numbered Sections panel (in the existing flow visual language) that scrolls to a section; recipes get clickable `convention_refs` chips that open the referenced convention at the named section, plus `related_recipes` chips.

**Files:**
- Modify: `src/web/app.js` (extend `renderDoc`; add `openConventionAt`)
- Modify: `src/web/style.css` (one small `.doc-sections` rule)

**Interfaces:**
- Produces: `openConventionAt(profileId, topic, section, anchor) -> Promise<void>`.
- Consumes: Task 1's `meta.sections` (`[{id,title,tokens}]`), `meta.convention_refs` (`[{topic,section}]`), `meta.related_recipes` (`[id]`); Task 4's `renderDoc`, `slugify`.

- [ ] **Step 1: Add structured panels to `renderDoc`**

In `src/web/app.js`, in `renderDoc`, insert the following **between** `renderBody();` and the `head.querySelector(".doc-mode").onclick` line (i.e. after the body first renders, before the toggle handler):

```js
  // Conventions: a numbered Sections panel (reuses the flow .step language).
  if (kind === "convention" && Array.isArray(meta.sections) && meta.sections.length) {
    const panel = h(`<div class="doc-sections"><h4 style="margin:0 0 6px">Sections</h4><div class="steps"></div></div>`);
    const steps = panel.querySelector(".steps");
    meta.sections.forEach((s, i) => {
      const sid = s.id || slugify(s.title);
      const tok = s.tokens ? ` <span class="hint">~${s.tokens} tok</span>` : "";
      const stepRowEl = h(`<div class="step" style="cursor:pointer"><span class="num">${i + 1}</span><span class="id-chip">#${esc(sid)}</span> <span class="arg">${esc(s.title)}</span>${tok}</div>`);
      stepRowEl.onclick = () => {
        const el = prose.querySelector("#sec-" + (window.CSS && CSS.escape ? CSS.escape(sid) : sid));
        if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
      };
      steps.appendChild(stepRowEl);
    });
    card.insertBefore(panel, prose);
  }

  // Recipes: clickable convention cross-refs + related-recipe chips.
  if (kind === "recipe") {
    const chips = [];
    (meta.convention_refs || []).forEach(cr => {
      const label = cr.section ? `${cr.topic}#${cr.section}` : cr.topic;
      const chip = h(`<span class="step-tag convention" style="cursor:pointer;min-width:0">${esc(label)}</span>`);
      chip.onclick = () => openConventionAt(profileId, cr.topic, cr.section, card);
      chips.push(chip);
    });
    (meta.related_recipes || []).forEach(rid => {
      const chip = h(`<span class="step-tag recipe" style="cursor:pointer;min-width:0">${esc(rid)}</span>`);
      chip.onclick = () => renderDoc({ id: rid, title: rid }, "recipe", profileId, card);
      chips.push(chip);
    });
    if (chips.length) {
      const refRow = h(`<div class="row" style="margin-top:6px"><span class="muted" style="font-weight:700;margin-right:4px">refs:</span></div>`);
      chips.forEach(c => refRow.appendChild(c));
      card.insertBefore(refRow, prose);
    }
  }
```

- [ ] **Step 2: Add the `openConventionAt` helper**

In `src/web/app.js`, immediately after `renderDoc` (after its closing brace), add:

```js
// Open the convention reader (with its Sections panel) and scroll to a section.
async function openConventionAt(profileId, topic, section, anchor) {
  let meta = { id: topic, title: topic };
  try {
    const pd = await api("/api/profile/" + encodeURIComponent(profileId));
    const found = (pd.conventions || []).find(c => c.id === topic);
    if (found) meta = found;
  } catch (e) { /* fall back to the minimal meta */ }
  const card = await renderDoc(meta, "convention", profileId, anchor);
  if (card && section) {
    setTimeout(() => {
      const el = card.querySelector("#sec-" + (window.CSS && CSS.escape ? CSS.escape(section) : section));
      if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
    }, 60);
  }
}
```

- [ ] **Step 3: Add the `.doc-sections` spacing rule**

In `src/web/style.css`, add after the `.prev table.rules` rule from Task 4:

```css
.doc-sections { margin: var(--s2) 0 var(--s4); padding: var(--s3); border: var(--bw) dashed var(--ink); border-radius: var(--r); background: var(--surface-2); }
.doc-sections h4 { font-family: var(--font-display); font-weight: 400; font-size: 18px; color: var(--ink); }
.doc-sections .step { padding: 5px 0; }
```

- [ ] **Step 4: Syntax-check and build**

Run: `node --check src/web/app.js && cargo build -p palugada 2>&1 | tail -3`
Expected: clean syntax; build succeeds.

- [ ] **Step 5: Manually verify the panels and navigation**

Run: `PALUGADA_KNOWLEDGE="$PWD/knowledge" ./target/debug/palugada web`
Verify in the browser (Knowledge → `android-mvvm`):
1. Open the `architecture` convention → a **Sections** panel lists 4 numbered rows (`#overview`, `#layers`, `#uistate`, `#data-flow`) with `~N tok`.
2. Click the `#uistate` row → the rendered body scrolls to the "Sealed UiState" heading.
3. Open the `feature` recipe → a **refs:** row shows green chips `architecture#layers`, `architecture#uistate`, `architecture#data-flow`.
4. Click `architecture#uistate` → the `architecture` convention reader opens, scrolled to the UiState section.
Stop the server with Ctrl-C.

- [ ] **Step 6: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "feat(web): numbered Sections panel + clickable recipe→convention cross-refs"
```

---

## Task 6: Separate browse from authoring; dedupe rows; tokenize colors; inline save pill

Make the profile page read like tuntun's one-concern screens: a clean convention/recipe **browse** surface up top (add-forms hidden behind buttons), with Import / Fact families / Flows / Generate moved into a clearly separated **Author & configure** region. Factor the duplicated list-row builder into one helper shared with the Knowledge view. Fix the hardcoded slate borders so forms survive light theme, and surface an inline validity pill on the editor save.

**Files:**
- Modify: `src/web/app.js` (`renderProfileDetail`, `renderKnowledge`, `editDoc`, `addConventionForm`/`addRecipeForm`/overlay/credentials border colors; add `docRow`)

**Interfaces:**
- Produces: `docRow(meta, kind, profileId) -> HTMLElement` (one `.lrow` with id-chip, title, at-a-glance meta, and a View action wired to `renderDoc`).
- Consumes: Task 4's `renderDoc`; existing `addConventionForm`, `addRecipeForm`, `importCard`, `flowsCard`, `generateForm`.

- [ ] **Step 1: Add the shared `docRow` helper**

In `src/web/app.js`, immediately before `renderProfileDetail` (before line 477), add:

```js
// One convention/recipe list row, shared by Profile detail and Knowledge so both
// read identically. Convention rows show section count; recipe rows show ref count.
function docRow(meta, kind, profileId) {
  const metaBits = kind === "convention"
    ? `${(meta.sections || []).length} sections`
    : `${(meta.convention_refs || []).length} refs`;
  const row = h(`<div class="lrow"><span class="id-chip">${esc(meta.id)}</span> <span class="ttl">${esc(meta.title || meta.id)}</span> <span class="meta">· ${metaBits}</span><span class="actions"><a class="link">View</a></span></div>`);
  row.querySelector("a").onclick = () => renderDoc(meta, kind, profileId, row);
  return row;
}
```

- [ ] **Step 2: Rewrite `renderProfileDetail` (browse vs author separation)**

In `src/web/app.js`, replace the entire `renderProfileDetail` function (lines 477-522) with:

```js
async function renderProfileDetail(id) {
  view.innerHTML = backLink("Profiles") + viewHead("Profile", id);
  document.getElementById("back").onclick = renderProfiles;
  let d;
  try { d = await api("/api/profile/" + encodeURIComponent(id)); }
  catch (e) { toast(e.message, true); return; }

  // ── Browse: conventions ──
  const cv = h(`<div class="card"><div class="card-head"><h3>Conventions</h3><span class="count">${d.conventions.length}</span></div>
    <div class="card-note">Standing standards for this stack — the "right way" to write code here. Agents pull these automatically (CLI: <code>q</code>, <code>brief</code>).</div>
    <div class="list" id="cv-list"></div>
    <div class="row" id="cv-addrow" style="margin-top:6px"><button class="ghost cv-add">+ Add convention</button></div></div>`);
  const cvList = cv.querySelector("#cv-list");
  if (!d.conventions.length) cvList.appendChild(h(`<div class="muted">No conventions yet — add one to start this profile's playbook.</div>`));
  d.conventions.forEach(c => cvList.appendChild(docRow(c, "convention", id)));
  cv.querySelector(".cv-add").onclick = () => {
    if (cv.querySelector(".ac-host")) return;
    const host = h(`<div class="ac-host"></div>`);
    host.appendChild(addConventionForm(id));
    cv.querySelector("#cv-addrow").insertAdjacentElement("beforebegin", host);
  };
  view.appendChild(cv);

  // ── Browse: recipes ──
  const rc = h(`<div class="card"><div class="card-head"><h3>Recipes</h3><span class="count">${d.recipes.length}</span></div>
    <div class="card-note">Step-by-step guides for one task. Agents pull these by name (CLI: <code>for &lt;task&gt;</code>, <code>brief feature/refactor</code>).</div>
    <div class="list" id="rc-list"></div>
    <div class="row" id="rc-addrow" style="margin-top:6px"><button class="ghost rc-add">+ Add recipe</button></div></div>`);
  const rcList = rc.querySelector("#rc-list");
  if (!d.recipes.length) rcList.appendChild(h(`<div class="muted">No recipes yet.</div>`));
  d.recipes.forEach(r => rcList.appendChild(docRow(r, "recipe", id)));
  rc.querySelector(".rc-add").onclick = () => {
    if (rc.querySelector(".ar-host")) return;
    const host = h(`<div class="ar-host"></div>`);
    host.appendChild(addRecipeForm(id));
    rc.querySelector("#rc-addrow").insertAdjacentElement("beforebegin", host);
  };
  view.appendChild(rc);

  // ── Author & configure (separated lower region) ──
  view.appendChild(h(`<div class="view-head" style="margin-top:var(--s7)"><div class="eyebrow">Author &amp; configure</div><h2 class="head" style="font-size:30px">Build this profile</h2></div>`));
  view.appendChild(importCard(id));
  view.appendChild(h(`<div class="card"><h3>Fact families</h3>
    <div class="muted">Categories of symbols palugada extracts from YOUR code (e.g. viewmodel, repository, command).
    <code>palugada fact &lt;family&gt;</code> lists them with file:line. Defined in the profile's
    <code>extractors.yaml</code> (regex / tree-sitter) — not edited here.</div><div>${
    d.fact_families.map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div></div>`));
  view.appendChild(flowsCard(id, d));
  view.appendChild(generateForm(id));
}
```

- [ ] **Step 3: Dedupe `renderKnowledge` to use `docRow`**

In `src/web/app.js`, in `renderKnowledge`'s `load` function, replace the convention `forEach` (lines 789-793) with:

```js
    pd.conventions.forEach(c => clList.appendChild(docRow(c, "convention", id)));
```

Replace the recipe `forEach` (lines 797-801) with:

```js
    pd.recipes.forEach(c => rlList.appendChild(docRow(c, "recipe", id)));
```

- [ ] **Step 4: Tokenize the four hardcoded `#2b313c` borders**

In `src/web/app.js`, replace every occurrence of `#2b313c` with `var(--ink)` (4 sites: the overlay-add box at line 238, the credentials tokens divider at line 340, the addConventionForm box at line 668, the addRecipeForm box at line 713). They are all `border...:1px solid #2b313c` → `border...:1px solid var(--ink)`.

Run this to confirm none remain:
Run: `grep -n "#2b313c" src/web/app.js || echo "none left"`
Expected: `none left`

- [ ] **Step 5: Add an inline validity pill to the profile editor save**

In `src/web/app.js`, in `editDoc`, replace the `#ed-save` click handler (lines 432-439) with:

```js
  card.querySelector("#ed-save").onclick = async () => {
    const markdown = card.querySelector("#ed-body").value;
    card.querySelectorAll(".ed-result").forEach(el => el.remove());
    try {
      await api(`/api/profile/${encodeURIComponent(profile)}/${kind}/${encodeURIComponent(id)}/body`, "POST", { markdown });
      toast(`saved ${kind} ${id}`);
      card.querySelector(".row .spacer").insertAdjacentElement("afterend", h(`<span class="ok-pill ed-result">saved ✓</span>`));
    } catch (e) {
      toast(e.message, true);
      card.querySelector(".row .spacer").insertAdjacentElement("afterend", h(`<span class="warn-pill ed-result">✗ ${esc(e.message)}</span>`));
    }
  };
```

(The editor stays open after save so the inline result is visible; the existing **close** link dismisses it.)

- [ ] **Step 6: Syntax-check and build**

Run: `node --check src/web/app.js && cargo build -p palugada 2>&1 | tail -3`
Expected: clean syntax; build succeeds.

- [ ] **Step 7: Manually verify the restructured profile page**

Run: `PALUGADA_KNOWLEDGE="$PWD/knowledge" ./target/debug/palugada web`
Verify in the browser:
1. **Profiles** → open `android-mvvm`. The page opens to clean Conventions and Recipes lists with **no** add-forms expanded inline.
2. Each convention row shows `· N sections`; each recipe row shows `· N refs`.
3. Click **+ Add convention** → the add form appears just above the button; click View on a row → the Doc Reader opens (same as Knowledge).
4. Below the lists, an **Author & configure** heading separates Import / Fact families / Flows / Generate.
5. Switch to **Light** theme and open **+ Add convention** → the form's top divider and inputs render correctly (no invisible/dark-on-light slate border).
6. Knowledge view rows look identical to Profile-detail rows (shared `docRow`).
7. From a Project (Projects → open one → Effective Rules), click **edit** on a project rule, Save → an inline `saved ✓` pill appears next to the Save button (editor stays open). *(This exercises the profile editor path via `editDoc`/overlay; the inline pill change applies to `editDoc`.)*
Stop the server with Ctrl-C.

- [ ] **Step 8: Commit**

```bash
git add src/web/app.js
git commit -m "feat(web): separate browse from authoring, dedupe doc rows, fix light-theme borders, inline save pill"
```

---

## Milestone Verification

After all six tasks:

- [ ] **Full test suite green:** `cargo test -p palugada 2>&1 | tail -15` → all pass.
- [ ] **Clippy clean:** `cargo clippy -p palugada --all-targets 2>&1 | tail -15` → no new warnings.
- [ ] **JS syntax:** `node --check src/web/app.js` → no output.
- [ ] **Every profile validates:** for `android-mvvm flutter-bloc rust-cli kmp`, `palugada profile validate <id>` exits 0 with the four new checks `[ok]`.
- [ ] **Consistency proof (the core ask):** create a throwaway profile, hand-add a convention via the web **+ Add convention** form, then run `palugada profile validate <id>` — it passes (the writer already emits section ids/tokens), and the new convention renders in the Doc Reader with a Sections panel. The web UX is therefore identical for every profile by construction, and a profile that *can't* render cleanly fails validation.

---

## Self-Review (coverage against the synthesis)

- **Un-drop rich index data** → Task 1. ✅
- **Doc Reader (rendered markdown, FULL default + brief toggle)** → Task 4. ✅ (FULL default per user decision.)
- **Sections panel in the flow visual language** → Task 5. ✅
- **Clickable recipe→convention cross-refs + related** → Task 5. ✅
- **View separation (browse vs author), dedupe list rows** → Task 6. ✅
- **Tokenize hardcoded colors, inline save validity** → Task 6. ✅
- **Hard-fail per-profile validate (the consistency mechanism)** → Task 2 + Task 3 (data cleanup so existing profiles pass). ✅
- **Keep vanilla JS + comic aesthetic, no framework, no palette change** → Global Constraints; all CSS uses existing tokens. ✅

**Known risks / watch-items:**
- The hand-rolled `mdToHtml` is intentionally minimal. Verify it against the real `android-mvvm` convention bodies (Task 4 Step 7); if a body uses markdown it doesn't cover, the FULL view degrades to plain paragraphs rather than breaking. Always escape via `esc()`/`mdInline` (no raw HTML injection).
- `set_doc_body` remains last-write-wins (no ETag); out of scope for this milestone.
- Brief view keys off fenced code blocks; conventions with no code fences look similar in brief and FULL — acceptable since FULL is the default.
- If `node` is unavailable in the executor's environment, the JS syntax gate falls back to manual browser verification.
