//! Project indexer (`palugada index`).
//!
//! Walks a project's files and applies the bound profile's declarative
//! extraction rules (`extractors.yaml`) to produce per-project facts under
//! `<repo>/.palugada/index/`. Local-only and re-runnable — there is no shared
//! corpus and no `sync`; each developer indexes their own checkout.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

#[derive(Deserialize, Default)]
pub struct Extractors {
    #[serde(default)]
    ignore_dirs: Vec<String>,
    #[serde(default)]
    families: Vec<Family>,
}

#[derive(Deserialize, Default)]
struct Family {
    id: String,
    #[serde(default)]
    ext: Vec<String>,
    #[serde(default)]
    path_contains: String,
    #[serde(default)]
    regex: String,
    #[serde(default)]
    language: String,
    /// Path to a `.scm` tree-sitter query, relative to the profile dir.
    #[serde(default)]
    query: String,
}

#[derive(Serialize, Deserialize)]
struct Symbol {
    name: String,
    kind: String,
    file: String,
    line: usize,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    indexed_at: String,
    git_sha: String,
    total: usize,
    symbols: usize,
    #[serde(default)]
    counts: BTreeMap<String, usize>,
}

#[derive(Debug)]
enum Extractor {
    Regex(Regex),
    TreeSitter { language: String, query: tree_sitter::Query },
}

#[derive(Debug)]
pub struct CompiledFamily {
    pub id: String,
    pub ext: Vec<String>,
    pub path_contains: String,
    extractor: Extractor,
}

/// Compile every family into a regex or tree-sitter extractor and validate it.
/// `profile_dir` is where `.scm` query paths resolve from.
pub fn compile_families(cfg: &Extractors, profile_dir: &Path) -> Result<Vec<CompiledFamily>, String> {
    let mut families: Vec<CompiledFamily> = Vec::new();
    for f in &cfg.families {
        let id_ok = !f.id.is_empty()
            && f.id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !id_ok {
            return Err(format!(
                "family id '{}' is invalid — use only [a-z0-9_-] (ids become index file names)",
                f.id
            ));
        }
        // `symbols`/`manifest` are the generic-index file names; a family with
        // either id would overwrite them and serve wrong data (minor 4).
        if f.id == "symbols" || f.id == "manifest" {
            return Err(format!(
                "family id '{}' is reserved (collides with the generic index file {}.json)",
                f.id, f.id
            ));
        }
        let has_regex = !f.regex.is_empty();
        let has_query = !f.query.is_empty();
        let extractor = match (has_regex, has_query) {
            (true, true) => return Err(format!("family '{}': set either regex or query, not both", f.id)),
            (false, false) => return Err(format!("family '{}': must set a regex or a query", f.id)),
            (true, false) => {
                let re = Regex::new(&f.regex).map_err(|e| format!("family '{}': invalid regex: {e}", f.id))?;
                Extractor::Regex(re)
            }
            (false, true) => {
                if f.language.is_empty() {
                    return Err(format!("family '{}': a query needs a `language`", f.id));
                }
                let lang = language_for(&f.language)?;
                let scm = profile_dir.join(&f.query);
                let src = fs::read_to_string(&scm)
                    .map_err(|e| format!("family '{}': read {}: {e}", f.id, scm.display()))?;
                let query = tree_sitter::Query::new(&lang, &src)
                    .map_err(|e| format!("family '{}': invalid query {}: {e}", f.id, scm.display()))?;
                if query.capture_index_for_name("name").is_none() {
                    return Err(format!("family '{}': query {} has no @name capture", f.id, scm.display()));
                }
                Extractor::TreeSitter { language: f.language.clone(), query }
            }
        };
        families.push(CompiledFamily {
            id: f.id.clone(),
            ext: f.ext.clone(),
            path_contains: f.path_contains.clone(),
            extractor,
        });
    }
    Ok(families)
}

fn family_matches(f: &CompiledFamily, path_str: &str, ext: &str) -> bool {
    (f.ext.is_empty() || f.ext.iter().any(|x| x == ext))
        && (f.path_contains.is_empty() || path_str.contains(f.path_contains.as_str()))
}

/// Ids of every family whose ext/path_contains rules match `path_str`.
pub fn families_for_path(path_str: &str, ext: &str, families: &[CompiledFamily]) -> Vec<String> {
    families.iter().filter(|f| family_matches(f, path_str, ext)).map(|f| f.id.clone()).collect()
}

/// Map a profile-declared `language` string to its bundled tree-sitter grammar.
/// Adding a language later = add its crate + one arm here (no profile change).
fn language_for(name: &str) -> Result<tree_sitter::Language, String> {
    match name {
        "kotlin" => Ok(tree_sitter_kotlin_ng::LANGUAGE.into()),
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        "dart" => Ok(tree_sitter_dart::LANGUAGE.into()),
        other => Err(format!("unsupported language '{other}' (supported: kotlin, rust, dart)")),
    }
}

const KOTLIN_TAGS: &str = include_str!("tags/kotlin.scm");
const RUST_TAGS: &str = include_str!("tags/rust.scm");
const DART_TAGS: &str = include_str!("tags/dart.scm");

