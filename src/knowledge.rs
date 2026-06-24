//! Knowledge reads (step 1 of the knowledge layer): conventions (`q`),
//! recipes (`for`), and a keyword search (`s`). These read the bundled profile
//! at `knowledge/profiles/<profile>/` — no indexer required.

use crate::config::{expand_home, GlobalConfig};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

// ── locating the knowledge/ dir ───────────────────────────────────────────

/// Resolve the `knowledge/` directory (the one containing `profiles/`).
/// Order: `PALUGADA_KNOWLEDGE` env → config `engine.knowledge_path` →
/// walk up from the executable → walk up from the cwd.
pub fn knowledge_dir(global: &GlobalConfig) -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("PALUGADA_KNOWLEDGE") {
        if !p.is_empty() {
            return require_profiles(expand_home(&p));
        }
    }
    if !global.engine.knowledge_path.is_empty() {
        return require_profiles(expand_home(&global.engine.knowledge_path));
    }
    if let Some(found) = detect_knowledge_dir() {
        return Ok(found);
    }
    Err(
        "can't locate the knowledge/ directory — set `engine.knowledge_path` in \
~/.palugada.yaml or the PALUGADA_KNOWLEDGE env var (running `palugada config init` \
from inside the palugada repo auto-detects it)"
            .to_string(),
    )
}

/// Best-effort auto-detection by walking up from the executable, then the cwd,
/// looking for a directory that contains `knowledge/profiles`.
pub fn detect_knowledge_dir() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        // Resolve symlinks first: a launcher symlinked onto PATH (install.sh's
        // ~/.local/bin/palugada -> ~/.local/share/palugada/palugada, or
        // Homebrew/Scoop's bin shim) reports the symlink path on macOS, so we
        // must canonicalize to the real binary to find the adjacent knowledge/.
        let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
        if let Some(found) = walk_up(&exe) {
            return Some(found);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(found) = walk_up(&cwd) {
            return Some(found);
        }
    }
    None
}

fn walk_up(start: &Path) -> Option<PathBuf> {
    let mut cur = start.to_path_buf();
    loop {
        if cur.join("knowledge").join("profiles").is_dir() {
            return Some(cur.join("knowledge"));
        }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None => return None,
        }
    }
}

fn require_profiles(kn: PathBuf) -> Result<PathBuf, String> {
    if kn.join("profiles").is_dir() {
        Ok(kn)
    } else {
        Err(format!("{} has no profiles/ subdirectory", kn.display()))
    }
}

/// If exactly one profile is bundled, return its id (used as a last-resort
/// default so `palugada q` works out of the box).
pub fn only_profile(kn: &Path) -> Option<String> {
    let dir = kn.join("profiles");
    let entries = fs::read_dir(&dir).ok()?;
    let mut found: Option<String> = None;
    for e in entries.flatten() {
        if e.path().is_dir() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('_') {
                continue; // skip _template, _index, etc.
            }
            if found.is_some() {
                return None; // more than one
            }
            found = Some(name);
        }
    }
    found
}

// ── _index.json shapes ────────────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct ConvIndex {
    #[serde(default)]
    topics: Vec<ConvTopic>,
}

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

#[derive(Deserialize, Default)]
struct RecipeIndex {
    #[serde(default)]
    recipes: Vec<RecipeEntry>,
}

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

/// Read `<conv_dir>/_index.json`; a missing dir/file yields an empty index
/// (a project with no convention overlay).
fn read_conv_index_in(conv_dir: &Path) -> Result<ConvIndex, String> {
    let p = conv_dir.join("_index.json");
    if !p.exists() {
        return Ok(ConvIndex::default());
    }
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
}

fn read_recipe_index(kn: &Path, profile: &str) -> Result<RecipeIndex, String> {
    let p = kn
        .join("profiles")
        .join(profile)
        .join("recipes")
        .join("_index.json");
    if !p.exists() {
        return Ok(RecipeIndex::default());
    }
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
}

// ── data accessors (typed; for the web console / programmatic use) ──────────

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
    /// Provenance (filled only by inherit::*_provenance; empty for plain reads).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub origin: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub from: String,
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

/// Conventions in an arbitrary conventions dir (profile or per-project overlay)
/// as data. A missing dir/index yields an empty list.
pub fn conventions_in(conv_dir: &Path) -> Result<Vec<TopicMeta>, String> {
    Ok(read_conv_index_in(conv_dir)?
        .topics
        .into_iter()
        .map(|t| TopicMeta {
            id: t.id,
            title: t.title,
            description: t.description,
            tags: t.tags,
            sections: t.sections,
            related: t.related,
            origin: String::new(),
            from: String::new(),
        })
        .collect())
}

