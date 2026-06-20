# CLI Markdown Import (`convention add` / `recipe add`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users author conventions/recipes by importing a plain SKILL.md-style markdown file (`palugada convention add <file.md>` / `palugada recipe add <file.md>`), with palugada auto-deriving sections, token estimates, slug ids, and the `_index.json` entry.

**Architecture:** One front-matter parser + verbatim-body importers in `knowledge.rs`, sharing a small file/index writer with the existing `add_convention_in`/`add_recipe` (refactored to delegate, output unchanged). Two thin CLI subcommands in `main.rs` resolve the target dir (profile conventions/recipes, or the per-project convention overlay) and call the importer.

**Tech Stack:** Rust, `serde`/`serde_yaml`/`serde_json`, `tempfile` for tests, clap-derive subcommands.

## Global Constraints

- Input is SKILL.md-style: front-matter `title` (string), `description` (string), `tags` (list), `id` (string, optional) + body (`# Title`, `## Section`s).
- Auto-mapping: `id` = front-matter `id` else `slug(title)`; sections derived from `##` headings via the existing `sections()` (skips code fences); per-section `tokens = body.len()/4 + 8`, `code = body contains a ``` fence`; `_index.json` upserted via existing `upsert_index`.
- **Body stored verbatim** (the markdown after front-matter) — no `{#slug}` anchor injection.
- Title fallback: front-matter `title` → else first `# H1` → else error.
- `id` validated with the existing `validate_doc_id` (`[a-z0-9_-]`).
- Upsert by id: replace if `<id>.md` exists (report `updated`), else `created`.
- Target: profile dir by default; `--project <name>` (convention only) → `<repo>/.palugada/conventions/` overlay via `effective::overlay_dir`. `recipe add` rejects `--project` (recipes are profile-scoped).
- Existing `add_convention`/`add_convention_in`/`add_recipe` output must stay byte-identical (their tests must stay green).
- CI parity: `cargo build --release` + `cargo test --release` green, no new warnings. No version bump / release.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

### Task 1: `knowledge.rs` — front-matter parser, shared writers, importers

**Files:**
- Modify: `src/knowledge.rs`
- Test: `src/knowledge.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub fn add_convention_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String>` — returns `(id, replaced)`.
  - `pub fn add_recipe_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String>`.
- Internal helpers: `front_matter_region`, `DocMeta` + `parse_doc_front_matter`, `first_h1`, `render_sections`, `write_convention_files`, `write_recipe_files`.
- Consumes existing: `sections()`, `slug()`, `strip_frontmatter()`, `validate_doc_id()`, `yaml_scalar()`, `upsert_index()`, `Section`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/knowledge.rs`:

```rust
#[test]
fn parse_front_matter_reads_fields_and_defaults() {
    let raw = "---\ntitle: Error Handling\ndescription: do X\ntags: [rs, error]\n---\n\n# Error Handling\nbody\n";
    let m = parse_doc_front_matter(raw).unwrap();
    assert_eq!(m.title.as_deref(), Some("Error Handling"));
    assert_eq!(m.description, "do X");
    assert_eq!(m.tags, vec!["rs".to_string(), "error".to_string()]);
    assert_eq!(m.id, None);
    // no front-matter → all defaults
    let m2 = parse_doc_front_matter("# Title\nbody").unwrap();
    assert_eq!(m2.title, None);
    assert!(m2.tags.is_empty());
}

#[test]
fn import_convention_derives_sections_and_stores_body_verbatim() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("conventions");
    let raw = "---\ntitle: Error Handling\ndescription: short\ntags: [rs, error]\n---\n\n# Error Handling\n> summary\n\n## Result Type\nuse Result\n\n## With Code\n```rust\nfn f() {}\n```\n";
    let (id, replaced) = add_convention_from_markdown(&dir, raw).unwrap();
    assert_eq!(id, "error-handling");
    assert!(!replaced);
    // metadata derived
    let metas = conventions_in(&dir).unwrap();
    let m = metas.iter().find(|c| c.id == "error-handling").unwrap();
    assert_eq!(m.title, "Error Handling");
    assert_eq!(m.sections, vec!["Result Type".to_string(), "With Code".to_string()]);
    // body stored verbatim (blockquote preserved)
    let md = convention_md_in(&dir, "error-handling").unwrap();
    assert!(md.contains("> summary"), "verbatim body must keep the blockquote: {md}");
    assert!(md.contains("## Result Type"));
    // re-import same id → replaced
    let (_id2, replaced2) = add_convention_from_markdown(&dir, raw).unwrap();
    assert!(replaced2);
}

