# Web Markdown Upload + Split → Conventions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** In `palugada web`, upload/paste a markdown file, split it by `# H1` into candidate conventions, preview (edit id/title/tags, include/exclude), and import the selected ones into the profile.

**Architecture:** A pure splitter in `knowledge.rs` (`split_markdown_conventions`) feeds two web routes — `import/preview` (no-write parse, annotates `exists`) and `import/commit` (loops the existing `add_convention_from_markdown`). The browser reads the file with `FileReader` and drives a preview→commit flow; commit normalizes each piece into front-matter + body and reuses the importer (sections/tokens/anchors/`_index.json` are produced automatically).

**Tech Stack:** Rust, `serde_json`/`serde_yaml`, `tempfile` tests; vanilla-JS web console (`FileReader`, no multipart).

## Global Constraints

- Split by level-1 heading (`# H1`), fence-aware (a `# ` inside ``` is body text). Each `# H1` block = one candidate; its `## ` headings = sections; the candidate `body` is the markdown **after** the H1 line (H1 excluded). No `# H1` → one candidate over the whole body (title from file front-matter, else empty).
- Conventions only (no recipes); target = the profile (no overlay).
- id auto = `slug(title)`; validated against `[a-z0-9_-]`. Upsert by id (created/updated).
- Commit validates **all** pieces before writing; if any piece has an empty/invalid id or empty title, write none and return an error.
- Preview echoes each candidate's `body` so commit is a faithful round-trip.
- Reuse existing: `parse_doc_front_matter`, `strip_frontmatter`, `slug`, `sections()`, `add_convention_from_markdown`, `conventions()`, `knowledge_dir()`, `write_op`/`jv`.
- CI parity: `cargo build --release` + `cargo test --release` green, no new warnings. No version bump / release.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

### Task 1: `knowledge.rs` — `split_markdown_conventions` + `valid_doc_id`

**Files:**
- Modify: `src/knowledge.rs`
- Test: `src/knowledge.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces:
  - `pub struct ConventionDraft { pub id: String, pub title: String, pub sections: Vec<String>, pub body: String }`
  - `pub fn split_markdown_conventions(raw: &str) -> Vec<ConventionDraft>`
  - `pub fn valid_doc_id(id: &str) -> bool`
- Consumes existing: `parse_doc_front_matter`, `strip_frontmatter`, `slug`, `sections()`, `validate_doc_id`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/knowledge.rs`:

```rust
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
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test --bin palugada split_ valid_doc_id_rules`
Expected: FAIL — items not defined. (Run each name separately; `cargo test` takes one filter — use `cargo test --bin palugada split_` then `cargo test --bin palugada valid_doc_id_rules`.)

- [ ] **Step 3: Implement the splitter + validator**

In `src/knowledge.rs`, near the other doc helpers (after `first_h1`), add:

```rust
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

fn is_h1(line: &str) -> bool {
    // `# Heading` but not `## ...`
    line.trim_start().strip_prefix("# ").map(|r| !r.is_empty()).unwrap_or(false)
}

