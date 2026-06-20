# Profile Flow Editor (web) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `palugada web` profile-detail Flows section editable — add/remove/reorder steps per flow, create/delete whole flows — saving only the `flows:` block of `profile.yaml`.

**Architecture:** A surgical `profile::set_flows` rewrites just the `flows:` block (preserving comments/`description`/`fact_families`/`review_map`). One `POST /api/profile/{id}/flows` route saves the whole flows map. The web `flowsCard` (replacing the read-only Flows pills) holds the map in memory and drives add/remove/reorder using the `conventions`/`recipes` lists already in `profile_json`.

**Tech Stack:** Rust (`std::fs`, surgical text edit), `serde_json`; vanilla-JS web console; `tempfile` tests.

## Global Constraints

- Step vocabulary: engine tokens `code.recent`/`symbol.find`/`module.info`/`diff.scan`/`prd.context`; `convention(<id>)`; `recipe(<id>)`; `convention(by-file-kind)`.
- `set_flows` rewrites ONLY the `flows:` block; everything else (comments, `description`, `fact_families`, `review_map`) stays byte-for-byte.
- Flow names validated `[a-z0-9_-]`, non-empty. Empty step list allowed (`[]`). Empty flows map allowed.
- No skill regeneration — flows are read live by `brief`.
- Reuse: web `flows()` reader + `profile_json` (already returns `flows`, `conventions`, `recipes`); `write_op`/`json!`/`knowledge_dir()`.
- CI parity: `cargo build --release` + `cargo test --release` green, no new warnings. No version bump / release.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

### Task 1: `profile::set_flows` — surgical `flows:` block writer

**Files:**
- Modify: `src/profile.rs`
- Test: `src/profile.rs` (inline `#[cfg(test)]`)

**Interfaces:**
- Produces: `pub fn set_flows(kn: &Path, id: &str, flows: &BTreeMap<String, Vec<String>>) -> Result<(), String>`

- [ ] **Step 1: Add the import**

At the top of `src/profile.rs`, add to the `use` lines:
```rust
use std::collections::BTreeMap;
```

- [ ] **Step 2: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `src/profile.rs`:

```rust
#[test]
fn set_flows_replaces_block_and_preserves_rest() {
    let kn = tempfile::tempdir().unwrap();
    let dir = kn.path().join("profiles").join("p");
    std::fs::create_dir_all(&dir).unwrap();
    let original = "schema_version: \"1.0\"\nid: p\ntitle: \"P\"\nlanguages: [kotlin]\n\nfact_families:\n  - { id: vm, symbol: true }\n\n# retrieval flows comment\nflows:\n  bugfix:   [code.recent, convention(errorhandling)]\n  review:   [diff.scan, convention(by-file-kind)]\n\n# review map comment\nreview_map:\n  vm: [architecture]\n";
    std::fs::write(dir.join("profile.yaml"), original).unwrap();

    let mut flows = BTreeMap::new();
    flows.insert("bugfix".to_string(), vec!["code.recent".to_string(), "convention(errorhandling)".to_string(), "convention(r8-analyzer)".to_string()]);
    flows.insert("optimize".to_string(), vec!["convention(r8-analyzer)".to_string()]);
    set_flows(kn.path(), "p", &flows).unwrap();

    let out = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
    assert!(out.contains("bugfix: [code.recent, convention(errorhandling), convention(r8-analyzer)]"), "{out}");
    assert!(out.contains("optimize: [convention(r8-analyzer)]"));
    assert!(!out.contains("review:   [diff.scan"), "old review flow line removed");
    // rest preserved
    assert!(out.contains("# retrieval flows comment"));
    assert!(out.contains("# review map comment"));
    assert!(out.contains("review_map:\n  vm: [architecture]"));
    assert!(out.contains("fact_families:\n  - { id: vm, symbol: true }"));
    assert!(out.contains("languages: [kotlin]"));
}

#[test]
fn set_flows_rejects_bad_flow_name() {
    let kn = tempfile::tempdir().unwrap();
    let dir = kn.path().join("profiles").join("p");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("profile.yaml"), "id: p\nflows:\n  bugfix: [diff.scan]\n").unwrap();
    let mut flows = BTreeMap::new();
    flows.insert("Bad Name".to_string(), vec!["diff.scan".to_string()]);
    assert!(set_flows(kn.path(), "p", &flows).is_err());
}

#[test]
fn set_flows_inserts_block_when_absent() {
    let kn = tempfile::tempdir().unwrap();
    let dir = kn.path().join("profiles").join("p");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("profile.yaml"), "id: p\nlanguages: [kotlin]\n\nreview_map:\n  vm: [a]\n").unwrap();
    let mut flows = BTreeMap::new();
    flows.insert("bugfix".to_string(), vec!["diff.scan".to_string()]);
    set_flows(kn.path(), "p", &flows).unwrap();
    let out = std::fs::read_to_string(dir.join("profile.yaml")).unwrap();
    assert!(out.contains("flows:\n  bugfix: [diff.scan]"), "{out}");
    assert!(out.contains("review_map:"));
}
```