/// The profile's conventions as data (id/title/description/tags + section titles).
pub fn conventions(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    conventions_in(&kn.join("profiles").join(profile).join("conventions"))
}

/// The profile's recipes as data.
pub fn recipes(kn: &Path, profile: &str) -> Result<Vec<RecipeMeta>, String> {
    Ok(read_recipe_index(kn, profile)?
        .recipes
        .into_iter()
        .map(|r| RecipeMeta {
            id: r.id,
            title: r.title,
            description: r.description,
            tags: r.tags,
            convention_refs: r.convention_refs,
            related_recipes: r.related_recipes,
            origin: String::new(),
            from: String::new(),
        })
        .collect())
}

/// Raw markdown of one convention file in an arbitrary conventions dir.
pub fn convention_md_in(conv_dir: &Path, id: &str) -> Result<String, String> {
    let p = conv_dir.join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

/// Raw markdown of one convention file (the profile's OWN local body, un-merged).
/// Used by the web `…/convention/<id>/raw` route so edits read/write the child's
/// verbatim file; merged reads go through inherit::resolve_convention_raw.
pub fn convention_md(kn: &Path, profile: &str, id: &str) -> Result<String, String> {
    convention_md_in(&kn.join("profiles").join(profile).join("conventions"), id)
}

/// Raw markdown of one recipe file (the profile's OWN local body, un-merged).
/// Used by the web `…/recipe/<id>/raw` route; merged reads go through
/// inherit::resolve_recipe_raw.
pub fn recipe_md(kn: &Path, profile: &str, id: &str) -> Result<String, String> {
    let p = kn
        .join("profiles")
        .join(profile)
        .join("recipes")
        .join(format!("{id}.md"));
    fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))
}

// ── writers (author conventions/recipes from the web console) ──────────────

#[derive(serde::Deserialize)]
pub struct SectionSpec {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub code: bool,
}

#[derive(serde::Deserialize)]
pub struct ConventionSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub sections: Vec<SectionSpec>,
}

#[derive(serde::Deserialize)]
pub struct RecipeSpec {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub body: String,
}

/// Kebab-case a heading: lowercase, runs of non-alphanumeric → single '-', trimmed.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !out.is_empty() && !dash {
            out.push('-');
            dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

fn validate_doc_id(id: &str) -> Result<(), String> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(format!("invalid id '{id}' — use only [a-z0-9_-]"));
    }
    Ok(())
}

/// Quote a YAML scalar if it could be misparsed (Rust debug = double-quoted).
fn yaml_scalar(s: &str) -> String {
    if s.is_empty() || s.contains(['"', ':', '#', '\n']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("{s:?}")
    } else {
        s.to_string()
    }
}

/// Insert-or-replace an object (matched by `id`) in a JSON index file's array.
fn upsert_index(
    path: &Path,
    array_key: &str,
    id: &str,
    entry: serde_json::Value,
) -> Result<(), String> {
    let mut root: serde_json::Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
            .map_err(|e| format!("parse {}: {e}", path.display()))?
    } else {
        serde_json::json!({ "schema_version": "1.0", array_key: [] })
    };
    let arr = root
        .get_mut(array_key)
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| format!("{} has no '{array_key}' array", path.display()))?;
    arr.retain(|e| e.get("id").and_then(|v| v.as_str()) != Some(id));
    arr.push(entry);
    let out = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    fs::write(path, out + "\n").map_err(|e| format!("write {}: {e}", path.display()))
}

/// Author a convention: write `<id>.md` (front-matter + `## Title {#slug}`
/// sections) and upsert it into `conventions/_index.json`.
pub fn add_convention(kn: &Path, profile: &str, spec: &ConventionSpec) -> Result<(), String> {
    add_convention_in(&kn.join("profiles").join(profile).join("conventions"), spec)
}

/// Author a convention into an arbitrary conventions dir (profile or overlay):
/// write `<id>.md` (front-matter + `## Title {#slug}` sections) and upsert it
/// into that dir's `_index.json`.
pub fn add_convention_in(dir: &Path, spec: &ConventionSpec) -> Result<(), String> {
    validate_doc_id(&spec.id)?;
    let mut secs: Vec<(String, String, usize, bool)> = Vec::new();
    let mut body = format!("# {}\n", spec.title);
    for s in &spec.sections {
        let sid = slug(&s.title);
        let tokens = s.body.len() / 4 + 8;
        body.push_str(&format!(
            "\n## {} {{#{}}}\n{}\n",
            s.title,
            sid,
            s.body.trim()
        ));
        secs.push((sid, s.title.clone(), tokens, s.code));
    }
    let (fm_sections, index_sections) = render_sections(&secs);
    write_convention_files(
        dir,
        &spec.id,
        &spec.title,
        &spec.description,
        &spec.tags,
        &fm_sections,
        index_sections,
        &body,
    )
}