/// Map a file extension to a language that has a generic tags query.
pub fn language_for_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "kt" | "kts" => Some("kotlin"),
        "rs" => Some("rust"),
        "dart" => Some("dart"),
        _ => None,
    }
}

/// The embedded tree-sitter tags query for a language, if any.
pub fn tags_query(lang: &str) -> Option<&'static str> {
    match lang {
        "kotlin" => Some(KOTLIN_TAGS),
        "rust" => Some(RUST_TAGS),
        "dart" => Some(DART_TAGS),
        _ => None,
    }
}

/// Emit symbols from one file: regex families inline, tree-sitter families
/// against a tree parsed once per distinct language present in `applicable`.
fn extract_file(text: &str, rel: &str, applicable: &[&CompiledFamily], symbols: &mut Vec<Symbol>) {
    use std::collections::BTreeSet;
    use tree_sitter::StreamingIterator;

    for fam in applicable {
        if let Extractor::Regex(re) = &fam.extractor {
            for caps in re.captures_iter(text) {
                if let Some(m) = caps.name("name") {
                    let line = text[..m.start()].bytes().filter(|&b| b == b'\n').count() + 1;
                    symbols.push(Symbol {
                        name: m.as_str().to_string(),
                        kind: fam.id.clone(),
                        file: rel.to_string(),
                        line,
                    });
                }
            }
        }
    }

    let langs: BTreeSet<&str> = applicable
        .iter()
        .filter_map(|f| match &f.extractor {
            Extractor::TreeSitter { language, .. } => Some(language.as_str()),
            _ => None,
        })
        .collect();
    for langname in langs {
        let lang = match language_for(langname) {
            Ok(l) => l,
            Err(_) => continue,
        };
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&lang).is_err() {
            continue;
        }
        let tree = match parser.parse(text, None) {
            Some(t) => t,
            None => continue,
        };
        for fam in applicable {
            if let Extractor::TreeSitter { language, query } = &fam.extractor {
                if language != langname {
                    continue;
                }
                let name_idx = match query.capture_index_for_name("name") {
                    Some(i) => i,
                    None => continue,
                };
                let mut cur = tree_sitter::QueryCursor::new();
                let mut it = cur.matches(query, tree.root_node(), text.as_bytes());
                while let Some(m) = it.next() {
                    for c in m.captures {
                        if c.index == name_idx {
                            if let Ok(nm) = c.node.utf8_text(text.as_bytes()) {
                                symbols.push(Symbol {
                                    name: nm.to_string(),
                                    kind: fam.id.clone(),
                                    file: rel.to_string(),
                                    line: c.node.start_position().row + 1,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── generic symbol index (all definitions, via per-language tags query) ─────

#[derive(Serialize, Deserialize, Default, Clone)]
struct SymbolDef {
    name: String,
    kind: String,
    file: String,
    line: usize,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    signature: String,
}

const SIG_CAP: usize = 160;

/// Walk up to the nearest definition node.
fn nearest_decl(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    let mut n = Some(node);
    while let Some(cur) = n {
        match cur.kind() {
            "class_declaration" | "object_declaration" | "function_declaration" | "property_declaration" => {
                return Some(cur)
            }
            _ => n = cur.parent(),
        }
    }
    None
}

/// Name of the class/object enclosing `decl` (empty if top-level).
fn enclosing_type_name(decl: tree_sitter::Node, bytes: &[u8]) -> String {
    let mut n = decl.parent();
    while let Some(cur) = n {
        if matches!(cur.kind(), "class_declaration" | "object_declaration") {
            if let Some(nm) = cur.child_by_field_name("name") {
                return nm.utf8_text(bytes).unwrap_or("").to_string();
            }
        }
        n = cur.parent();
    }
    String::new()
}

/// Declaration header: source from the decl start to its body, whitespace-collapsed and capped.
fn signature_of(decl: tree_sitter::Node, text: &str) -> String {
    let mut end = decl.end_byte();
    let mut walk = decl.walk();
    for child in decl.children(&mut walk) {
        if matches!(child.kind(), "function_body" | "class_body" | "enum_class_body") {
            end = child.start_byte();
            break;
        }
    }
    let raw = text.get(decl.start_byte()..end).unwrap_or("");
    let collapsed = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > SIG_CAP {
        let s: String = collapsed.chars().take(SIG_CAP).collect();
        format!("{s}…")
    } else {
        collapsed
    }
}

/// Generic pass: extract all definitions from one file via its language tags query.
fn extract_symbols(text: &str, rel: &str, lang_name: &str, out: &mut Vec<SymbolDef>) {
    use tree_sitter::StreamingIterator;
    let q_src = match tags_query(lang_name) {
        Some(q) => q,
        None => return,
    };
    let lang = match language_for(lang_name) {
        Ok(l) => l,
        Err(_) => return,
    };
    let query = match tree_sitter::Query::new(&lang, q_src) {
        Ok(q) => q,
        Err(_) => return,
    };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return;
    }
    let tree = match parser.parse(text, None) {
        Some(t) => t,
        None => return,
    };
    let names = query.capture_names();
    let bytes = text.as_bytes();
    let mut cur = tree_sitter::QueryCursor::new();
    let mut it = cur.matches(&query, tree.root_node(), bytes);
    while let Some(m) = it.next() {
        for c in m.captures {
            let kind0 = names[c.index as usize];
            let name = match c.node.utf8_text(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => continue,
            };
            let decl = nearest_decl(c.node).unwrap_or(c.node);
            let scope = enclosing_type_name(decl, bytes);
            let kind = if kind0 == "function" && !scope.is_empty() { "method" } else { kind0 };
            out.push(SymbolDef {
                name,
                kind: kind.to_string(),
                file: rel.to_string(),
                line: c.node.start_position().row + 1,
                scope,
                signature: signature_of(decl, text),
            });
        }
    }
}

/// Read + compile a profile's extractors.yaml. Returns (ignore_dirs, families).
pub fn load_families(kn: &Path, profile: &str) -> Result<(Vec<String>, Vec<CompiledFamily>), String> {
    let profile_dir = kn.join("profiles").join(profile);
    let ext_path = profile_dir.join("extractors.yaml");
    let raw = fs::read_to_string(&ext_path)
        .map_err(|e| format!("no extractors.yaml for profile '{profile}' ({}): {e}", ext_path.display()))?;
    let cfg: Extractors = serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", ext_path.display()))?;
    let families = compile_families(&cfg, &profile_dir)?;
    Ok((cfg.ignore_dirs, families))
}

#[derive(Deserialize, Default)]
struct ProfileFacts {
    #[serde(default)]
    fact_families: Vec<FactFamily>,
}
#[derive(Deserialize)]
struct FactFamily {
    id: String,
}

/// The fact-family ids the profile declares (validates `fact <family>`).
pub fn fact_families(kn: &Path, profile: &str) -> Result<Vec<String>, String> {
    let p = kn.join("profiles").join(profile).join("profile.yaml");
    let raw = fs::read_to_string(&p).map_err(|e| format!("read {}: {e}", p.display()))?;
    let pf: ProfileFacts = serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", p.display()))?;
    Ok(pf.fact_families.into_iter().map(|f| f.id).collect())
}

/// Look up indexed facts of one family, optionally filtered by name substring.
pub fn fact_report(
    repo: &Path,
    kn: &Path,
    profile: &str,
    family: &str,
    name: Option<&str>,
) -> Result<String, String> {
    let known = fact_families(kn, profile)?;
    if !known.iter().any(|f| f == family) {
        return Err(format!(
            "unknown fact family '{family}' for profile '{profile}' (available: {})",
            if known.is_empty() { "none".to_string() } else { known.join(", ") }
        ));
    }
    let p = repo.join(".palugada").join("index").join(format!("{family}.json"));
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return Ok(format!("(no '{family}' facts indexed — run `palugada index`)")),
    };
    let symbols: Vec<Symbol> =
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;
    let needle = name.map(|n| n.to_lowercase());
    let mut out = String::new();
    let mut hits = 0;
    for s in &symbols {
        if let Some(n) = &needle {
            if !s.name.to_lowercase().contains(n.as_str()) {
                continue;
            }
        }
        out.push_str(&format!("{:<32} {}:{}\n", s.name, s.file, s.line));
        hits += 1;
        if hits >= 30 {
            out.push_str("… (more matches; narrow the query)\n");
            break;
        }
    }
    if hits == 0 {
        out.push_str(&format!(
            "(no '{family}' facts{})",
            name.map(|n| format!(" matching '{n}'")).unwrap_or_default()
        ));
    }
    Ok(out)
}

pub fn run(repo: &Path, kn: &Path, profile: &str) -> Result<(), String> {
    let (ignore, families) = load_families(kn, profile)?;
    if families.is_empty() {
        return Err(format!("profile '{profile}' declares no extraction families"));
    }

    let mut facts: Vec<Symbol> = Vec::new();
    let mut defs: Vec<SymbolDef> = Vec::new();

    // Never index our own output dir, regardless of the profile's ignore_dirs
    // (M3: a profile that omits `.palugada` would otherwise index the previous
    // run's symbols.json into a feedback loop).
    let self_dir = repo.join(".palugada");
    for entry in WalkDir::new(repo).into_iter().filter_entry(|e| {
        // Never filter the walk ROOT — a repo cloned into a dir named like an
        // ignore entry (e.g. `build`/`target`) must still be scanned (M2).
        e.depth() == 0 || (e.path() != self_dir && !is_ignored(e, &ignore))
    }) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        // Match families on the REPO-RELATIVE path, not the absolute walk path —
        // otherwise a `path_contains` rule can match a segment of the checkout's
        // parent dirs (M1), and it also matches `brief`'s git-relative classify.
        let rel = path
            .strip_prefix(repo)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let applicable: Vec<&CompiledFamily> =
            families.iter().filter(|f| family_matches(f, &rel, &ext)).collect();
        let lang = language_for_ext(&ext).filter(|l| tags_query(l).is_some());
        if applicable.is_empty() && lang.is_none() {
            continue;
        }

        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue, // skip binary / unreadable files
        };

        if !applicable.is_empty() {
            extract_file(&text, &rel, &applicable, &mut facts);
        }
        if let Some(l) = lang {
            extract_symbols(&text, &rel, l, &mut defs);
        }
    }

    // Write index artifacts — clear first so stale per-kind files are removed.
    // Non-atomic rebuild: a concurrent read during re-index sees "no index" briefly.
    // Acceptable for a local single-developer CLI; switch to write-to-temp + rename if that changes.
    let out = repo.join(".palugada").join("index");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("clear {}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;

    // generic symbol index (all definitions)
    write_json(&out.join("symbols.json"), &defs)?;

    // curated fact families → per-family files
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &facts {
        *counts.entry(s.kind.clone()).or_insert(0) += 1;
    }
    for kind in counts.keys() {
        let fam: Vec<&Symbol> = facts.iter().filter(|s| &s.kind == kind).collect();
        write_json(&out.join(format!("{kind}.json")), &fam)?;
    }

    let manifest = Manifest {
        indexed_at: chrono::Utc::now().to_rfc3339(),
        git_sha: git_sha(repo),
        total: facts.len(),
        symbols: defs.len(),
        counts: counts.clone(),
    };
    write_json(&out.join("manifest.json"), &manifest)?;

    println!("Indexed {} -> {}", repo.display(), out.display());
    for (k, c) in &counts {
        println!("  {:<12} {}", k, c);
    }
    println!("  {:<12} {}", "symbols", defs.len());
    println!("  {:<12} {}", "FACTS", facts.len());
    Ok(())
}

/// Search the generic symbol index by name (case-insensitive substring),
/// optionally filtered by kind.
pub fn symbol_search(repo: &Path, query: &str, kind: Option<&str>) -> Result<(), String> {
    println!("{}", symbol_report(repo, query, kind)?.trim_end());
    Ok(())
}

/// Like `symbol_search` but returns the formatted result as a string (for
/// `brief`). A missing index degrades to a note rather than an error.
pub fn symbol_report(repo: &Path, query: &str, kind: Option<&str>) -> Result<String, String> {
    let p = repo.join(".palugada").join("index").join("symbols.json");
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return Ok(format!("(no index at {} — run `palugada index`)", p.display())),
    };
    let symbols: Vec<SymbolDef> =
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;

    let needle = query.to_lowercase();
    let mut out = String::new();
    let mut hits = 0;
    for s in &symbols {
        if let Some(k) = kind {
            if s.kind != k {
                continue;
            }
        }
        if !query.is_empty() && !s.name.to_lowercase().contains(&needle) {
            continue;
        }
        let sig = if s.signature.is_empty() { s.name.clone() } else { s.signature.clone() };
        let scope = if s.scope.is_empty() { String::new() } else { format!("{}  ·  ", s.scope) };
        out.push_str(&format!("{:<9} {}  ·  {}{}:{}\n", s.kind, sig, scope, s.file, s.line));
        hits += 1;
        if hits >= 40 {
            out.push_str("… (more matches; narrow the query or use --kind)\n");
            break;
        }
    }
    if hits == 0 {
        out.push_str(&format!("(no symbol matches '{query}'; {} indexed)", symbols.len()));
    }
    Ok(out)
}

/// Symbols DEFINED IN a specific repo-relative file. Used when `brief`'s target
/// is a path — name-matching a path string never hits, so `brief bugfix <file>`
/// used to always show a dead symbol step (P5). Empty note if none/absent index.
pub fn symbols_in_file(repo: &Path, file: &str) -> Result<String, String> {
    let p = repo.join(".palugada").join("index").join("symbols.json");
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return Ok(format!("(no index at {} — run `palugada index`)", p.display())),
    };
    let symbols: Vec<SymbolDef> =
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;
    let norm = |s: &str| s.replace('\\', "/");
    let target = norm(file);
    // Prefer exact path matches; only fall back to suffix matching (target may
    // carry an extra leading segment) when there is NO exact match — otherwise a
    // sibling like `sub/src/a.rs` would leak into `src/a.rs`'s listing.
    let has_exact = symbols.iter().any(|s| norm(&s.file) == target);
    let matches = symbols.iter().filter(|s| {
        let f = norm(&s.file);
        if has_exact {
            f == target
        } else {
            f.ends_with(&format!("/{target}")) || target.ends_with(&format!("/{f}"))
        }
    });
    let mut out = String::new();
    let mut hits = 0;
    for s in matches {
        let sig = if s.signature.is_empty() { s.name.clone() } else { s.signature.clone() };
        let scope = if s.scope.is_empty() { String::new() } else { format!("{}  ·  ", s.scope) };
        out.push_str(&format!("{:<9} {}  ·  {}{}\n", s.kind, sig, scope, s.line));
        hits += 1;
        if hits >= 40 {
            out.push_str("… (more; open the file)\n");
            break;
        }
    }
    if hits == 0 {
        out.push_str(&format!("(no indexed symbols in {file})"));
    }
    Ok(out)
}

/// Module prefix for a target: a file → its parent dir; anything else → itself.
fn module_prefix(target: &str) -> String {
    let p = Path::new(target);
    if p.extension().is_some() {
        p.parent().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()
    } else {
        target.trim_end_matches('/').to_string()
    }
}

/// Summarise indexed symbols whose file lives under the target's module prefix.
pub fn module_report(repo: &Path, target: &str) -> String {
    if target.is_empty() {
        return "(module.info needs a target path)".to_string();
    }
    let prefix = module_prefix(target);
    let p = repo.join(".palugada").join("index").join("symbols.json");
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return format!("(no index at {} — run `palugada index`)", p.display()),
    };
    let symbols: Vec<SymbolDef> = match serde_json::from_str(&data) {
        Ok(s) => s,
        Err(e) => return format!("(parse {}: {e})", p.display()),
    };
    let in_module: Vec<&SymbolDef> = symbols
        .iter()
        .filter(|s| s.file == prefix || s.file.starts_with(&format!("{prefix}/")))
        .collect();
    if in_module.is_empty() {
        return format!("(no indexed symbols under '{prefix}')");
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &in_module {
        *counts.entry(s.kind.clone()).or_insert(0) += 1;
    }
    let summary: Vec<String> = counts.iter().map(|(k, c)| format!("{k}: {c}")).collect();
    let mut out = format!("module {prefix} — {} symbols ({})\n", in_module.len(), summary.join(", "));
    for s in in_module.iter().take(30) {
        out.push_str(&format!("  {:<12} {:<28} {}:{}\n", s.kind, s.name, s.file, s.line));
    }
    out
}

fn is_ignored(e: &DirEntry, ignore: &[String]) -> bool {
    e.file_name()
        .to_str()
        .map(|n| ignore.iter().any(|ig| ig == n))
        .unwrap_or(false)
}

fn write_json<T: Serialize>(path: &Path, val: &T) -> Result<(), String> {
    let data = serde_json::to_string_pretty(val).map_err(|e| e.to_string())?;
    fs::write(path, data).map_err(|e| format!("write {}: {e}", path.display()))
}

/// A one-line staleness note for the repo's index, or `None` when it is fresh
/// (or absent). Primary signal is the git commit: if the checkout's HEAD differs
/// from the sha recorded at index time, the index is behind. When git is
/// unavailable, falls back to age vs `stale_days` (0 disables the time check).
/// This is why `palugada symbol/fact/brief` can warn that line numbers may be
/// stale instead of silently serving them (P3).
pub fn staleness_note(repo: &Path, stale_days: u32) -> Option<String> {
    let mpath = repo.join(".palugada").join("index").join("manifest.json");
    let raw = fs::read_to_string(&mpath).ok()?;
    let m: Manifest = serde_json::from_str(&raw).ok()?;

    let head = git_sha(repo);
    if !head.is_empty() && !m.git_sha.is_empty() {
        if head == m.git_sha {
            return None; // indexed at the current commit
        }
        let behind = commits_between(repo, &m.git_sha, &head);
        return Some(match behind {
            Some(n) if n > 0 => {
                format!("index is {n} commit(s) behind HEAD — run `palugada index` to refresh")
            }
            _ => "index was built at a different commit — run `palugada index` to refresh"
                .to_string(),
        });
    }

    // No git: fall back to age of the index.
    if stale_days > 0 {
        if let Some(days) = index_age_days(&m.indexed_at) {
            if days >= stale_days as i64 {
                return Some(format!(
                    "index is {days} day(s) old — run `palugada index` to refresh"
                ));
            }
        }
    }
    None
}

/// Number of commits in `from..to` (i.e. how far `to` is ahead of `from`).
fn commits_between(repo: &Path, from: &str, to: &str) -> Option<usize> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-list", "--count", &format!("{from}..{to}")])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse::<usize>().ok()
}

/// Whole days between an RFC3339 `indexed_at` and now (None if unparseable).
fn index_age_days(indexed_at: &str) -> Option<i64> {
    let then = chrono::DateTime::parse_from_rfc3339(indexed_at).ok()?;
    Some((chrono::Utc::now().signed_duration_since(then.with_timezone(&chrono::Utc))).num_days())
}

fn git_sha(repo: &Path) -> String {
    std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a throwaway knowledge dir with one profile + extractors.yaml.
    fn fixture(extractors_yaml: &str) -> (tempfile::TempDir, tempfile::TempDir) {
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(&prof).unwrap();
        fs::write(prof.join("extractors.yaml"), extractors_yaml).unwrap();
        let repo = tempfile::tempdir().unwrap();
        (kn, repo)
    }

    #[test]
    fn staleness_note_flags_old_index_and_absent_index(/* P3 */) {
        let repo = tempfile::tempdir().unwrap();
        // No index yet → no note (the "run index" hint is handled elsewhere).
        assert_eq!(staleness_note(repo.path(), 7), None);

        // Write a manifest with no git_sha and an old indexed_at → time fallback.
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(
            idx.join("manifest.json"),
            r#"{"indexed_at":"2000-01-01T00:00:00Z","git_sha":"","total":0,"symbols":0,"counts":{}}"#,
        )
        .unwrap();
        let note = staleness_note(repo.path(), 7).expect("old index should warn");
        assert!(note.contains("day(s) old"), "{note}");
        // stale_days = 0 disables the time-based check.
        assert_eq!(staleness_note(repo.path(), 0), None);
    }

    #[test]
    fn kotlin_grammar_loads_and_unknown_language_errors() {
        let lang = language_for("kotlin").unwrap();
        let q = tree_sitter::Query::new(&lang, r#"(class_declaration name: (identifier) @name)"#).unwrap();
        assert!(q.capture_index_for_name("name").is_some());
        assert!(language_for("klingon").unwrap_err().contains("klingon"));
    }

    #[test]
    fn extract_symbols_finds_defs_with_scope_and_kind() {
        let src = "class LoginViewModel : ViewModel() {\n  val title: String = \"x\"\n  fun login(u: String): Boolean { return true }\n}\nfun topLevel() {}\n// fun ghost() {}\nobject Cfg\n";
        let mut out = Vec::new();
        extract_symbols(src, "A.kt", "kotlin", &mut out);
        let by = |k: &str, n: &str| out.iter().find(|s| s.kind == k && s.name == n).cloned();
        assert!(by("class", "LoginViewModel").is_some());
        assert!(by("object", "Cfg").is_some());
        let login = by("method", "login").expect("login is a method");
        assert_eq!(login.scope, "LoginViewModel");
        assert!(login.signature.contains("fun login"), "sig was {:?}", login.signature);
        let tl = by("function", "topLevel").expect("topLevel is a function");
        assert_eq!(tl.scope, "");
        assert!(by("property", "title").is_some());
        assert!(out.iter().all(|s| s.name != "ghost"), "comment fun must not be captured");
    }

    #[test]
    fn tags_registry_resolves_rust() {
        assert_eq!(language_for_ext("rs"), Some("rust"));
        let q = tags_query("rust").unwrap();
        let lang = language_for("rust").unwrap();
        assert!(tree_sitter::Query::new(&lang, q).is_ok(), "rust.scm must compile");
    }

    #[test]
    fn extract_symbols_finds_rust_defs() {
        let src = "pub struct Config { pub a: u32 }\npub fn run(x: u32) -> u32 { x }\npub trait Host { fn ping(&self); }\n// fn ghost() {}\n";
        let mut out = Vec::new();
        extract_symbols(src, "lib.rs", "rust", &mut out);
        let by = |k: &str, n: &str| out.iter().find(|s| s.kind == k && s.name == n).cloned();
        assert!(by("struct", "Config").is_some());
        assert!(by("function", "run").is_some());
        assert!(by("trait", "Host").is_some());
        assert!(out.iter().all(|s| s.name != "ghost"), "comment fn must not be captured");
    }

    #[test]
    fn tags_registry_resolves_dart() {
        assert_eq!(language_for_ext("dart"), Some("dart"));
        let q = tags_query("dart").unwrap();
        let lang = language_for("dart").unwrap();
        assert!(tree_sitter::Query::new(&lang, q).is_ok(), "dart.scm must compile");
    }

    #[test]
    fn extract_symbols_finds_dart_defs() {
        let src = "class CounterCubit extends Cubit<int> {\n  CounterCubit() : super(0);\n}\nclass HomePage extends StatelessWidget {}\n";
        let mut out = Vec::new();
        extract_symbols(src, "home.dart", "dart", &mut out);
        assert!(out.iter().any(|s| s.name == "CounterCubit"));
        assert!(out.iter().any(|s| s.name == "HomePage"));
    }

    #[test]
    fn tags_registry_resolves_kotlin() {
        assert_eq!(language_for_ext("kt"), Some("kotlin"));
        assert_eq!(language_for_ext("kts"), Some("kotlin"));
        assert_eq!(language_for_ext("txt"), None);
        let q = tags_query("kotlin").unwrap();
        let lang = language_for("kotlin").unwrap();
        assert!(tree_sitter::Query::new(&lang, q).is_ok(), "kotlin.scm must compile");
    }

    #[test]
    fn families_for_path_matches_by_ext_and_path() {
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n  - id: i18n\n    ext: [xml]\n    path_contains: values\n    regex: '<string\\s+name=\"(?P<name>[^\"]+)\"'\n",
        ).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fams = compile_families(&cfg, dir.path()).unwrap();
        assert_eq!(families_for_path("app/Login.kt", "kt", &fams), vec!["viewmodel".to_string()]);
        assert_eq!(families_for_path("app/values/strings.xml", "xml", &fams), vec!["i18n".to_string()]);
        // xml outside a values/ dir does not match i18n
        assert!(families_for_path("app/other/x.xml", "xml", &fams).is_empty());
    }

    #[test]
    fn fact_report_rejects_unknown_family() {
        let (kn, repo) = fixture(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        );
        let prof = kn.path().join("profiles").join("p");
        fs::write(prof.join("profile.yaml"), "fact_families:\n  - { id: viewmodel, symbol: true }\n").unwrap();
        let err = fact_report(repo.path(), kn.path(), "p", "widget", None).unwrap_err();
        assert!(err.contains("widget"), "{err}");
        assert!(err.contains("viewmodel"), "should list available families: {err}");
    }

    #[test]
    fn fact_report_filters_by_kind_and_name() {
        let (kn, repo) = fixture(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)ViewModel'\n",
        );
        let prof = kn.path().join("profiles").join("p");
        fs::write(
            prof.join("profile.yaml"),
            "fact_families:\n  - { id: viewmodel, symbol: true }\n  - { id: service, symbol: true }\n",
        ).unwrap();
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        // fact lookups read the per-family file, not symbols.json
        fs::write(idx.join("viewmodel.json"),
            r#"[{"name":"LoginViewModel","kind":"viewmodel","file":"a.kt","line":1},
                {"name":"PaymentViewModel","kind":"viewmodel","file":"b.kt","line":2}]"#).unwrap();
        let all = fact_report(repo.path(), kn.path(), "p", "viewmodel", None).unwrap();
        assert!(all.contains("LoginViewModel") && all.contains("PaymentViewModel"));
        let one = fact_report(repo.path(), kn.path(), "p", "viewmodel", Some("login")).unwrap();
        assert!(one.contains("LoginViewModel") && !one.contains("PaymentViewModel"));
    }

    #[test]
    fn module_report_summarises_symbols_under_prefix() {
        let repo = tempfile::tempdir().unwrap();
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(idx.join("symbols.json"),
            r#"[{"name":"LoginViewModel","kind":"viewmodel","file":"feature/auth/Login.kt","line":1},
                {"name":"AuthService","kind":"service","file":"feature/auth/Auth.kt","line":2},
                {"name":"HomeViewModel","kind":"viewmodel","file":"feature/home/Home.kt","line":3}]"#).unwrap();
        // target is a file → its directory becomes the module prefix
        let out = module_report(repo.path(), "feature/auth/Login.kt");
        assert!(out.contains("LoginViewModel") && out.contains("AuthService"));
        assert!(!out.contains("HomeViewModel"), "home is outside feature/auth");
    }

    #[test]
    fn symbols_in_file_lists_only_that_files_defs(/* P5 */) {
        let repo = tempfile::tempdir().unwrap();
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(
            idx.join("symbols.json"),
            r#"[{"name":"Foo","kind":"struct","file":"src/a.rs","line":3},
                {"name":"Bar","kind":"struct","file":"src/b.rs","line":9}]"#,
        )
        .unwrap();
        let out = symbols_in_file(repo.path(), "src/a.rs").unwrap();
        assert!(out.contains("Foo"), "{out}");
        assert!(!out.contains("Bar"), "other files must not leak: {out}");
        // a file with no indexed symbols → a degraded note
        assert!(symbols_in_file(repo.path(), "src/none.rs").unwrap().contains("no indexed symbols"));
    }

    #[test]
    fn symbols_in_file_prefers_exact_over_sibling_suffix(/* review MEDIUM-2 */) {
        // A monorepo with both `src/a.rs` and `sub/src/a.rs`: querying `src/a.rs`
        // must return ONLY its symbols, not the nested sibling's.
        let repo = tempfile::tempdir().unwrap();
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(
            idx.join("symbols.json"),
            r#"[{"name":"Foo","kind":"struct","file":"src/a.rs","line":1},
                {"name":"Bar","kind":"struct","file":"sub/src/a.rs","line":2}]"#,
        )
        .unwrap();
        let out = symbols_in_file(repo.path(), "src/a.rs").unwrap();
        assert!(out.contains("Foo"), "{out}");
        assert!(!out.contains("Bar"), "nested sibling `sub/src/a.rs` leaked: {out}");
    }

    #[test]
    fn module_report_needs_a_target() {
        let repo = tempfile::tempdir().unwrap();
        assert!(module_report(repo.path(), "").contains("needs a target"));
    }

    #[test]
    fn rejects_family_with_both_regex_and_query() {
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: x\n    ext: [kt]\n    regex: 'a'\n    query: q.scm\n    language: kotlin\n",
        ).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let err = compile_families(&cfg, dir.path()).unwrap_err();
        assert!(err.contains("either regex or query"), "{err}");
    }

    #[test]
    fn rejects_query_without_language() {
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: x\n    ext: [kt]\n    query: q.scm\n",
        ).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let err = compile_families(&cfg, dir.path()).unwrap_err();
        assert!(err.contains("language"), "{err}");
    }

    #[test]
    fn rejects_query_without_name_capture() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("q.scm"), "(class_declaration) @other\n").unwrap();
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: x\n    ext: [kt]\n    language: kotlin\n    query: q.scm\n",
        ).unwrap();
        let err = compile_families(&cfg, dir.path()).unwrap_err();
        assert!(err.contains("@name"), "{err}");
    }

    #[test]
    fn tree_sitter_extracts_and_skips_comments() {
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(prof.join("extractors")).unwrap();
        fs::write(prof.join("extractors.yaml"),
            "families:\n  - id: viewmodel\n    ext: [kt]\n    language: kotlin\n    query: extractors/vm.scm\n").unwrap();
        fs::write(prof.join("extractors").join("vm.scm"),
            "(class_declaration name: (identifier) @name (#match? @name \"ViewModel$\"))\n").unwrap();
        let repo = tempfile::tempdir().unwrap();
        fs::write(repo.path().join("A.kt"),
            "class LoginViewModel : ViewModel()\n// class GhostViewModel removed\n").unwrap();
        run(repo.path(), kn.path(), "p").unwrap();
        let data = fs::read_to_string(repo.path().join(".palugada").join("index").join("symbols.json")).unwrap();
        assert!(data.contains("LoginViewModel"), "{data}");
        assert!(!data.contains("GhostViewModel"), "comment must not be extracted: {data}");
    }

    #[test]
    fn rejects_path_traversal_family_id() {
        let (kn, repo) = fixture(
            "families:\n  - id: \"../evil\"\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        );
        let err = run(repo.path(), kn.path(), "p").unwrap_err();
        assert!(err.contains("../evil"), "{err}");
    }

    #[test]
    fn rejects_reserved_family_id_symbols() {
        // A family named `symbols` would overwrite the generic index (minor 4).
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: symbols\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let err = compile_families(&cfg, dir.path()).unwrap_err();
        assert!(err.contains("reserved"), "{err}");
    }

    #[test]
    fn family_path_contains_matches_repo_relative_not_absolute(/* M1 */) {
        // Repo rooted at a dir whose name contains "myvalues"; a file at the repo
        // ROOT must NOT match `path_contains: myvalues` (only the absolute path
        // does). The family should produce no fact for it.
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(&prof).unwrap();
        fs::write(
            prof.join("extractors.yaml"),
            "families:\n  - id: special\n    path_contains: myvalues\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        )
        .unwrap();
        let base = tempfile::tempdir().unwrap();
        let repo = base.path().join("myvalues");
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("A.kt"), "class Foo {}\n").unwrap();

        run(&repo, kn.path(), "p").unwrap();
        let special = repo.join(".palugada/index/special.json");
        assert!(
            !special.exists(),
            "root file matched path_contains via the absolute path (M1 regression)"
        );
    }

    #[test]
    fn walk_root_named_like_ignore_dir_is_still_scanned(/* M2 */) {
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(&prof).unwrap();
        fs::write(
            prof.join("extractors.yaml"),
            "ignore_dirs: [build]\nfamilies:\n  - id: t\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        )
        .unwrap();
        let base = tempfile::tempdir().unwrap();
        let repo = base.path().join("build"); // repo cloned into a `build/` dir
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("A.kt"), "class Foo {}\n").unwrap();

        run(&repo, kn.path(), "p").unwrap();
        let syms = fs::read_to_string(repo.join(".palugada/index/symbols.json")).unwrap();
        assert!(syms.contains("Foo"), "walk root `build` was wrongly ignored (M2): {syms}");
    }

    #[test]
    fn does_not_index_its_own_output_dir(/* M3 */) {
        // A profile that omits `.palugada` from ignore_dirs must still not eat its
        // own previous output on the next run.
        let kn = tempfile::tempdir().unwrap();
        let prof = kn.path().join("profiles").join("p");
        fs::create_dir_all(&prof).unwrap();
        fs::write(
            prof.join("extractors.yaml"),
            // no ignore_dirs; `leak` (no ext) would scan symbols.json's JSON text.
            "families:\n  - id: leak\n    regex: '\"name\":\"(?P<name>\\w+)\"'\n",
        )
        .unwrap();
        let repo = tempfile::tempdir().unwrap();
        fs::write(repo.path().join("A.kt"), "class Foo {}\n").unwrap();

        run(repo.path(), kn.path(), "p").unwrap(); // run 1 writes symbols.json (has "name":"Foo")
        run(repo.path(), kn.path(), "p").unwrap(); // run 2 must not scan .palugada/index
        let leak = repo.path().join(".palugada/index/leak.json");
        if leak.exists() {
            let body = fs::read_to_string(&leak).unwrap();
            assert!(
                !body.contains(".palugada"),
                "indexed its own output dir (M3 self-index loop): {body}"
            );
        }
    }

    #[test]
    fn reindex_clears_stale_family_files() {
        let (kn, repo) = fixture(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)ViewModel'\n",
        );
        fs::write(repo.path().join("A.kt"), "class LoginViewModel {}").unwrap();
        run(repo.path(), kn.path(), "p").unwrap();
        let idx = repo.path().join(".palugada").join("index");
        assert!(idx.join("viewmodel.json").exists());
        // family disappears from the code → its file must disappear from the index
        fs::write(repo.path().join("A.kt"), "class Login {}").unwrap();
        run(repo.path(), kn.path(), "p").unwrap();
        assert!(!idx.join("viewmodel.json").exists(), "stale viewmodel.json survived re-index");
        assert!(idx.join("symbols.json").exists());
    }
}