#[test]
fn import_convention_title_falls_back_to_h1_and_errors_when_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("conventions");
    // no title field, but a # H1 exists
    let (id, _) = add_convention_from_markdown(&dir, "# My Rule\n## A\nx\n").unwrap();
    assert_eq!(id, "my-rule");
    // neither title field nor H1 → error
    assert!(add_convention_from_markdown(&dir, "no heading here\n").is_err());
}

#[test]
fn import_recipe_writes_and_upserts() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("recipes");
    let raw = "---\ntitle: Scaffold X\ndescription: how to\ntags: [scaffold]\n---\n\n# Scaffold X\nstep 1\nstep 2\n";
    let (id, replaced) = add_recipe_from_markdown(&dir, raw).unwrap();
    assert_eq!(id, "scaffold-x");
    assert!(!replaced);
    let r = recipes_in(&dir).unwrap();
    assert!(r.iter().any(|x| x.id == "scaffold-x"));
    assert!(recipe_md_in(&dir, "scaffold-x").unwrap().contains("step 1"));
    assert!(add_recipe_from_markdown(&dir, raw).unwrap().1, "re-import → replaced");
}
```

Note: this task also needs dir-taking recipe readers `recipes_in`/`recipe_md_in` for the test (mirror the existing `conventions_in`/`convention_md_in`). Add them in Step 3.

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --bin palugada import_ parse_front_matter`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement parser + readers + writers + importers**

In `src/knowledge.rs`, add the front-matter region extractor near `strip_frontmatter`:

```rust
/// Return the raw YAML front-matter block (between the leading `---` fences), if any.
fn front_matter_region(raw: &str) -> Option<&str> {
    let t = raw.trim_start();
    let rest = t.strip_prefix("---")?;
    let idx = rest.find("\n---")?;
    Some(&rest[..idx])
}

#[derive(serde::Deserialize, Default)]
pub struct DocMeta {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Parse the leading YAML front-matter into DocMeta; no front-matter → defaults.
pub fn parse_doc_front_matter(raw: &str) -> Result<DocMeta, String> {
    match front_matter_region(raw) {
        Some(fm) => serde_yaml::from_str(fm).map_err(|e| format!("parse front-matter: {e}")),
        None => Ok(DocMeta::default()),
    }
}

/// First `# H1` heading text in a markdown body (ignores `##`+).
fn first_h1(body: &str) -> Option<String> {
    for line in body.lines() {
        if let Some(h) = line.trim_start().strip_prefix("# ") {
            return Some(h.trim().to_string());
        }
    }
    None
}
```

Add dir-taking recipe readers (mirror `conventions_in`/`convention_md_in`):

```rust
/// Recipes in an arbitrary recipes dir. A missing dir/index yields an empty list.
pub fn recipes_in(dir: &Path) -> Result<Vec<RecipeMeta>, String> {
    let p = dir.join("_index.json");
    if !p.exists() {
        return Ok(Vec::new());
    }
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let idx: RecipeIndex = serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;
    Ok(idx.recipes.into_iter()
        .map(|r| RecipeMeta { id: r.id, title: r.title, description: r.description, tags: r.tags })
        .collect())
}

