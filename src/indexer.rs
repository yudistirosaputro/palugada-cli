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
struct Extractors {
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
    regex: String,
}

#[derive(Serialize, Deserialize)]
struct Symbol {
    name: String,
    kind: String,
    file: String,
    line: usize,
}

#[derive(Serialize)]
struct Manifest {
    indexed_at: String,
    git_sha: String,
    total: usize,
    counts: BTreeMap<String, usize>,
}

struct CompiledFamily {
    id: String,
    ext: Vec<String>,
    path_contains: String,
    re: Regex,
}

pub fn run(repo: &Path, kn: &Path, profile: &str) -> Result<(), String> {
    let ext_path = kn.join("profiles").join(profile).join("extractors.yaml");
    let raw = fs::read_to_string(&ext_path).map_err(|e| {
        format!("no extractors.yaml for profile '{profile}' ({}): {e}", ext_path.display())
    })?;
    let cfg: Extractors =
        serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", ext_path.display()))?;

    if cfg.families.is_empty() {
        return Err(format!("profile '{profile}' declares no extraction families"));
    }

    let mut families: Vec<CompiledFamily> = Vec::new();
    for f in &cfg.families {
        let re = Regex::new(&f.regex)
            .map_err(|e| format!("family '{}': invalid regex: {e}", f.id))?;
        families.push(CompiledFamily {
            id: f.id.clone(),
            ext: f.ext.clone(),
            path_contains: f.path_contains.clone(),
            re,
        });
    }

    let ignore = cfg.ignore_dirs.clone();
    let mut symbols: Vec<Symbol> = Vec::new();

    for entry in WalkDir::new(repo)
        .into_iter()
        .filter_entry(|e| !is_ignored(e, &ignore))
    {
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
        let path_str = path.to_string_lossy();

        let applicable: Vec<&CompiledFamily> = families
            .iter()
            .filter(|f| {
                (f.ext.is_empty() || f.ext.iter().any(|x| x == &ext))
                    && (f.path_contains.is_empty() || path_str.contains(f.path_contains.as_str()))
            })
            .collect();
        if applicable.is_empty() {
            continue;
        }

        let text = match fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => continue, // skip binary / unreadable files
        };
        let rel = path
            .strip_prefix(repo)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        for fam in applicable {
            for caps in fam.re.captures_iter(&text) {
                if let Some(m) = caps.name("name") {
                    let line = text[..m.start()].bytes().filter(|&b| b == b'\n').count() + 1;
                    symbols.push(Symbol {
                        name: m.as_str().to_string(),
                        kind: fam.id.clone(),
                        file: rel.clone(),
                        line,
                    });
                }
            }
        }
    }

    // Write index artifacts.
    let out = repo.join(".palugada").join("index");
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;

    write_json(&out.join("symbols.json"), &symbols)?;

    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for s in &symbols {
        *counts.entry(s.kind.clone()).or_insert(0) += 1;
    }
    for kind in counts.keys() {
        let fam: Vec<&Symbol> = symbols.iter().filter(|s| &s.kind == kind).collect();
        write_json(&out.join(format!("{kind}.json")), &fam)?;
    }

    let manifest = Manifest {
        indexed_at: chrono::Utc::now().to_rfc3339(),
        git_sha: git_sha(repo),
        total: symbols.len(),
        counts: counts.clone(),
    };
    write_json(&out.join("manifest.json"), &manifest)?;

    println!("Indexed {} -> {}", repo.display(), out.display());
    for (k, c) in &counts {
        println!("  {:<12} {}", k, c);
    }
    println!("  {:<12} {}", "TOTAL", symbols.len());
    Ok(())
}

/// Search the project's indexed symbols by name (case-insensitive substring).
pub fn symbol_search(repo: &Path, query: &str) -> Result<(), String> {
    println!("{}", symbol_report(repo, query)?.trim_end());
    Ok(())
}

/// Like `symbol_search` but returns the formatted result as a string (for
/// `brief`). A missing index degrades to a note rather than an error.
pub fn symbol_report(repo: &Path, query: &str) -> Result<String, String> {
    let p = repo.join(".palugada").join("index").join("symbols.json");
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return Ok(format!("(no index at {} — run `palugada index`)", p.display())),
    };
    let symbols: Vec<Symbol> =
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;

    let needle = query.to_lowercase();
    let mut out = String::new();
    let mut hits = 0;
    for s in &symbols {
        if query.is_empty() || s.name.to_lowercase().contains(&needle) {
            out.push_str(&format!("{:<12} {:<32} {}:{}\n", s.kind, s.name, s.file, s.line));
            hits += 1;
            if hits >= 30 {
                out.push_str("… (more matches; narrow the query)\n");
                break;
            }
        }
    }
    if hits == 0 {
        out.push_str(&format!("(no symbol matches '{query}'; {} indexed)", symbols.len()));
    }
    Ok(out)
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