- [ ] **Step 3: Run tests, verify they fail**

Run: `cargo test --bin palugada set_flows`
Expected: FAIL — `set_flows` not defined.

- [ ] **Step 4: Implement `set_flows`**

Add to `src/profile.rs` (after `scaffold_new`):

```rust
fn valid_flow_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// Replace the `flows:` block of a profile's `profile.yaml` with `flows`,
/// preserving every other line (comments, description, fact_families,
/// review_map). Flow names must be `[a-z0-9_-]`; steps are written verbatim.
pub fn set_flows(kn: &Path, id: &str, flows: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    for name in flows.keys() {
        if !valid_flow_name(name) {
            return Err(format!("invalid flow name '{name}' — use only [a-z0-9_-]"));
        }
    }
    let path = kn.join("profiles").join(id).join("profile.yaml");
    let raw = fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;

    // Render the new block.
    let mut block = String::from("flows:\n");
    for (name, steps) in flows {
        block.push_str(&format!("  {}: [{}]\n", name, steps.join(", ")));
    }

    let lines: Vec<&str> = raw.lines().collect();
    let new_content = if let Some(start) = lines.iter().position(|l| l.trim_end() == "flows:") {
        // Replace `flows:` + the contiguous indented lines that follow it.
        let mut end = start + 1;
        while end < lines.len() && lines[end].starts_with([' ', '\t']) {
            end += 1;
        }
        let mut out = String::new();
        for l in &lines[..start] {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&block);
        for l in &lines[end..] {
            out.push_str(l);
            out.push('\n');
        }
        out
    } else if let Some(rm) = lines.iter().position(|l| l.trim_end() == "review_map:") {
        // No flows block yet: insert before review_map.
        let mut out = String::new();
        for l in &lines[..rm] {
            out.push_str(l);
            out.push('\n');
        }
        out.push_str(&block);
        out.push('\n');
        for l in &lines[rm..] {
            out.push_str(l);
            out.push('\n');
        }
        out
    } else {
        // Append at end.
        let mut out = raw.clone();
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
        out.push_str(&block);
        out
    };
    fs::write(&path, new_content).map_err(|e| format!("write {}: {e}", path.display()))
}
```

- [ ] **Step 5: Run tests, verify pass**

Run: `cargo test --bin palugada set_flows`
Expected: 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/profile.rs
git commit -m "$(printf 'feat(profile): set_flows surgical flows-block writer (preserves the rest)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 2: `web.rs` — save-flows route

**Files:**
- Modify: `src/web.rs`
- Test: `src/web.rs` (`route_parses_paths`)

**Interfaces:**
- New `Route` variant: `SetFlows(String)`.
- Consumes: `profile::set_flows`, `knowledge_dir()`.

- [ ] **Step 1: Write the failing route test**

Add to `route_parses_paths()` in `src/web.rs`:
```rust
    assert_eq!(route("POST", "/api/profile/p/flows"), Route::SetFlows("p".into()));
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test --bin palugada route_parses_paths`
Expected: FAIL — variant/pattern missing.

- [ ] **Step 3: Add variant, pattern, handler**

In the `Route` enum (near `AddConvention`):
```rust
    SetFlows(String),
```