/// Raw markdown of one recipe file in an arbitrary recipes dir.
pub fn recipe_md_in(dir: &Path, id: &str) -> Result<String, String> {
    let p = dir.join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}
```

Add the shared section renderer + file writers, then refactor `add_convention_in` to use them:

```rust
/// Render `sections` (sid, title, tokens, code) into the front-matter block lines
/// and the JSON section metas for the index.
fn render_sections(secs: &[(String, String, usize, bool)]) -> (String, Vec<serde_json::Value>) {
    let mut fm = String::new();
    let mut idx = Vec::new();
    for (sid, title, tokens, code) in secs {
        fm.push_str(&format!(
            "  - {{ id: {}, title: {}, tokens: {}, code: {} }}\n",
            sid, yaml_scalar(title), tokens, code
        ));
        idx.push(serde_json::json!({ "id": sid, "title": title, "tokens": tokens }));
    }
    (fm, idx)
}

/// Write `<id>.md` (canonical front-matter + the given body, verbatim) and upsert
/// the conventions index.
fn write_convention_files(
    dir: &Path,
    id: &str,
    title: &str,
    description: &str,
    tags: &[String],
    fm_sections: &str,
    index_sections: Vec<serde_json::Value>,
    body: &str,
) -> Result<(), String> {
    validate_doc_id(id)?;
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let fm = format!(
        "---\nid: {}\ntitle: {}\ndescription: {}\nsections:\n{}tags: [{}]\n---\n\n",
        id, yaml_scalar(title), yaml_scalar(description), fm_sections, tags.join(", ")
    );
    fs::write(dir.join(format!("{id}.md")), format!("{fm}{body}"))
        .map_err(|e| format!("write convention: {e}"))?;
    let entry = serde_json::json!({
        "id": id, "title": title, "file": format!("{id}.md"),
        "description": description, "tags": tags, "sections": index_sections,
    });
    upsert_index(&dir.join("_index.json"), "topics", id, entry)
}
```

Refactor `add_convention_in` (preserve byte-identical output — generated body + slug anchors):

```rust
pub fn add_convention_in(dir: &Path, spec: &ConventionSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    let mut secs: Vec<(String, String, usize, bool)> = Vec::new();
    let mut body = format!("# {}\n", spec.title);
    for s in &spec.sections {
        let sid = slug(&s.title);
        let tokens = s.body.len() / 4 + 8;
        body.push_str(&format!("\n## {} {{#{}}}\n{}\n", s.title, sid, s.body.trim()));
        secs.push((sid, s.title.clone(), tokens, s.code));
    }
    let (fm_sections, index_sections) = render_sections(&secs);
    write_convention_files(dir, &spec.id, &spec.title, &spec.description, &spec.tags, &fm_sections, index_sections, &body)
}
```

Add the convention importer:

```rust
/// Import a plain markdown doc as a convention into `dir`. Front-matter supplies
/// title/description/tags/id (title falls back to the first `# H1`); sections are
/// derived from `##` headings; the body is stored verbatim. Returns (id, replaced).
pub fn add_convention_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String> {
    let meta = parse_doc_front_matter(raw)?;
    let body = strip_frontmatter(raw);
    let title = meta.title.clone().or_else(|| first_h1(body)).ok_or_else(|| {
        "convention needs a title: add a `title:` field or a `# Heading`".to_string()
    })?;
    let id = meta.id.clone().unwrap_or_else(|| slug(&title));
    validate_doc_id(&id)?;
    let secs: Vec<(String, String, usize, bool)> = sections(body)
        .iter()
        .map(|s| (slug(&s.title), s.title.clone(), s.body.len() / 4 + 8, s.body.contains("```")))
        .collect();
    if secs.is_empty() {
        eprintln!("warning: no `##` sections found in convention '{id}' — added with an empty outline");
    }
    let replaced = dir.join(format!("{id}.md")).exists();
    let (fm_sections, index_sections) = render_sections(&secs);
    write_convention_files(dir, &id, &title, &meta.description, &meta.tags, &fm_sections, index_sections, body)?;
    Ok((id, replaced))
}
```

Add the recipe writer + importer, and refactor `add_recipe` to delegate:

```rust
fn write_recipe_files(
    dir: &Path,
    id: &str,
    title: &str,
    description: &str,
    tags: &[String],
    body: &str,
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
    });
    upsert_index(&dir.join("_index.json"), "recipes", id, entry)
}

