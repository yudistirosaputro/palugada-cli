//! Personal corpus — a local, per-user markdown store at `~/.palugada/personal/`
//! for fetched tickets (PRD layer D). The IO helpers take an explicit `dir` so
//! they're testable against a temp directory; `dir()` is the real default.

use crate::clients::Issue;
use std::fs;
use std::path::{Path, PathBuf};

/// The default corpus directory: `~/.palugada/personal/`.
pub fn dir() -> PathBuf {
    crate::config::home_dir().join(".palugada").join("personal")
}

/// Safe file stem from a doc key: keep `[A-Za-z0-9._-]`, map everything else
/// (separators, `#`, spaces) to `_`. Prevents path traversal and separators.
pub fn sanitize_name(key: &str) -> String {
    let s: String = key
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '_' })
        .collect();
    // Trim leading/trailing `_`/`.` so no traversal (`..`) or hidden-file names.
    let trimmed = s.trim_matches(|c| c == '_' || c == '.');
    if trimmed.is_empty() {
        "doc".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Markdown document for a fetched issue: YAML front-matter + description body.
pub fn format_issue_doc(i: &Issue, fetched_at: &str) -> String {
    format!(
        "---\nkey: {}\nsummary: {}\nstatus: {}\ntype: {}\nassignee: {}\nsource: issue_tracker\nfetched_at: {}\n---\n\n# {} — {}\n\n{}\n",
        i.key, i.summary, i.status, i.issue_type, i.assignee, fetched_at, i.key, i.summary, i.description
    )
}

/// Save a fetched issue into `dir`, returning the written path.
pub fn save_issue(dir: &Path, i: &Issue, fetched_at: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{}.md", sanitize_name(&i.key)));
    fs::write(&path, format_issue_doc(i, fetched_at)).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Doc names (file stems) in the corpus, sorted.
pub fn list(dir: &Path) -> Result<Vec<String>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(stem) = path.file_stem() {
                names.push(stem.to_string_lossy().to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

/// Read one saved doc by name.
pub fn cat(dir: &Path, name: &str) -> Result<String, String> {
    let path = dir.join(format!("{}.md", sanitize_name(name)));
    fs::read_to_string(&path).map_err(|e| format!("no corpus doc '{name}' ({}): {e}", path.display()))
}

/// Case-insensitive substring search across saved docs. Returns
/// (doc_name, first_matching_line) per hit.
pub fn search(dir: &Path, kw: &str) -> Result<Vec<(String, String)>, String> {
    let needle = kw.to_lowercase();
    let mut hits: Vec<(String, String)> = Vec::new();
    for name in list(dir)? {
        let body = cat(dir, &name)?;
        if let Some(line) = body
            .lines()
            .find(|l| l.to_lowercase().contains(&needle) && !l.starts_with("---"))
        {
            hits.push((name, line.trim_start_matches('#').trim().to_string()));
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue() -> Issue {
        Issue {
            key: "PROJ-1".into(),
            summary: "Add export".into(),
            status: "Open".into(),
            issue_type: "Story".into(),
            assignee: "me".into(),
            description: "Export to CSV from the watchlist screen.".into(),
        }
    }

    #[test]
    fn sanitize_name_strips_separators_and_unsafe_chars() {
        assert_eq!(sanitize_name("PROJ-123"), "PROJ-123");
        assert_eq!(sanitize_name("owner/name#42"), "owner_name_42");
        assert_eq!(sanitize_name("../etc/passwd"), "etc_passwd");
    }

    #[test]
    fn format_issue_doc_has_frontmatter_and_body() {
        let d = format_issue_doc(&issue(), "2026-06-14T00:00:00Z");
        assert!(d.starts_with("---\nkey: PROJ-1\n"));
        assert!(d.contains("fetched_at: 2026-06-14T00:00:00Z"));
        assert!(d.contains("Export to CSV"));
    }

    #[test]
    fn save_list_cat_search_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        save_issue(dir, &issue(), "2026-06-14T00:00:00Z").unwrap();
        assert_eq!(list(dir).unwrap(), vec!["PROJ-1".to_string()]);
        assert!(cat(dir, "PROJ-1").unwrap().contains("Add export"));
        let hits = search(dir, "csv").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "PROJ-1");
        assert!(search(dir, "nonexistent-term").unwrap().is_empty());
    }
}