/// Render `sections` (sid, title, tokens, code) into the front-matter block lines
/// and the JSON section metas for the index.
fn render_sections(secs: &[(String, String, usize, bool)]) -> (String, Vec<serde_json::Value>) {
    let mut fm = String::new();
    let mut idx = Vec::new();
    for (sid, title, tokens, code) in secs {
        fm.push_str(&format!(
            "  - {{ id: {}, title: {}, tokens: {}, code: {} }}\n",
            sid,
            yaml_scalar(title),
            tokens,
            code
        ));
        idx.push(serde_json::json!({ "id": sid, "title": title, "tokens": tokens }));
    }
    (fm, idx)
}

/// Write `<id>.md` (canonical front-matter + the given body, verbatim) and upsert
/// the conventions index.
#[allow(clippy::too_many_arguments)]
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
        id,
        yaml_scalar(title),
        yaml_scalar(description),
        fm_sections,
        tags.join(", ")
    );
    fs::write(dir.join(format!("{id}.md")), format!("{fm}{body}"))
        .map_err(|e| format!("write convention: {e}"))?;
    let entry = serde_json::json!({
        "id": id, "title": title, "file": format!("{id}.md"),
        "description": description, "tags": tags, "sections": index_sections,
    });
    upsert_index(&dir.join("_index.json"), "topics", id, entry)
}

/// Import a plain markdown doc as a convention into `dir`. Front-matter supplies
/// title/description/tags/id (title falls back to the first `# H1`); sections are
/// derived from `##` headings; the body is stored verbatim. Returns (id, replaced).
pub fn add_convention_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String> {
    let meta = parse_doc_front_matter(raw)?;
    let body = strip_frontmatter(raw);
    let title = meta
        .title
        .clone()
        .or_else(|| first_h1(body))
        .ok_or_else(|| {
            "convention needs a title: add a `title:` field or a `# Heading`".to_string()
        })?;
    let id = meta.id.clone().unwrap_or_else(|| slug(&title));
    validate_doc_id(&id)?;
    let secs: Vec<(String, String, usize, bool)> = sections(body)
        .iter()
        .map(|s| {
            (
                slug(&s.title),
                s.title.clone(),
                s.body.len() / 4 + 8,
                s.body.contains("```"),
            )
        })
        .collect();
    if secs.is_empty() {
        eprintln!(
            "warning: no `##` sections found in convention '{id}' — added with an empty outline"
        );
    }
    let replaced = dir.join(format!("{id}.md")).exists();
    let (fm_sections, index_sections) = render_sections(&secs);
    // Inject `{#slug}` anchors so imported conventions match the hand-authored
    // anchored style (section ids already use slug(title), so they line up).
    let body_out = inject_anchors(body);
    write_convention_files(
        dir,
        &id,
        &title,
        &meta.description,
        &meta.tags,
        &fm_sections,
        index_sections,
        &body_out,
    )?;
    Ok((id, replaced))
}

/// Author a recipe: write `<id>.md` and upsert it into `recipes/_index.json`.
pub fn add_recipe(kn: &Path, profile: &str, spec: &RecipeSpec) -> Result<(), String> {
    let dir = kn.join("profiles").join(profile).join("recipes");
    let body = format!("# {}\n\n{}\n", spec.title, spec.body.trim());
    write_recipe_files(
        &dir,
        &spec.id,
        &spec.title,
        &spec.description,
        &spec.tags,
        &body,
    )
}

/// Write `<id>.md` (front-matter + the given body, verbatim) and upsert the
/// recipes index.
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
        id,
        yaml_scalar(title),
        yaml_scalar(description),
        tags.join(", "),
        body
    );
    fs::write(dir.join(format!("{id}.md")), md).map_err(|e| format!("write recipe: {e}"))?;
    let entry = serde_json::json!({
        "id": id, "title": title, "description": description,
        "file": format!("{id}.md"), "tags": tags,
    });
    upsert_index(&dir.join("_index.json"), "recipes", id, entry)
}