/// Import a plain markdown doc as a recipe into `dir`. Body stored verbatim.
pub fn add_recipe_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String> {
    let meta = parse_doc_front_matter(raw)?;
    let body = strip_frontmatter(raw);
    let title = meta.title.clone().or_else(|| first_h1(body)).ok_or_else(|| {
        "recipe needs a title: add a `title:` field or a `# Heading`".to_string()
    })?;
    let id = meta.id.clone().unwrap_or_else(|| slug(&title));
    validate_doc_id(&id)?;
    let replaced = dir.join(format!("{id}.md")).exists();
    write_recipe_files(dir, &id, &title, &meta.description, &meta.tags, body)?;
    Ok((id, replaced))
}
```

Refactor the existing `add_recipe` to build the generated body (`# title` + blank line + trimmed body) and delegate (preserve byte-identical output):

```rust
pub fn add_recipe(kn: &Path, profile: &str, spec: &RecipeSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    let dir = kn.join("profiles").join(profile).join("recipes");
    let body = format!("# {}\n\n{}\n", spec.title, spec.body.trim());
    write_recipe_files(&dir, &spec.id, &spec.title, &spec.description, &spec.tags, &body)
}
```

(Confirm the original `add_recipe` wrote exactly `...---\n\n# {title}\n\n{body.trim()}\n`; the above reproduces it. If the original had a different trailing newline, match it exactly so the existing recipe test stays green.)

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --bin palugada knowledge::`
Expected: new import/parse tests PASS; existing `add_convention_writes_md_and_index`, `set_body_overwrites_verbatim_and_guards`, `add_convention_in_then_read_and_override_body` still PASS (refactor preserved output).

- [ ] **Step 5: Commit**

```bash
git add src/knowledge.rs
git commit -m "$(printf 'feat(knowledge): markdown import for conventions/recipes + shared writers\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 2: `main.rs` — `convention add` / `recipe add` subcommands

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `knowledge::add_convention_from_markdown`, `knowledge::add_recipe_from_markdown`, `effective::overlay_dir`, `resolve_profile`, `GlobalConfig`.

- [ ] **Step 1: Add the subcommand enums + Commands variants**

In `src/main.rs`, add two `Commands` variants (near `Profile`/`Project`):

```rust
    /// Author a convention from a plain markdown file.
    Convention {
        #[command(subcommand)]
        action: ConventionCmd,
    },
    /// Author a recipe from a plain markdown file.
    Recipe {
        #[command(subcommand)]
        action: RecipeCmd,
    },
```

Add the enums (mirror `ProfileCmd` style):

```rust
#[derive(Subcommand)]
enum ConventionCmd {
    /// Import a markdown file as a convention: `convention add <file.md>`.
    /// Writes to the profile, or to a project's overlay with global `--project`.
    Add {
        /// Path to the markdown file (front-matter + `# Title` + `## Section`s).
        file: String,
        /// Profile override (ignored when `--project` selects an overlay).
        #[arg(long)]
        profile: Option<String>,
    },
}