fn h1_text(line: &str) -> String {
    line.trim_start().strip_prefix("# ").map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Split a markdown document into candidate conventions, one per `# H1`
/// (fence-aware). No `# H1` → a single draft over the whole body, titled from
/// the file front-matter (else empty). Pure; no I/O.
pub fn split_markdown_conventions(raw: &str) -> Vec<ConventionDraft> {
    let file_meta = parse_doc_front_matter(raw).unwrap_or_default();
    let body = strip_frontmatter(raw);
    let lines: Vec<&str> = body.lines().collect();

    // H1 line indices, ignoring lines inside ``` fences.
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
        let id = if title.is_empty() { String::new() } else { slug(&title) };
        let b = body.trim().to_string();
        let secs = sections(&b).into_iter().map(|s| s.title).collect();
        return vec![ConventionDraft { id, title, sections: secs, body: b }];
    }

    let mut drafts = Vec::new();
    for (k, &start) in h1_idx.iter().enumerate() {
        let end = h1_idx.get(k + 1).copied().unwrap_or(lines.len());
        let title = h1_text(lines[start]);
        let id = slug(&title);
        let piece_body = lines[start + 1..end].join("\n").trim().to_string();
        let secs = sections(&piece_body).into_iter().map(|s| s.title).collect();
        drafts.push(ConventionDraft { id, title, sections: secs, body: piece_body });
    }
    drafts
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cargo test --bin palugada split_` then `cargo test --bin palugada valid_doc_id_rules`
Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add src/knowledge.rs
git commit -m "$(printf 'feat(knowledge): split_markdown_conventions splitter + valid_doc_id\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 2: `web.rs` — import preview + commit routes

**Files:**
- Modify: `src/web.rs`
- Test: `src/web.rs` (`route_parses_paths`)

**Interfaces:**
- New `Route` variants: `ImportPreview(String)`, `ImportCommit(String)`.
- Consumes: `split_markdown_conventions`, `conventions`, `add_convention_from_markdown`, `valid_doc_id`, `knowledge_dir()`.

- [ ] **Step 1: Write the failing route test**

Add to `route_parses_paths()` in `src/web.rs`:

```rust
    assert_eq!(route("POST", "/api/profile/p/import/preview"), Route::ImportPreview("p".into()));
    assert_eq!(route("POST", "/api/profile/p/import/commit"), Route::ImportCommit("p".into()));
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --bin palugada route_parses_paths`
Expected: FAIL — variants/patterns missing.

- [ ] **Step 3: Add variants, patterns, handlers**

In the `Route` enum:

```rust
    ImportPreview(String),
    ImportCommit(String),
```

In `fn route`, before `_ => Route::NotFound`:

```rust
        ("POST", ["api", "profile", id, "import", "preview"]) => Route::ImportPreview((*id).to_string()),
        ("POST", ["api", "profile", id, "import", "commit"]) => Route::ImportCommit((*id).to_string()),
```

In the dispatch `match` (near `Route::AddConvention`):

```rust
        Route::ImportPreview(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req { markdown: String }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            let existing: std::collections::BTreeSet<String> =
                crate::knowledge::conventions(&kn, &id)?.into_iter().map(|c| c.id).collect();
            let candidates: Vec<serde_json::Value> = crate::knowledge::split_markdown_conventions(&req.markdown)
                .into_iter()
                .map(|d| json!({
                    "id": d.id, "title": d.title, "sections": d.sections,
                    "body": d.body, "exists": existing.contains(&d.id),
                }))
                .collect();
            let warnings: Vec<String> = if candidates.is_empty() {
                vec!["no headings found — add a `# Heading` per topic".to_string()]
            } else {
                vec![]
            };
            Ok(json!({ "candidates": candidates, "warnings": warnings }))
        }),
        Route::ImportCommit(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Piece {
                #[serde(default)] id: String,
                #[serde(default)] title: String,
                #[serde(default)] description: String,
                #[serde(default)] tags: Vec<String>,
                #[serde(default)] body: String,
            }
            #[derive(serde::Deserialize)]
            struct Req { pieces: Vec<Piece> }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            if req.pieces.is_empty() {
                return Err("no pieces selected".to_string());
            }
            // Validate ALL before writing (write none on any invalid).
            for p in &req.pieces {
                if p.title.trim().is_empty() {
                    return Err(format!("piece '{}' needs a title", p.id));
                }
                if !crate::knowledge::valid_doc_id(p.id.trim()) {
                    return Err(format!("invalid id '{}' — use only [a-z0-9_-]", p.id));
                }
            }
            let dir = kn.join("profiles").join(&id).join("conventions");
            let (mut created, mut updated) = (0u32, 0u32);
            let mut ids: Vec<String> = Vec::new();
            for p in &req.pieces {
                let raw = format!(
                    "---\nid: {}\ntitle: {}\ndescription: {}\ntags: [{}]\n---\n\n# {}\n{}",
                    p.id.trim(), p.title.trim(), p.description, p.tags.join(", "), p.title.trim(), p.body
                );
                let (cid, replaced) = crate::knowledge::add_convention_from_markdown(&dir, &raw)?;
                if replaced { updated += 1 } else { created += 1 }
                ids.push(cid);
            }
            Ok(json!({ "created": created, "updated": updated, "ids": ids }))
        }),
```

- [ ] **Step 4: Run tests + build, verify pass**

Run: `cargo test --bin palugada route_parses_paths && cargo build`
Expected: route test PASS; build clean (0 warnings).

- [ ] **Step 5: Commit**

```bash
git add src/web.rs
git commit -m "$(printf 'feat(web): import/preview + import/commit routes (split markdown to conventions)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 3: `web/app.js` + `style.css` — upload panel & preview

**Files:**
- Modify: `src/web/app.js`
- Modify: `src/web/style.css`

**Interfaces:**
- Consumes `POST /api/profile/<id>/import/preview` and `.../import/commit`.
- Hooks into `renderProfileDetail(id)`.

- [ ] **Step 1: Add the import card + preview functions**

In `src/web/app.js`, add two functions (near `addConventionForm`):