In `fn route`, after the `recipe` POST pattern:
```rust
        ("POST", ["api", "profile", id, "flows"]) => Route::SetFlows((*id).to_string()),
```

In the dispatch `match` (near `Route::AddConvention`):
```rust
        Route::SetFlows(id) => write_op(|| {
            #[derive(serde::Deserialize)]
            struct Req {
                #[serde(default)]
                flows: std::collections::BTreeMap<String, Vec<String>>,
            }
            let kn = knowledge_dir()?;
            let req: Req = serde_json::from_str(body).map_err(|e| format!("bad JSON: {e}"))?;
            crate::profile::set_flows(&kn, &id, &req.flows)?;
            Ok(json!({ "ok": true, "flows": req.flows.len() }))
        }),
```

- [ ] **Step 4: Run test + build, verify pass**

Run: `cargo test --bin palugada route_parses_paths && cargo build`
Expected: route test PASS; build clean (0 warnings).

- [ ] **Step 5: Commit**

```bash
git add src/web.rs
git commit -m "$(printf 'feat(web): POST /api/profile/{id}/flows — save edited flows\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 3: `web/app.js` + `style.css` — editable Flows card

**Files:**
- Modify: `src/web/app.js`
- Modify: `src/web/style.css`

**Interfaces:**
- Consumes `POST /api/profile/<id>/flows`; reads `d.flows`, `d.conventions`, `d.recipes` from `profile_json`.

- [ ] **Step 1: Add `flowsCard`**

In `src/web/app.js`, add (near `addConventionForm`):

```javascript
function flowsCard(id, d) {
  const flows = JSON.parse(JSON.stringify(d.flows || {}));
  const convIds = (d.conventions || []).map(c => c.id);
  const recipeIds = (d.recipes || []).map(r => r.id);
  const ENGINE = ["code.recent", "symbol.find", "module.info", "diff.scan", "prd.context"];
  const card = h(`<div class="card"><h3>Flows</h3>
    <div class="muted">Steps <code>palugada brief &lt;flow&gt;</code> assembles. Saved to the profile's <code>profile.yaml</code>; <code>brief</code> uses it live — no skill regeneration needed.</div>
    <div class="flows-body"></div>
    <div class="row" style="margin-top:8px"><input class="fl-new" placeholder="new flow name (e.g. optimize)">
      <button class="ghost fl-add-flow">+ add flow</button><span class="spacer"></span><button class="fl-save">Save flows</button></div></div>`);
  const bodyEl = card.querySelector(".flows-body");

  function rerender() {
    bodyEl.innerHTML = "";
    Object.keys(flows).forEach(fname => {
      const fl = h(`<div class="flow"><div class="row"><strong>${esc(fname)}</strong> <a class="link fl-del-flow">× flow</a></div><div class="fl-steps"></div></div>`);
      const stepsEl = fl.querySelector(".fl-steps");
      flows[fname].forEach((step, i) => {
        const chip = h(`<span class="step-chip"><code>${esc(step)}</code> <a class="link fl-up">↑</a> <a class="link fl-down">↓</a> <a class="link fl-rm">×</a></span>`);
        chip.querySelector(".fl-rm").onclick = () => { flows[fname].splice(i, 1); rerender(); };
        chip.querySelector(".fl-up").onclick = () => { if (i > 0) { const a = flows[fname]; [a[i - 1], a[i]] = [a[i], a[i - 1]]; rerender(); } };
        chip.querySelector(".fl-down").onclick = () => { const a = flows[fname]; if (i < a.length - 1) { [a[i], a[i + 1]] = [a[i + 1], a[i]]; rerender(); } };
        stepsEl.appendChild(chip);
      });
      const add = h(`<div class="row" style="margin-top:4px">
        <select class="fl-kind"><option value="engine">engine</option><option value="convention">convention(…)</option><option value="recipe">recipe(…)</option><option value="bfk">convention(by-file-kind)</option></select>
        <select class="fl-val"></select><button class="ghost fl-add-step">add step</button></div>`);
      const kindEl = add.querySelector(".fl-kind");
      const valEl = add.querySelector(".fl-val");
      function fillVal() {
        const k = kindEl.value;
        const opts = k === "engine" ? ENGINE : k === "convention" ? convIds : k === "recipe" ? recipeIds : [];
        valEl.style.display = opts.length ? "" : "none";
        valEl.innerHTML = opts.map(o => `<option>${esc(o)}</option>`).join("");
      }
      kindEl.onchange = fillVal;
      fillVal();
      add.querySelector(".fl-add-step").onclick = () => {
        const k = kindEl.value;
        let step;
        if (k === "engine") step = valEl.value;
        else if (k === "convention") step = valEl.value ? `convention(${valEl.value})` : "";
        else if (k === "recipe") step = valEl.value ? `recipe(${valEl.value})` : "";
        else step = "convention(by-file-kind)";
        if (!step) { toast("nothing to add (no conventions/recipes?)", true); return; }
        flows[fname].push(step);
        rerender();
      };
      fl.appendChild(add);
      fl.querySelector(".fl-del-flow").onclick = () => {
        if (confirm(`delete flow '${fname}'?`)) { delete flows[fname]; rerender(); }
      };
      bodyEl.appendChild(fl);
    });
  }
  rerender();

  card.querySelector(".fl-add-flow").onclick = () => {
    const name = card.querySelector(".fl-new").value.trim();
    if (!/^[a-z0-9_-]+$/.test(name)) { toast("flow name: a-z 0-9 - _ only", true); return; }
    if (flows[name]) { toast("flow already exists", true); return; }
    flows[name] = [];
    card.querySelector(".fl-new").value = "";
    rerender();
  };
  card.querySelector(".fl-save").onclick = async () => {
    try {
      await api(`/api/profile/${encodeURIComponent(id)}/flows`, "POST", { flows });
      toast("saved flows");
      renderProfileDetail(id);
    } catch (e) { toast(e.message, true); }
  };
  return card;
}
```

- [ ] **Step 2: Replace the read-only Flows pills with the editor**

In `renderProfileDetail(id)`, the combined "Fact families / Flows" card currently
renders both as pills. Change it to a **Fact families-only** card, then append the
editable flows card. Replace the block that appends `<h3>Fact families</h3> … <h3>Flows</h3> … pills …`
with:

```javascript
  view.appendChild(h(`<div class="card"><h3>Fact families</h3>
    <div class="muted">Categories of symbols palugada extracts from YOUR code (e.g. viewmodel, repository, command).
    <code>palugada fact &lt;family&gt;</code> lists them with file:line. Defined in the profile's
    <code>extractors.yaml</code> (regex / tree-sitter) — not edited here.</div><div>${
    d.fact_families.map(f => `<span class="pill">${esc(f)}</span>`).join("") || '<span class="muted">none</span>'
  }</div></div>`));
  view.appendChild(flowsCard(id, d));