#[derive(Subcommand)]
enum RecipeCmd {
    /// Import a markdown file as a recipe: `recipe add <file.md>`.
    Add {
        /// Path to the markdown file (front-matter + body).
        file: String,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
}
```

- [ ] **Step 2: Wire dispatch**

In the main `match` (near `Commands::Profile { action } => cmd_profile(action, project)`):

```rust
        Commands::Convention { action } => cmd_convention(action, project),
        Commands::Recipe { action } => cmd_recipe(action, project),
```

- [ ] **Step 3: Implement the handlers**

Add near `cmd_profile`:

```rust
fn cmd_convention(action: ConventionCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        ConventionCmd::Add { file, profile } => {
            if !file.ends_with(".md") {
                return Err(format!("expected a .md file, got '{file}'"));
            }
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let dir = if let Some(name) = project {
                // Per-project convention overlay (cycle C).
                let entry = global.projects.registered.get(name)
                    .ok_or_else(|| format!("project '{name}' is not registered"))?;
                effective::overlay_dir(&entry.repo_path)
            } else {
                let prof = resolve_profile(&global, None, profile.as_deref(), &kn)?;
                kn.join("profiles").join(&prof).join("conventions")
            };
            let (id, replaced) = knowledge::add_convention_from_markdown(&dir, &raw)?;
            let verb = if replaced { "updated" } else { "created" };
            println!("{verb} {id} -> {}", dir.join(format!("{id}.md")).display());
            Ok(())
        }
    }
}