```javascript
function importCard(id) {
  const box = h(`<div class="card"><h3>Import markdown file</h3>
    <div class="muted">Upload or paste a markdown doc; palugada splits it by <code># H1</code> into candidate
    conventions (sections come from <code>##</code>). Preview, edit, then import into this profile.</div>
    <input type="file" class="im-file" accept=".md,.markdown,text/markdown">
    <label>or paste markdown</label><textarea class="im-text" placeholder="# Firebase Integration&#10;## Setup&#10;..."></textarea>
    <div class="row" style="margin-top:6px"><button class="im-detect">Detect</button></div>
    <div class="im-preview"></div></div>`);
  const fileEl = box.querySelector(".im-file");
  const textEl = box.querySelector(".im-text");
  fileEl.onchange = () => {
    const f = fileEl.files[0];
    if (!f) return;
    const r = new FileReader();
    r.onload = () => { textEl.value = r.result; };
    r.readAsText(f);
  };
  box.querySelector(".im-detect").onclick = async () => {
    const markdown = textEl.value;
    if (!markdown.trim()) { toast("paste or choose a markdown file first", true); return; }
    try {
      const res = await api(`/api/profile/${encodeURIComponent(id)}/import/preview`, "POST", { markdown });
      renderImportPreview(box.querySelector(".im-preview"), id, res);
    } catch (e) { toast(e.message, true); }
  };
  return box;
}

function renderImportPreview(host, id, res) {
  host.innerHTML = "";
  if (res.warnings && res.warnings.length) {
    host.appendChild(h(`<div class="warn">${res.warnings.map(w => `⚠ ${esc(w)}`).join("<br>")}</div>`));
  }
  if (!res.candidates || !res.candidates.length) return;
  const rows = [];
  res.candidates.forEach(c => {
    const badge = c.exists ? `<span class="warn-pill">will update</span>` : `<span class="ok-pill">new</span>`;
    const row = h(`<div class="candidate">
      <label class="row"><input type="checkbox" class="im-inc" checked style="width:auto"> &nbsp;include</label> ${badge}
      <label>id</label><input class="im-id" value="${esc(c.id)}">
      <label>title</label><input class="im-title" value="${esc(c.title)}">
      <label>tags (comma-separated)</label><input class="im-tags" value="">
      <div class="muted">sections: ${c.sections.map(esc).join(", ") || "(none)"}</div>
    </div>`);
    row._body = c.body;
    rows.push(row);
    host.appendChild(row);
  });
  const go = h(`<div class="row" style="margin-top:6px"><span class="spacer"></span><button class="im-go">Import selected</button></div>`);
  host.appendChild(go);
  go.querySelector(".im-go").onclick = async () => {
    const pieces = rows.filter(r => r.querySelector(".im-inc").checked).map(r => ({
      id: r.querySelector(".im-id").value.trim(),
      title: r.querySelector(".im-title").value.trim(),
      description: "",
      tags: splitCsv(r.querySelector(".im-tags").value),
      body: r._body,
    }));
    if (!pieces.length) { toast("select at least one candidate", true); return; }
    if (pieces.some(p => !p.id || !p.title)) { toast("every selected piece needs id + title", true); return; }
    try {
      const r2 = await api(`/api/profile/${encodeURIComponent(id)}/import/commit`, "POST", { pieces });
      toast(`imported: ${r2.created} created, ${r2.updated} updated`);
      renderProfileDetail(id);
    } catch (e) { toast(e.message, true); }
  };
}
```

- [ ] **Step 2: Mount the card in `renderProfileDetail`**

In `renderProfileDetail(id)`, after the Conventions card is appended
(`view.appendChild(cv);`), add:

```javascript
  view.appendChild(importCard(id));
```

- [ ] **Step 3: Style the candidate rows + textarea**

In `src/web/style.css`, append:

```css
.candidate { border: 1px dashed #313845; border-radius: 6px; padding: 8px; margin: 6px 0; }
.im-text { width: 100%; min-height: 120px; }
```

- [ ] **Step 4: Build + manual render check**

Run: `cargo build` (assets are `include_str!`, so the binary must rebuild).
Then `cargo run -q -- web --port 7799` (open the printed loopback URL), Profiles →
a profile → confirm the "Import markdown file" card renders with file input,
textarea, and Detect.

Quick automated check that the new strings are served:
```bash
./target/debug/palugada web --port 7799 >/dev/null 2>&1 &
SRV=$!; sleep 1.5
curl -s http://127.0.0.1:7799/app.js | grep -oE "Import markdown file|renderImportPreview" | sort -u
kill $SRV 2>/dev/null
```
Expected: both strings present.

- [ ] **Step 5: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "$(printf 'feat(web): Import markdown card — upload/paste, split preview, import selected\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 4: End-to-end verification

**Files:** none (verification only).

- [ ] **Step 1: Full build + test**

Run: `cargo test --release && cargo build --release`
Expected: all tests pass (≥102 + new splitter/route tests), no warnings, release builds.

- [ ] **Step 2: Live e2e — preview + commit against a profile**

Drive the API directly (the UI is thin over it). Use a registered profile that is
safe to mutate, then clean up. Recommended: import into `rust-cli` (or
`android-mvvm`) and delete the demo convention after.

```bash
BIN="$PWD/target/release/palugada"; KN="$PWD/knowledge"
"$BIN" web --port 7799 >/dev/null 2>&1 & SRV=$!; sleep 1.5
B=http://127.0.0.1:7799
# single-H1 doc → one candidate
PREVIEW='{"markdown":"# Firebase Integration\n> add firebase\n\n## Setup\nadd the SDK\n\n## Auth\nsign-in flow\n"}'
echo "=== preview (android-mvvm) ==="; curl -s -X POST "$B/api/profile/android-mvvm/import/preview" -H 'Content-Type: application/json' -d "$PREVIEW" | python3 -m json.tool
echo "=== commit ==="; curl -s -X POST "$B/api/profile/android-mvvm/import/commit" -H 'Content-Type: application/json' \
  -d '{"pieces":[{"id":"firebase-integration","title":"Firebase Integration","description":"","tags":["firebase","android"],"body":"> add firebase\n\n## Setup\nadd the SDK\n\n## Auth\nsign-in flow\n"}]}'
kill $SRV 2>/dev/null
echo; echo "=== stored + anchored + listed? ==="
PALUGADA_KNOWLEDGE="$KN" "$BIN" q firebase-integration --profile android-mvvm | head -8
PALUGADA_KNOWLEDGE="$KN" "$BIN" q --list --profile android-mvvm | grep firebase-integration
# cleanup
rm -f "$KN/profiles/android-mvvm/conventions/firebase-integration.md"
git checkout "$KN/profiles/android-mvvm/conventions/_index.json"
echo "=== tree clean? ==="; git status --porcelain knowledge/
```
Expected: preview returns one candidate `firebase-integration` with sections
`["Setup","Auth"]`, `exists:false`; commit returns `created:1`; `q` shows the body
with injected anchors (`## Setup {#setup}`); `q --list` shows it; cleanup leaves
`knowledge/` clean.

- [ ] **Step 3: Live e2e — multi-`# H1` split**

```bash
BIN="$PWD/target/release/palugada"
"$BIN" web --port 7799 >/dev/null 2>&1 & SRV=$!; sleep 1.5
curl -s -X POST http://127.0.0.1:7799/api/profile/android-mvvm/import/preview -H 'Content-Type: application/json' \
  -d '{"markdown":"# Alpha\n## One\nx\n# Beta\n## Two\ny\n"}' | python3 -c "import sys,json; d=json.load(sys.stdin); print([c['id'] for c in d['candidates']])"
kill $SRV 2>/dev/null
```
Expected: `['alpha', 'beta']` (two candidates).

- [ ] **Step 4: Confirm working tree clean + reinstall**

Run: `git status --porcelain` (empty) then `cargo install --path . --force` so the
local `palugada` has the feature.

---

## Self-review

**Spec coverage:**
- Split rule (`# H1`, fence-aware, sections from `##`, body excludes H1, no-H1 single draft) → Task 1 `split_markdown_conventions` + tests.
- preview route (annotate `exists`, warnings) → Task 2 `ImportPreview`.
- commit route (validate-all, normalize per piece, reuse importer, created/updated) → Task 2 `ImportCommit`.
- web upload/paste → Detect → preview (include/edit id/title/tags) → Import selected → Task 3.
- conventions-only, profile target, upsert → Tasks 2/3 (no recipe/overlay paths).
- error handling (empty markdown, no candidates, empty/invalid id, collision=update) → Task 2 handlers + Task 3 UI guards.
- tests + e2e → Task 1 unit tests + Task 2 route test + Task 4.

**Placeholder scan:** All code steps contain concrete code. No TBD/TODO.

**Type consistency:** `ConventionDraft { id, title, sections, body }` produced in Task 1, serialized in Task 2's preview, echoed as `body` and consumed by Task 3's commit pieces. `valid_doc_id(&str)->bool` defined in Task 1, used in Task 2. Commit `pieces:[{id,title,description,tags,body}]` shape matches between Task 2 (`Piece`) and Task 3 (JS object). `add_convention_from_markdown(dir,&str)->(String,bool)` consumed unchanged. Routes `ImportPreview(String)`/`ImportCommit(String)` consistent across enum/patterns/handlers/test.