```

(This drops the old read-only Flows pills + its caption from that card; the
flowsCard now owns Flows. Keep the `generateForm(id)` append that follows.)

- [ ] **Step 3: Style chips + flow box**

Append to `src/web/style.css`:
```css
.flow { border: 1px dashed #313845; border-radius: 6px; padding: 8px; margin: 6px 0; }
.step-chip { display: inline-block; background: #2b313c; border-radius: 4px; padding: 2px 7px; margin: 2px; font-size: 12px; }
.step-chip code { color: #cdd6e0; }
```

- [ ] **Step 4: Build + render check**

Run: `cargo build` (assets are `include_str!`, binary must rebuild). Then serve and
confirm the editor strings ship:
```bash
./target/debug/palugada web --port 7799 >/dev/null 2>&1 & SRV=$!; sleep 1.5
curl -s http://127.0.0.1:7799/app.js | grep -oE "function flowsCard|Save flows" | sort -u
kill $SRV 2>/dev/null
```
Expected: both strings present. Then `cargo run -q -- web --port 7799`, open Profiles →
android-mvvm → confirm the Flows editor renders each flow with step chips + add controls.

- [ ] **Step 5: Commit**

```bash
git add src/web/app.js src/web/style.css
git commit -m "$(printf 'feat(web): editable Flows card — add/remove/reorder steps, add/delete flows\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 4: End-to-end verification

**Files:** none (verification only).

- [ ] **Step 1: Full build + test**

Run: `cargo test --release && cargo build --release`
Expected: all tests pass (≥107 + new set_flows/route tests), no warnings, release builds.

- [ ] **Step 2: Live e2e — add a step + new flow via the API, brief honors it**

Drive the route directly (the UI is thin over it). Back up android-mvvm's
profile.yaml first so the demo is reversible.

```bash
BIN="$PWD/target/release/palugada"; KN="$PWD/knowledge"
cp "$KN/profiles/android-mvvm/profile.yaml" /tmp/android-mvvm-profile.bak
"$BIN" web --port 7799 >/dev/null 2>&1 & SRV=$!; sleep 1.5
# add convention(r8-analyzer) to bugfix + a new optimize flow (send the full flows map)
curl -s -X POST http://127.0.0.1:7799/api/profile/android-mvvm/flows -H 'Content-Type: application/json' -d '{
  "flows": {
    "bugfix":   ["code.recent","symbol.find","convention(errorhandling)","convention(testing)","convention(r8-analyzer)"],
    "feature":  ["prd.context","recipe(feature)","module.info","convention(architecture)"],
    "refactor": ["module.info","convention(architecture)","convention(style)","recipe(refactor)"],
    "review":   ["diff.scan","convention(by-file-kind)"],
    "optimize": ["convention(r8-analyzer)"]
  }}'
kill $SRV 2>/dev/null
echo; echo "=== profile.yaml: flows updated + comments/review_map preserved? ==="
grep -nE "convention\(r8-analyzer\)|optimize:|review_map:|# Maps each fact family" "$KN/profiles/android-mvvm/profile.yaml"
echo "=== brief bugfix pulls r8? ==="
PALUGADA_KNOWLEDGE="$KN" "$BIN" brief bugfix Foo --project status-saver 2>/dev/null | grep -iE "convention: r8-analyzer|R8 Analyzer"
echo "=== brief optimize works? ==="
PALUGADA_KNOWLEDGE="$KN" "$BIN" brief optimize Foo --project status-saver 2>/dev/null | grep -iE "R8 Analyzer|convention: r8-analyzer"
```
Expected: profile.yaml shows `convention(r8-analyzer)` in bugfix + an `optimize:`
flow; the `review_map:` block and the `# Maps each fact family…` comment are still
present; `brief bugfix` and `brief optimize` render the r8-analyzer convention.

- [ ] **Step 3: Restore the demo edit**

```bash
KN="$PWD/knowledge"
cp /tmp/android-mvvm-profile.bak "$KN/profiles/android-mvvm/profile.yaml"
git status --porcelain knowledge/profiles/android-mvvm/profile.yaml   # expect clean
```
(The flow wiring was a demo; restore so the commit only contains code. If the user
wants r8 wired for real, do it as a separate explicit change.)

- [ ] **Step 4: Confirm tree clean + reinstall**

Run: `git status --porcelain` (only expected: the untracked `knowledge/profiles/kmp/`
the user created — leave it). Then `cargo install --path . --force`.

---

## Self-review

**Spec coverage:**
- Surgical `flows:` block write preserving the rest → Task 1 `set_flows` + comment-preservation test.
- Save route → Task 2 `SetFlows`.
- Editable Flows UI (add/remove/reorder steps, add/delete flows, step vocabulary) → Task 3 `flowsCard`.
- Read flows already works (`profile_json`) — no new read code.
- Tests + e2e (brief honors edits, comments/review_map survive) → Task 1 tests + Task 4.
- No-regen note → Task 3 caption.

**Placeholder scan:** All code steps contain concrete code. No TBD/TODO.

**Type consistency:** `set_flows(kn, id, &BTreeMap<String, Vec<String>>)` defined in Task 1, called in Task 2's handler with the deserialized `req.flows` (same type). `Route::SetFlows(String)` consistent across enum/pattern/handler/test. `flowsCard(id, d)` consumes `d.flows`/`d.conventions`/`d.recipes` (the exact keys `profile_json` returns). Engine token list matches `skillmap::engine_label`'s five tokens.