fn cmd_recipe(action: RecipeCmd, project: Option<&str>) -> Result<(), String> {
    match action {
        RecipeCmd::Add { file, profile } => {
            if project.is_some() {
                return Err("recipes are profile-scoped; drop --project and use --profile".to_string());
            }
            if !file.ends_with(".md") {
                return Err(format!("expected a .md file, got '{file}'"));
            }
            let raw = std::fs::read_to_string(&file).map_err(|e| format!("read {file}: {e}"))?;
            let global = GlobalConfig::load_or_default()?;
            let kn = knowledge::knowledge_dir(&global)?;
            let prof = resolve_profile(&global, None, profile.as_deref(), &kn)?;
            let dir = kn.join("profiles").join(&prof).join("recipes");
            let (id, replaced) = knowledge::add_recipe_from_markdown(&dir, &raw)?;
            let verb = if replaced { "updated" } else { "created" };
            println!("{verb} {id} -> {}", dir.join(format!("{id}.md")).display());
            Ok(())
        }
    }
}
```

- [ ] **Step 4: Build + smoke**

Run:
```bash
cargo build 2>&1 | tail -3
printf '%s\n' '---' 'title: Sample Note' 'description: a test convention' 'tags: [rs, demo]' '---' '' '# Sample Note' '> quick summary' '' '## First' 'body one' '' '## Second' '```rust' 'fn x() {}' '```' > /tmp/sample-conv.md
cargo run -q -- convention add /tmp/sample-conv.md --profile rust-cli
cargo run -q -- q sample-note --profile rust-cli
cargo run -q -- q --list --profile rust-cli | grep sample-note
```
Expected: build clean; `created sample-note -> .../rust-cli/conventions/sample-note.md`; `q sample-note` prints the verbatim body (incl. `> quick summary`); `q --list` shows it. **Then revert the demo** so it isn't committed: `git -C knowledge checkout . 2>/dev/null; rm -f knowledge/profiles/rust-cli/conventions/sample-note.md` and restore `_index.json` (`git checkout knowledge/profiles/rust-cli/conventions/_index.json`).

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "$(printf 'feat(cli): convention add / recipe add — import plain markdown\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 3: End-to-end verification

**Files:** none (verification only).

- [ ] **Step 1: Full build + test**

Run: `cargo test --release && cargo build --release`
Expected: all tests pass (≥97 + new), no warnings, release builds.

- [ ] **Step 2: Live e2e — profile target + upsert**

```bash
BIN="$PWD/target/release/palugada"; KN="$PWD/knowledge"
printf '%s\n' '---' 'title: Demo Rule' 'description: first version' 'tags: [rs]' '---' '' '# Demo Rule' '## A' 'body a' > /tmp/demo.md
PALUGADA_KNOWLEDGE="$KN" "$BIN" convention add /tmp/demo.md --profile rust-cli   # created demo-rule
PALUGADA_KNOWLEDGE="$KN" "$BIN" convention add /tmp/demo.md --profile rust-cli   # updated demo-rule
PALUGADA_KNOWLEDGE="$KN" "$BIN" q demo-rule --profile rust-cli | head
# cleanup: remove the demo convention + restore the index
rm -f "$KN/profiles/rust-cli/conventions/demo-rule.md"
git checkout "$KN/profiles/rust-cli/conventions/_index.json"
```
Expected: first run `created`, second `updated`; `q demo-rule` shows the body. Cleanup leaves the rust-cli profile pristine (`git status --porcelain knowledge/` clean).

- [ ] **Step 3: Live e2e — overlay target + recipe**

```bash
BIN="$PWD/target/release/palugada"; KN="$PWD/knowledge"
# overlay: write to THIS repo's .palugada/conventions (it is a registered/bound project? use --project if registered, else a scratch)
printf '%s\n' '---' 'title: Team Rule' 'tags: [rs]' '---' '# Team Rule' '## X' 'x' > /tmp/team.md
# (register a scratch project or reuse one; verify origin=project via effective_rules)
# recipe to profile:
printf '%s\n' '---' 'title: Quick Recipe' 'description: demo' 'tags: [demo]' '---' '# Quick Recipe' 'do this' > /tmp/rec.md
PALUGADA_KNOWLEDGE="$KN" "$BIN" recipe add /tmp/rec.md --profile rust-cli       # created quick-recipe
PALUGADA_KNOWLEDGE="$KN" "$BIN" recipe add /tmp/team.md --project anything       # ERROR: recipes are profile-scoped
PALUGADA_KNOWLEDGE="$KN" "$BIN" for --list --profile rust-cli | grep quick-recipe
# cleanup
rm -f "$KN/profiles/rust-cli/recipes/quick-recipe.md"
git checkout "$KN/profiles/rust-cli/recipes/_index.json"
```
Expected: recipe `created quick-recipe`; `recipe add --project` errors with the profile-scoped message; cleanup leaves `knowledge/` clean. (For the convention-overlay path, exercise it against a registered scratch project as in earlier e2e and confirm `palugada project rules <name>` shows the imported convention with origin `project`, then remove the project + its `.palugada/`.)

- [ ] **Step 4: Confirm working tree clean**

Run: `git status --porcelain`
Expected: empty (all demo artifacts cleaned up; only the committed source changes remain on the branch).

---

## Self-review

**Spec coverage:**
- Input contract (front-matter title/description/tags/id) → Task 1 `parse_doc_front_matter` + importers.
- Auto-mapping (sections/tokens/code/slug/_index) → Task 1 `add_convention_from_markdown` + `render_sections`.
- Verbatim body → Task 1 (writers take `body` param; importer passes `strip_frontmatter(raw)`).
- Title fallback + id derivation + validation → Task 1 (`first_h1`, `slug`, `validate_doc_id`).
- Commands `convention add` / `recipe add`, profile vs overlay target, recipe rejects `--project`, upsert created/updated → Task 2.
- Error handling (missing file, not `.md`, no title, bad id, unregistered project) → Task 2 handlers + Task 1 errors.
- Tests + e2e → Task 1 unit tests + Task 3.

**Placeholder scan:** All code steps contain concrete code. The only guarded note is "confirm `add_recipe` original trailing newline matches" — a byte-fidelity check with the exact target shown, not an open TODO.

**Type consistency:** `add_convention_from_markdown`/`add_recipe_from_markdown` return `(String, bool)` used identically in Task 2. `DocMeta { id: Option<String>, title: Option<String>, description: String, tags: Vec<String> }` consistent across parser + importers. `render_sections(&[(String,String,usize,bool)]) -> (String, Vec<Value>)` and `write_convention_files(...)`/`write_recipe_files(...)` signatures match their call sites in `add_convention_in`/`add_recipe`/importers. `recipes_in`/`recipe_md_in` mirror the existing `conventions_in`/`convention_md_in`.
