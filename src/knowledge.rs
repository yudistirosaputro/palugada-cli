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
    Err("can't locate the knowledge/ directory — set `engine.knowledge_path` in \
~/.palugada.yaml or the PALUGADA_KNOWLEDGE env var (running `palugada config init` \
from inside the palugada repo auto-detects it)"
        .to_string())
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
    sections: Vec<ConvSection>,
}

#[derive(Deserialize, Default)]
struct ConvSection {
    #[serde(default)]
    title: String,
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
}

fn read_conv_index(kn: &Path, profile: &str) -> Result<ConvIndex, String> {
    let p = kn.join("profiles").join(profile).join("conventions").join("_index.json");
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
}

fn read_recipe_index(kn: &Path, profile: &str) -> Result<RecipeIndex, String> {
    let p = kn.join("profiles").join(profile).join("recipes").join("_index.json");
    let data = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))
}

// ── q: conventions ──────────────────────────────────────────────────────

pub fn list_topics(kn: &Path, profile: &str) -> Result<(), String> {
    let idx = read_conv_index(kn, profile)?;
    if idx.topics.is_empty() {
        println!("(no conventions in profile '{profile}')");
        return Ok(());
    }
    println!("Conventions in profile '{profile}':");
    for t in &idx.topics {
        println!("  {:<16} {}", t.id, t.description);
    }
    Ok(())
}

pub fn query(kn: &Path, profile: &str, topic_arg: &str, brief: bool) -> Result<(), String> {
    let (name, section) = parse_topic_arg(topic_arg);
    let path = kn
        .join("profiles")
        .join(profile)
        .join("conventions")
        .join(format!("{name}.md"));
    let raw = fs::read_to_string(&path).map_err(|e| {
        format!("no convention '{name}' in profile '{profile}' ({}): {e}", path.display())
    })?;
    let body = strip_frontmatter(&raw);

    if brief {
        println!("{}", convention_outline_str(&raw, name));
        return Ok(());
    }

    if let Some(n) = section {
        let secs = sections(body);
        let s = secs
            .get(n.saturating_sub(1))
            .ok_or_else(|| format!("section {n} not found in '{name}' (it has {})", secs.len()))?;
        println!("## {}\n\n{}", s.title, s.body.trim());
        return Ok(());
    }

    println!("{}", body.trim());
    Ok(())
}

// ── for: recipes ──────────────────────────────────────────────────────────

pub fn list_recipes(kn: &Path, profile: &str) -> Result<(), String> {
    let idx = read_recipe_index(kn, profile)?;
    if idx.recipes.is_empty() {
        println!("(no recipes in profile '{profile}')");
        return Ok(());
    }
    println!("Recipes in profile '{profile}':");
    for r in &idx.recipes {
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

pub fn convention_outline(kn: &Path, profile: &str, name: &str) -> Result<String, String> {
    let path = kn
        .join("profiles")
        .join(profile)
        .join("conventions")
        .join(format!("{name}.md"));
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("no convention '{name}' in profile '{profile}': {e}"))?;
    Ok(convention_outline_str(&raw, name))
}

pub fn recipe_body(kn: &Path, profile: &str, task: &str) -> Result<String, String> {
    let path = kn
        .join("profiles")
        .join(profile)
        .join("recipes")
        .join(format!("{task}.md"));
    let raw = fs::read_to_string(&path)
        .map_err(|e| format!("no recipe '{task}' in profile '{profile}': {e}"))?;
    Ok(strip_frontmatter(&raw).trim().to_string())
}

// ── s: keyword search across the index ─────────────────────────────────────

pub fn search(kn: &Path, profile: &str, kw: &str) -> Result<(), String> {
    let needle = kw.to_lowercase();
    let mut hits = 0;

    if let Ok(idx) = read_conv_index(kn, profile) {
        for t in &idx.topics {
            let hay = format!(
                "{} {} {} {} {}",
                t.id,
                t.title,
                t.description,
                t.tags.join(" "),
                t.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>().join(" ")
            )
            .to_lowercase();
            if hay.contains(&needle) {
                println!("[convention] {:<16} {}", t.id, t.description);
                hits += 1;
            }
        }
    }
    if let Ok(idx) = read_recipe_index(kn, profile) {
        for r in &idx.recipes {
            let hay = format!("{} {} {} {}", r.id, r.title, r.description, r.tags.join(" ")).to_lowercase();
            if hay.contains(&needle) {
                println!("[recipe]     {:<16} {}", r.id, r.description);
                hits += 1;
            }
        }
    }
    if hits == 0 {
        println!("No matches for '{kw}' in profile '{profile}'.");
    }
    Ok(())
}

/// Convention topics whose tags intersect `keys` (lowercased file extensions
/// or family ids). Used by `brief`'s diff.scan to map changed files to rules.
pub fn topics_matching_tags(
    kn: &Path,
    profile: &str,
    keys: &std::collections::BTreeSet<String>,
) -> Vec<(String, String)> {
    let Ok(idx) = read_conv_index(kn, profile) else {
        return Vec::new();
    };
    let keys_lower: std::collections::BTreeSet<String> =
        keys.iter().map(|k| k.to_lowercase()).collect();
    idx.topics
        .iter()
        .filter(|t| t.tags.iter().any(|tag| keys_lower.contains(&tag.to_lowercase())))
        .map(|t| (t.id.clone(), t.description.clone()))
        .collect()
}

// ── markdown helpers ────────────────────────────────────────────────────

struct Section {
    title: String,
    body: String,
}

/// Split "architecture.2" → ("architecture", Some(2)); "architecture" → (_, None).
fn parse_topic_arg(arg: &str) -> (&str, Option<usize>) {
    if let Some((name, rest)) = arg.rsplit_once('.') {
        if let Ok(n) = rest.parse::<usize>() {
            return (name, Some(n));
        }
    }
    (arg, None)
}

/// Return the markdown body with the leading YAML front-matter removed.
fn strip_frontmatter(raw: &str) -> &str {
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
fn frontmatter_field(raw: &str, key: &str) -> Option<String> {
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
                cur = Some(Section { title, body: String::new() });
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
    fn sections_ignores_headers_inside_code_fences() {
        let body = "## One\ntext\n```sh\n## not a header\n```\nmore\n## Two\nend\n";
        let secs = sections(body);
        assert_eq!(secs.len(), 2, "{:?}", secs.iter().map(|s| &s.title).collect::<Vec<_>>());
        assert_eq!(secs[0].title, "One");
        assert!(secs[0].body.contains("## not a header"));
        assert_eq!(secs[1].title, "Two");
    }

    #[test]
    fn topics_matching_tags_filters_by_intersection() {
        let kn = tempfile::tempdir().unwrap();
        let conv = kn.path().join("profiles").join("p").join("conventions");
        std::fs::create_dir_all(&conv).unwrap();
        std::fs::write(
            conv.join("_index.json"),
            r#"{"topics":[
                {"id":"style","description":"kotlin style","tags":["kt","style"]},
                {"id":"css","description":"css rules","tags":["css"]},
                {"id":"mixed","description":"mixed case topic","tags":["KT","Style"]}
            ]}"#,
        )
        .unwrap();
        let mut keys = std::collections::BTreeSet::new();
        keys.insert("kt".to_string());
        let hits = topics_matching_tags(kn.path(), "p", &keys);
        // Both "style" (tags: ["kt","style"]) and "mixed" (tags: ["KT","Style"]) must match.
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|(id, _)| id == "style"));
        assert!(hits.iter().any(|(id, _)| id == "mixed"),
            "mixed-case tag 'KT' should match lowercase key 'kt'");
    }
}
