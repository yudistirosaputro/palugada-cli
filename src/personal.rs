//! Fetched-doc store — a local markdown cache of tickets (issue tracker) and
//! pages (wiki/DocSource). The IO helpers take an explicit `dir` so they're
//! testable and reused for the per-project cache (`<repo>/.palugada/docs/`).

use crate::clients::{Issue, WikiPage};
use std::fs;
use std::path::{Path, PathBuf};

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

/// Markdown document for a fetched wiki/doc page: YAML front-matter + body.
/// The title is collapsed to a single line so it can't break (or inject a
/// premature terminator into) the single-line `title:` front-matter scalar.
pub fn format_wiki_doc(page: &WikiPage, fetched_at: &str) -> String {
    let title = page.title.replace(['\n', '\r'], " ");
    format!(
        "---\nid: {}\ntitle: {}\nsource: wiki\nfetched_at: {}\n---\n\n# {}\n\n{}\n",
        page.id, title, fetched_at, title, page.body_html
    )
}

/// Save a fetched wiki page into `dir`, returning the written path. The file
/// stem is the sanitized title (friendly for `prd cat`), falling back to the id.
pub fn save_wiki(dir: &Path, page: &WikiPage, fetched_at: &str) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let stem = if page.title.trim().is_empty() {
        sanitize_name(&page.id)
    } else {
        sanitize_name(&page.title)
    };
    let path = dir.join(format!("{stem}.md"));
    fs::write(&path, format_wiki_doc(page, fetched_at)).map_err(|e| format!("write {}: {e}", path.display()))?;
    Ok(path)
}

/// Make a per-project docs cache self-ignoring: create it and drop a
/// `.gitignore` of `*` so fetched (possibly sensitive) docs are never committed.
/// `list` only sees `*.md`, so the `.gitignore` stays invisible to the corpus.
pub fn ensure_dir_ignored(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let gi = dir.join(".gitignore");
    if !gi.exists() {
        fs::write(&gi, "*\n").map_err(|e| format!("write {}: {e}", gi.display()))?;
    }
    Ok(())
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

/// Front-matter summary `(title, source, fetched_at)` for the web docs list.
/// `title` falls back to an issue's `summary:`; missing fields come back empty.
pub fn doc_summary(dir: &Path, name: &str) -> (String, String, String) {
    let body = cat(dir, name).unwrap_or_default();
    // Only scan the YAML front-matter block (between the first two `---` lines),
    // so a body line like `source: internal` can't be mistaken for metadata.
    let front: Vec<&str> = {
        let mut lines = body.lines();
        if lines.next() == Some("---") {
            lines.take_while(|l| *l != "---").collect()
        } else {
            Vec::new()
        }
    };
    let get = |k: &str| -> String {
        front.iter().find_map(|l| l.strip_prefix(k).map(|v| v.trim().to_string())).unwrap_or_default()
    };
    let title = {
        let t = get("title:");
        if t.is_empty() { get("summary:") } else { t }
    };
    (title, get("source:"), get("fetched_at:"))
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

    fn page() -> WikiPage {
        WikiPage {
            id: "38d68b31".into(),
            title: "PRD Onboarding".into(),
            body_html: "## TL;DR\nThe freelance dream.".into(),
        }
    }

    #[test]
    fn format_wiki_doc_has_frontmatter_and_body() {
        let d = format_wiki_doc(&page(), "2026-06-29T00:00:00Z");
        assert!(d.starts_with("---\nid: 38d68b31\n"));
        assert!(d.contains("source: wiki"));
        assert!(d.contains("title: PRD Onboarding"));
        assert!(d.contains("The freelance dream."));
    }

    #[test]
    fn save_wiki_uses_title_stem_and_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        save_wiki(dir, &page(), "2026-06-29T00:00:00Z").unwrap();
        assert_eq!(list(dir).unwrap(), vec!["PRD_Onboarding".to_string()]);
        assert!(cat(dir, "PRD Onboarding").unwrap().contains("freelance dream"));
        assert_eq!(search(dir, "freelance").unwrap().len(), 1);
    }

    #[test]
    fn save_wiki_falls_back_to_id_when_title_blank() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        let p = WikiPage { id: "abc123".into(), title: "  ".into(), body_html: "x".into() };
        save_wiki(dir, &p, "t").unwrap();
        assert_eq!(list(dir).unwrap(), vec!["abc123".to_string()]);
    }

    #[test]
    fn ensure_dir_ignored_writes_gitignore_and_list_skips_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("docs");
        ensure_dir_ignored(&dir).unwrap();
        assert_eq!(fs::read_to_string(dir.join(".gitignore")).unwrap(), "*\n");
        save_issue(&dir, &issue(), "t").unwrap();
        assert_eq!(list(&dir).unwrap(), vec!["PROJ-1".to_string()]);
    }

    #[test]
    fn doc_summary_reads_only_front_matter() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // A body line `source: internal` must NOT be picked up as metadata.
        let p = WikiPage { id: "x".into(), title: "My Page".into(), body_html: "source: internal\nbody".into() };
        save_wiki(dir, &p, "2026-06-30T00:00:00Z").unwrap();
        let (title, source, fetched) = doc_summary(dir, "My Page");
        assert_eq!(title, "My Page");
        assert_eq!(source, "wiki");
        assert_eq!(fetched, "2026-06-30T00:00:00Z");
        // Issue docs have no `title:`; title falls back to `summary:`.
        save_issue(dir, &issue(), "t").unwrap();
        let (t2, s2, _) = doc_summary(dir, "PROJ-1");
        assert_eq!(t2, "Add export");
        assert_eq!(s2, "issue_tracker");
    }
}