/// Import a plain markdown doc as a recipe into `dir`. Front-matter supplies
/// title/description/tags/id (title falls back to the first `# H1`); the body is
/// stored verbatim. Returns (id, replaced).
pub fn add_recipe_from_markdown(dir: &Path, raw: &str) -> Result<(String, bool), String> {
    let meta = parse_doc_front_matter(raw)?;
    let body = strip_frontmatter(raw);
    let title = meta
        .title
        .clone()
        .or_else(|| first_h1(body))
        .ok_or_else(|| "recipe needs a title: add a `title:` field or a `# Heading`".to_string())?;
    let id = meta.id.clone().unwrap_or_else(|| slug(&title));
    validate_doc_id(&id)?;
    let replaced = dir.join(format!("{id}.md")).exists();
    write_recipe_files(dir, &id, &title, &meta.description, &meta.tags, body)?;
    Ok((id, replaced))
}

/// Overwrite an existing convention's markdown verbatim (edit-only).
pub fn set_convention_body(
    kn: &Path,
    profile: &str,
    id: &str,
    markdown: &str,
) -> Result<(), String> {
    set_convention_body_in(
        &kn.join("profiles").join(profile).join("conventions"),
        id,
        markdown,
    )
}

/// Overwrite an existing convention's markdown verbatim in an arbitrary dir
/// (edit-only); errors if the file does not already exist.
pub fn set_convention_body_in(conv_dir: &Path, id: &str, markdown: &str) -> Result<(), String> {
    validate_doc_id(id)?;
    let p = conv_dir.join(format!("{id}.md"));
    if !p.exists() {
        return Err(format!(
            "convention '{id}' does not exist in {}",
            conv_dir.display()
        ));
    }
    fs::write(&p, markdown).map_err(|e| format!("write {}: {e}", p.display()))
}

/// Overwrite an existing recipe's markdown verbatim (edit-only).
pub fn set_recipe_body(kn: &Path, profile: &str, id: &str, markdown: &str) -> Result<(), String> {
    set_doc_body(kn, profile, "recipes", id, markdown)
}

/// Write `<dir>/<id>.md` verbatim; errors if it doesn't already exist (edit-only),
/// leaving the `_index.json` metadata (title/description/tags) untouched.
fn set_doc_body(
    kn: &Path,
    profile: &str,
    dir: &str,
    id: &str,
    markdown: &str,
) -> Result<(), String> {
    validate_doc_id(id)?;
    let p = kn
        .join("profiles")
        .join(profile)
        .join(dir)
        .join(format!("{id}.md"));
    if !p.exists() {
        let what = dir.strip_suffix('s').unwrap_or(dir);
        return Err(format!(
            "{what} '{id}' does not exist in profile '{profile}'"
        ));
    }
    fs::write(&p, markdown).map_err(|e| format!("write {}: {e}", p.display()))
}

// ── q: conventions ──────────────────────────────────────────────────────

/// A section selector parsed off a `q` topic argument.
enum Sel {
    Index(usize),
    Anchor(String),
}

/// "arch#data-flow" → ("arch", Anchor("data-flow")); "arch.2" → ("arch", Index(2));
/// "arch" → ("arch", None).
fn parse_topic_arg(arg: &str) -> (&str, Option<Sel>) {
    if let Some((name, rest)) = arg.rsplit_once('#') {
        if !rest.is_empty() {
            return (name, Some(Sel::Anchor(rest.to_string())));
        }
    }
    if let Some((name, rest)) = arg.rsplit_once('.') {
        if let Ok(n) = rest.parse::<usize>() {
            return (name, Some(Sel::Index(n)));
        }
    }
    (arg, None)
}

pub fn list_topics(kn: &Path, profile: &str) -> Result<(), String> {
    let topics = crate::inherit::merged_conventions(kn, profile)?; // defined in Task 4
    if topics.is_empty() {
        println!("(no conventions in profile '{profile}')");
        return Ok(());
    }
    println!("Conventions in profile '{profile}':");
    for t in &topics {
        println!("  {:<16} {}", t.id, t.description);
    }
    Ok(())
}

pub fn query(kn: &Path, profile: &str, topic_arg: &str, brief: bool) -> Result<(), String> {
    let (name, sel) = parse_topic_arg(topic_arg);
    let raw = crate::inherit::resolve_convention_raw(kn, profile, name)?
        .ok_or_else(|| format!("no convention '{name}' in profile '{profile}' or its parents"))?;
    let body = strip_frontmatter(&raw);

    if brief {
        println!("{}", convention_outline_str(&raw, name));
        return Ok(());
    }

    match sel {
        Some(Sel::Index(n)) => {
            let secs = crate::inherit::parse_sections(body);
            let s = secs.get(n.saturating_sub(1)).ok_or_else(|| {
                format!("section {n} not found in '{name}' (it has {})", secs.len())
            })?;
            println!("## {}\n\n{}", s.title, s.body.trim());
        }
        Some(Sel::Anchor(a)) => {
            let secs = crate::inherit::parse_sections(body);
            let s = secs.iter().find(|s| s.anchor == a).ok_or_else(|| {
                format!(
                    "section '#{a}' not found in '{name}' (sections: {})",
                    secs.iter()
                        .map(|s| s.anchor.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })?;
            println!("## {}\n\n{}", s.title, s.body.trim());
        }
        None => println!("{}", body.trim()),
    }
    Ok(())
}

// ── for: recipes ──────────────────────────────────────────────────────────

pub fn list_recipes(kn: &Path, profile: &str) -> Result<(), String> {
    let recipes = crate::inherit::merged_recipes(kn, profile)?;
    if recipes.is_empty() {
        println!("(no recipes in profile '{profile}')");
        return Ok(());
    }
    println!("Recipes in profile '{profile}':");
    for r in &recipes {
        println!("  {:<16} {}", r.id, r.description);
    }
    Ok(())
}

pub fn recipe(kn: &Path, profile: &str, task: &str) -> Result<(), String> {
    println!("{}", recipe_body(kn, profile, task)?);
    Ok(())
}

// ── string-returning variants (used by `brief`) ──────────────────────────

/// Outline string for a convention: description + numbered section titles.
fn convention_outline_str(raw: &str, name: &str) -> String {
    let body = strip_frontmatter(raw);
    let desc = frontmatter_field(raw, "description").unwrap_or_default();
    let secs = sections(body);
    let mut out = format!("{name} — {desc}\n({} sections)\n", secs.len());
    for (i, s) in secs.iter().enumerate() {
        out.push_str(&format!("  {}. {}\n", i + 1, s.title));
    }
    out.push_str(&format!("Drill in with `palugada q {name}.<N>`."));
    out
}

/// Outline (description + section titles) of one convention in an arbitrary dir.
pub fn convention_outline_in(conv_dir: &Path, name: &str) -> Result<String, String> {
    let raw = convention_md_in(conv_dir, name)
        .map_err(|_| format!("no convention '{name}' in {}", conv_dir.display()))?;
    Ok(convention_outline_str(&raw, name))
}

pub fn convention_outline(kn: &Path, profile: &str, name: &str) -> Result<String, String> {
    let raw = crate::inherit::resolve_convention_raw(kn, profile, name)?
        .ok_or_else(|| format!("no convention '{name}' in profile '{profile}' or its parents"))?;
    Ok(convention_outline_str(&raw, name))
}

pub fn recipe_body(kn: &Path, profile: &str, task: &str) -> Result<String, String> {
    let raw = crate::inherit::resolve_recipe_raw(kn, profile, task)?
        .ok_or_else(|| format!("no recipe '{task}' in profile '{profile}' or its parents"))?;
    Ok(strip_frontmatter(&raw).trim().to_string())
}

// ── s: keyword search across the index ─────────────────────────────────────

pub fn search(kn: &Path, profile: &str, kw: &str) -> Result<(), String> {
    let needle = kw.to_lowercase();
    let mut hits = 0;

    for t in crate::inherit::merged_conventions(kn, profile).unwrap_or_default() {
        let hay = format!(
            "{} {} {} {} {}",
            t.id,
            t.title,
            t.description,
            t.tags.join(" "),
            t.sections
                .iter()
                .map(|s| s.title.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        )
        .to_lowercase();
        if hay.contains(&needle) {
            println!("[convention] {:<16} {}", t.id, t.description);
            hits += 1;
        }
    }
    for r in crate::inherit::merged_recipes(kn, profile).unwrap_or_default() {
        let hay = format!(
            "{} {} {} {}",
            r.id,
            r.title,
            r.description,
            r.tags.join(" ")
        )
        .to_lowercase();
        if hay.contains(&needle) {
            println!("[recipe]     {:<16} {}", r.id, r.description);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("No matches for '{kw}' in profile '{profile}'.");
    }
    Ok(())
}

// ── markdown helpers ────────────────────────────────────────────────────

struct Section {
    title: String,
    body: String,
}

/// Return the markdown body with the leading YAML front-matter removed.
pub fn strip_frontmatter(raw: &str) -> &str {
    let t = raw.trim_start();
    if let Some(rest) = t.strip_prefix("---") {
        if let Some(idx) = rest.find("\n---") {
            let after = &rest[idx + "\n---".len()..];
            if let Some(nl) = after.find('\n') {
                return after[nl + 1..].trim_start_matches('\n');
            }
            return "";
        }
    }
    raw
}

/// Read a single scalar field out of the YAML front-matter (best-effort).
pub fn frontmatter_field(raw: &str, key: &str) -> Option<String> {
    let t = raw.trim_start();
    let rest = t.strip_prefix("---")?;
    let idx = rest.find("\n---")?;
    let fm = &rest[..idx];
    let prefix = format!("{key}:");
    for line in fm.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix(&prefix) {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

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

/// Append `{#slug}` anchors to `## ` headings that lack one (outside code fences),
/// so imported conventions match the hand-authored anchored style. Other lines —
/// including the body prose and fenced `## ` — are passed through untouched.
fn inject_anchors(body: &str) -> String {
    let mut out = String::new();
    let mut in_fence = false;
    for line in body.split_inclusive('\n') {
        let content = line.trim_end_matches('\n');
        if content.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if !in_fence {
            if let Some(rest) = content.strip_prefix("## ") {
                let title = rest.trim();
                if !title.is_empty() && !title.contains("{#") {
                    out.push_str(&format!("## {} {{#{}}}", title, slug(title)));
                    if line.ends_with('\n') {
                        out.push('\n');
                    }
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    out
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

/// True if `id` is a valid doc id (`[a-z0-9_-]`, non-empty).
pub fn valid_doc_id(id: &str) -> bool {
    validate_doc_id(id).is_ok()
}

/// A candidate convention parsed out of an uploaded markdown document.
#[derive(serde::Serialize, Debug, PartialEq)]
pub struct ConventionDraft {
    pub id: String,
    pub title: String,
    pub sections: Vec<String>,
    pub body: String,
}

/// `# Heading` (level-1) but not `## ...`.
fn is_h1(line: &str) -> bool {
    line.trim_start()
        .strip_prefix("# ")
        .map(|r| !r.trim().is_empty())
        .unwrap_or(false)
}

fn h1_text(line: &str) -> String {
    line.trim_start()
        .strip_prefix("# ")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Split a markdown document into candidate conventions, one per `# H1`
/// (fence-aware). No `# H1` → a single draft over the whole body, titled from
/// the file front-matter (else empty). Pure; no I/O.
pub fn split_markdown_conventions(raw: &str) -> Vec<ConventionDraft> {
    let file_meta = parse_doc_front_matter(raw).unwrap_or_default();
    let body = strip_frontmatter(raw);
    let lines: Vec<&str> = body.lines().collect();

    let mut in_fence = false;
    let mut h1_idx: Vec<usize> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && is_h1(line) {
            h1_idx.push(i);
        }
    }

    if h1_idx.is_empty() {
        let title = file_meta.title.unwrap_or_default();
        let id = if title.is_empty() {
            String::new()
        } else {
            slug(&title)
        };
        let b = body.trim().to_string();
        let secs = sections(&b).into_iter().map(|s| s.title).collect();
        return vec![ConventionDraft {
            id,
            title,
            sections: secs,
            body: b,
        }];
    }

    let mut drafts = Vec::new();
    for (k, &start) in h1_idx.iter().enumerate() {
        let end = h1_idx.get(k + 1).copied().unwrap_or(lines.len());
        let title = h1_text(lines[start]);
        let id = slug(&title);
        let piece_body = lines[start + 1..end].join("\n").trim().to_string();
        let secs = sections(&piece_body).into_iter().map(|s| s.title).collect();
        drafts.push(ConventionDraft {
            id,
            title,
            sections: secs,
            body: piece_body,
        });
    }
    drafts
}

/// Split a markdown body into `## ` sections (anchors stripped from titles).
/// Lines inside ``` fences are body text, never headers.
fn sections(body: &str) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    let mut cur: Option<Section> = None;
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            if let Some(rest) = line.strip_prefix("## ") {
                if let Some(s) = cur.take() {
                    out.push(s);
                }
                let title = rest.split("{#").next().unwrap_or(rest).trim().to_string();
                cur = Some(Section {
                    title,
                    body: String::new(),
                });
                continue;
            }
        }
        if let Some(s) = cur.as_mut() {
            s.body.push_str(line);
            s.body.push('\n');
        }
    }
    if let Some(s) = cur.take() {
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_kebabs_titles() {
        assert_eq!(slug("Errors in Coroutines"), "errors-in-coroutines");
        assert_eq!(slug("Sealed UiState!"), "sealed-uistate");
    }

    #[test]
    fn set_body_overwrites_verbatim_and_guards() {
        let kn = tempfile::tempdir().unwrap();
        let kp = kn.path();
        add_convention(
            kp,
            "p",
            &ConventionSpec {
                id: "arch".into(),
                title: "Arch".into(),
                description: "d".into(),
                tags: vec!["t".into()],
                sections: vec![SectionSpec {
                    title: "S".into(),
                    body: "old".into(),
                    code: false,
                }],
            },
        )
        .unwrap();
        set_convention_body(kp, "p", "arch", "# brand new body\n").unwrap();
        assert!(convention_md(kp, "p", "arch")
            .unwrap()
            .contains("brand new body"));
        // _index.json metadata preserved
        let cs = conventions(kp, "p").unwrap();
        let arch = cs.iter().find(|c| c.id == "arch").unwrap();
        assert_eq!(arch.title, "Arch");
        assert_eq!(arch.tags, vec!["t".to_string()]);
        // edit-only guard
        assert!(set_convention_body(kp, "p", "missing", "x").is_err());

        add_recipe(
            kp,
            "p",
            &RecipeSpec {
                id: "pag".into(),
                title: "Pag".into(),
                description: "".into(),
                tags: vec![],
                body: "steps".into(),
            },
        )
        .unwrap();
        set_recipe_body(kp, "p", "pag", "# r\nfresh\n").unwrap();
        assert!(recipe_md(kp, "p", "pag").unwrap().contains("fresh"));
        assert!(set_recipe_body(kp, "p", "nope", "x").is_err());
    }

    #[test]
    fn add_convention_writes_md_and_index() {
        let kn = tempfile::tempdir().unwrap();
        let c = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(
            c.join("_index.json"),
            r#"{"schema_version":"1.0","topics":[]}"#,
        )
        .unwrap();
        let spec = ConventionSpec {
            id: "errorhandling".into(),
            title: "Error Handling".into(),
            description: "Handle failures.".into(),
            tags: vec!["error".into()],
            sections: vec![SectionSpec {
                title: "Modeling Failures".into(),
                body: "Model errors explicitly.".into(),
                code: false,
            }],
        };
        add_convention(kn.path(), "p", &spec).unwrap();
        let md = convention_md(kn.path(), "p", "errorhandling").unwrap();
        assert!(
            md.contains("## Modeling Failures {#modeling-failures}"),
            "{md}"
        );
        assert!(md.contains("id: errorhandling"));
        let topics = conventions(kn.path(), "p").unwrap();
        assert_eq!(topics.len(), 1);
        assert_eq!(
            topics[0]
                .sections
                .iter()
                .map(|s| s.title.clone())
                .collect::<Vec<_>>(),
            vec!["Modeling Failures".to_string()]
        );
        assert_eq!(topics[0].sections[0].id, "modeling-failures");
        // re-adding the same id replaces, not duplicates
        add_convention(kn.path(), "p", &spec).unwrap();
        assert_eq!(conventions(kn.path(), "p").unwrap().len(), 1);
    }

    #[test]
    fn conventions_accessor_reads_index() {
        let kn = tempfile::tempdir().unwrap();
        let c = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&c).unwrap();
        std::fs::write(c.join("_index.json"),
            r#"{"topics":[{"id":"arch","title":"Arch","description":"d","tags":["x"],"sections":[{"id":"o","title":"Overview"}]}]}"#).unwrap();
        let v = conventions(kn.path(), "p").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "arch");
        assert_eq!(
            v[0].sections
                .iter()
                .map(|s| s.title.clone())
                .collect::<Vec<_>>(),
            vec!["Overview".to_string()]
        );
        assert_eq!(v[0].sections[0].id, "o");
    }

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
            sections: vec![SectionSpec {
                title: "Rule".into(),
                body: "do X".into(),
                code: false,
            }],
        };
        add_convention_in(&dir, &spec).unwrap();
        let metas = conventions_in(&dir).unwrap();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].id, "ours");
        assert!(convention_md_in(&dir, "ours").unwrap().contains("do X"));
        assert!(convention_outline_in(&dir, "ours")
            .unwrap()
            .contains("team rule"));

        set_convention_body_in(&dir, "ours", "---\nid: ours\n---\n# Ours\nnew body\n").unwrap();
        assert!(convention_md_in(&dir, "ours").unwrap().contains("new body"));

        // edit-only: unknown id errors
        assert!(set_convention_body_in(&dir, "nope", "x").is_err());
    }

    #[test]
    fn sections_ignores_headers_inside_code_fences() {
        let body = "## One\ntext\n```sh\n## not a header\n```\nmore\n## Two\nend\n";
        let secs = sections(body);
        assert_eq!(
            secs.len(),
            2,
            "{:?}",
            secs.iter().map(|s| &s.title).collect::<Vec<_>>()
        );
        assert_eq!(secs[0].title, "One");
        assert!(secs[0].body.contains("## not a header"));
        assert_eq!(secs[1].title, "Two");
    }

    #[test]
    fn parse_front_matter_reads_fields_and_defaults() {
        let raw = "---\ntitle: Error Handling\ndescription: do X\ntags: [rs, error]\n---\n\n# Error Handling\nbody\n";
        let m = parse_doc_front_matter(raw).unwrap();
        assert_eq!(m.title.as_deref(), Some("Error Handling"));
        assert_eq!(m.description, "do X");
        assert_eq!(m.tags, vec!["rs".to_string(), "error".to_string()]);
        assert_eq!(m.id, None);
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
        let metas = conventions_in(&dir).unwrap();
        let m = metas.iter().find(|c| c.id == "error-handling").unwrap();
        assert_eq!(m.title, "Error Handling");
        assert_eq!(
            m.sections
                .iter()
                .map(|s| s.title.clone())
                .collect::<Vec<_>>(),
            vec!["Result Type".to_string(), "With Code".to_string()]
        );
        let md = convention_md_in(&dir, "error-handling").unwrap();
        assert!(
            md.contains("> summary"),
            "verbatim body must keep the blockquote: {md}"
        );
        assert!(md.contains("## Result Type"));
        let (_id2, replaced2) = add_convention_from_markdown(&dir, raw).unwrap();
        assert!(replaced2);
    }

    #[test]
    fn split_one_h1_into_one_draft_with_sections() {
        let raw = "---\ntags: [fb]\n---\n\n# Firebase Integration\n> intro\n\n## Setup\na\n\n## Auth\nb\n";
        let d = split_markdown_conventions(raw);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].id, "firebase-integration");
        assert_eq!(d[0].title, "Firebase Integration");
        assert_eq!(d[0].sections, vec!["Setup".to_string(), "Auth".to_string()]);
        assert!(d[0].body.contains("> intro"));
        assert!(!d[0].body.starts_with("# "), "H1 line excluded from body");
    }

    #[test]
    fn split_multiple_h1_into_separate_drafts() {
        let raw = "# Firebase Auth\n## Sign In\nx\n# Firestore\n## Query\ny\n";
        let d = split_markdown_conventions(raw);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0].id, "firebase-auth");
        assert_eq!(d[0].sections, vec!["Sign In".to_string()]);
        assert_eq!(d[1].id, "firestore");
        assert_eq!(d[1].sections, vec!["Query".to_string()]);
    }

    #[test]
    fn split_no_h1_single_draft_title_from_front_matter() {
        let raw = "---\ntitle: Loose Notes\n---\n\nsome prose\n\n## A\nx\n";
        let d = split_markdown_conventions(raw);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].title, "Loose Notes");
        assert_eq!(d[0].id, "loose-notes");
        assert_eq!(d[0].sections, vec!["A".to_string()]);
    }

    #[test]
    fn split_ignores_h1_inside_code_fence() {
        let raw = "# Real\n```\n# fake heading\n```\n## S\nx\n";
        let d = split_markdown_conventions(raw);
        assert_eq!(d.len(), 1, "fenced # must not start a new piece");
        assert_eq!(d[0].title, "Real");
    }

    #[test]
    fn valid_doc_id_rules() {
        assert!(valid_doc_id("firebase-integration"));
        assert!(!valid_doc_id(""));
        assert!(!valid_doc_id("Bad Id"));
    }

    #[test]
    fn import_convention_injects_anchors_on_h2() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("conventions");
        let raw = "---\ntitle: T\n---\n\n# T\n\n## A Section\nbody\n\n## Pre Anchored {#custom}\nx\n\n```\n## not a heading\n```\n";
        add_convention_from_markdown(&dir, raw).unwrap();
        let md = convention_md_in(&dir, "t").unwrap();
        assert!(
            md.contains("## A Section {#a-section}"),
            "h2 without anchor gets one: {md}"
        );
        assert!(
            md.contains("## Pre Anchored {#custom}"),
            "existing anchor preserved"
        );
        assert!(md.contains("## not a heading\n"), "fenced ## kept");
        assert!(
            !md.contains("## not a heading {#"),
            "fenced ## must NOT be anchored"
        );
    }

    #[test]
    fn import_convention_title_falls_back_to_h1_and_errors_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("conventions");
        let (id, _) = add_convention_from_markdown(&dir, "# My Rule\n## A\nx\n").unwrap();
        assert_eq!(id, "my-rule");
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
        let md = std::fs::read_to_string(dir.join("scaffold-x.md")).unwrap();
        assert!(md.contains("step 1"));
        let idx = std::fs::read_to_string(dir.join("_index.json")).unwrap();
        assert!(idx.contains("scaffold-x"));
        assert!(
            add_recipe_from_markdown(&dir, raw).unwrap().1,
            "re-import → replaced"
        );
    }
}
