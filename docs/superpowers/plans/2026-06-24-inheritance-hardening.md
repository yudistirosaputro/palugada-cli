# Inheritance Hardening — Deferred Minors Cleanup

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Clear the six deferred-minor items from the profile-inheritance work (Plan A + Plan B) in one hardening pass.

**Architecture:** Three independent tasks by file — `src/inherit.rs` (I/O error propagation + carry tags in merged front-matter), `src/profile.rs` (a new md⇔index section-id parity validate check), `src/web.rs`+`src/web/app.js` (graceful `profile_json` degrade on a broken `extends`, a `languages:`/`title:`-at-offset-0 fallback, and removing a latent `CSS.escape`/`esc` coupling).

**Tech Stack:** Rust 2021 (`serde_json`, `tiny_http`), vanilla JS (`app.js`).

## Global Constraints

- Rust 2021; `Result<T,String>`; no `unwrap()/expect()/panic!` outside `#[cfg(test)]`; no new deps.
- **DO NOT run `cargo fmt`** (repo hand-formats wide-style; no rustfmt.toml). Match surrounding style; touch only required lines.
- **BIN crate — no lib target.** `cargo build` / `cargo test` (NOT `--lib`). Frontend: `node --check src/web/app.js` + manual reasoning.
- Build stays **warning-clean**; all tests green.

---

### Task 1: `inherit.rs` — I/O error propagation + carry `tags` in merged front-matter

**Files:** Modify `src/inherit.rs`. Test inline.

- [ ] **Step 1: Propagate non-NotFound I/O errors in `resolve_convention_raw`**

Replace the read loop (currently `if let Ok(raw) = std::fs::read_to_string(&md) { present.push(raw); }`) with:

```rust
        match std::fs::read_to_string(&md) {
            Ok(raw) => present.push(raw),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", md.display())),
        }
```

- [ ] **Step 2: Same for `resolve_recipe_raw`**

Replace its `if let Ok(raw) = std::fs::read_to_string(&path) { return Ok(Some(raw)); }` with:

```rust
        match std::fs::read_to_string(&path) {
            Ok(raw) => return Ok(Some(raw)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("read {}: {e}", path.display())),
        }
```

- [ ] **Step 3: Carry `tags` in `render_merged`**

Change `render_merged` to accept tags and emit a `tags:` line when non-empty:

```rust
fn render_merged(
    topic: &str,
    title: &str,
    description: &str,
    tags: &str,
    preamble: &str,
    secs: &[MergedSection],
) -> String {
    let mut out = format!("---\nid: {topic}\ntitle: {title}\ndescription: {description}\n");
    if !tags.is_empty() {
        out.push_str(&format!("tags: {tags}\n"));
    }
    out.push_str("---\n\n");
    out.push_str(preamble.trim_end());
    out.push('\n');
    for s in secs {
        out.push_str(&format!(
            "\n## {} {{#{}}}\n{}\n",
            s.title,
            s.anchor,
            s.body.trim_end()
        ));
    }
    out
}
```

In `resolve_convention_raw`, after `let description = pick("description");` add `let tags = pick("tags");` and pass it: `render_merged(topic, &title, &description, &tags, &preamble, &merged)`.

(`pick` reads the nearest-non-empty front-matter scalar child-first; convention front-matter stores tags inline as `tags: [a, b]`, so `pick("tags")` yields the `[a, b]` text — emitting `tags: [a, b]` is valid. The single-level verbatim path is unaffected.)

- [ ] **Step 4: Tests** (add to `inherit.rs` `#[cfg(test)] mod tests`)

```rust
    #[test]
    fn resolve_convention_propagates_non_notfound_io_error() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        // Make <topic>.md a DIRECTORY → read_to_string fails with a non-NotFound error.
        let cdir = kn.path().join("profiles/p/conventions");
        std::fs::create_dir_all(cdir.join("arch.md")).unwrap();
        let r = resolve_convention_raw(kn.path(), "p", "arch");
        assert!(r.is_err(), "a non-NotFound read error must propagate, got {r:?}");
    }

    #[test]
    fn merged_convention_carries_tags_from_nearest() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        // base defines arch with tags + a section; child overrides one section, omits tags
        conv(kn.path(), "base", "architecture",
            "---\nid: architecture\ntitle: Architecture\ntags: [kt, mvvm]\n---\n\n# Architecture\n\n## Layers {#layers}\nL\n");
        conv(kn.path(), "child", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nDF\n");
        let raw = resolve_convention_raw(kn.path(), "child", "architecture").unwrap().unwrap();
        assert!(raw.contains("tags: [kt, mvvm]"), "merged front-matter carries parent tags: {raw}");
    }
```

- [ ] **Step 5: Verify + commit**

Run: `cargo build 2>&1 | tail -2 && cargo test inherit::tests 2>&1 | tail -4` — clean, all pass (was 140 + 2 new). NO cargo fmt.

```bash
git add src/inherit.rs
git commit -m "harden(inherit): propagate non-NotFound read errors; carry tags in merged front-matter"
```

---

### Task 2: `profile.rs` — md⇔index section-id parity validate check

**Files:** Modify `src/profile.rs` (`web_render_checks`). Test inline.

**Interfaces:** Consumes `crate::knowledge::strip_frontmatter` (pub), `crate::inherit::parse_sections` (pub), `crate::knowledge::conventions`.

- [ ] **Step 1: Add a parity check** in `web_render_checks`, immediately AFTER the `out.push(check("doc files present", files));` line (and before the final `extends chain` push)

```rust
    // 5. md⇔index parity: each LOCAL topic's _index section ids match the
    //    `{#anchor}` ids actually present in its .md body.
    let mut parity: Result<String, String> = Ok("section ids match between _index and .md".into());
    'parity: {
        for t in &local_topics {
            let md_path = dir.join("conventions").join(format!("{}.md", t.id));
            let raw = match std::fs::read_to_string(&md_path) {
                Ok(r) => r,
                Err(_) => continue, // absence already reported by the doc-files check
            };
            let body = crate::knowledge::strip_frontmatter(&raw);
            let md_ids: BTreeSet<String> =
                crate::inherit::parse_sections(body).into_iter().map(|s| s.anchor).collect();
            let idx_ids: BTreeSet<String> = t.sections.iter().map(|s| s.id.clone()).collect();
            if md_ids != idx_ids {
                let only_idx: Vec<&str> = idx_ids.difference(&md_ids).map(|s| s.as_str()).collect();
                let only_md: Vec<&str> = md_ids.difference(&idx_ids).map(|s| s.as_str()).collect();
                parity = Err(format!(
                    "convention '{}' section ids differ: in _index only {:?}, in .md only {:?}",
                    t.id, only_idx, only_md
                ));
                break 'parity;
            }
        }
    }
    out.push(check("section id parity", parity));
```

(`local_topics` and `dir` are already in scope above. `BTreeSet` is already imported in profile.rs.)

- [ ] **Step 2: Test** (add to `profile.rs` tests)

```rust
    #[test]
    fn validate_flags_md_index_section_mismatch() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "p", None);
        // _index lists section `layers`, but the .md only has `## Overview {#overview}`
        fs::write(
            kn.path().join("profiles/p/conventions/_index.json"),
            r#"{"topics":[{"id":"arch","title":"Arch","sections":[{"id":"layers","title":"Layers","tokens":10}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/p/conventions/arch.md"),
            "# Arch\n## Overview {#overview}\nx\n").unwrap();
        let checks = validate(kn.path(), "p");
        let c = checks.iter().find(|c| c.name == "section id parity").unwrap();
        assert!(!c.ok && c.detail.contains("layers") && c.detail.contains("overview"), "{}", c.detail);
    }

    #[test]
    fn validate_passes_when_md_index_match() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "p", None);
        fs::write(
            kn.path().join("profiles/p/conventions/_index.json"),
            r#"{"topics":[{"id":"arch","title":"Arch","sections":[{"id":"overview","title":"Overview","tokens":10}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/p/conventions/arch.md"),
            "# Arch\n## Overview {#overview}\nx\n").unwrap();
        let parity = validate(kn.path(), "p").into_iter().find(|c| c.name == "section id parity").unwrap();
        assert!(parity.ok, "{}", parity.detail);
    }
```

(`base_profile` helper already exists in profile.rs tests from Plan A.)

- [ ] **Step 3: Verify the 4 BUNDLED profiles still validate clean** (the new check enforces real consistency)

Run:
```bash
cargo build 2>&1 | tail -2
KP="$(pwd)/knowledge"
for p in rust-cli android-mvvm flutter-bloc kmp; do
  echo "== $p =="; PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada profile validate "$p" 2>&1 | tail -12
done
```
Expected: every profile prints `profile '<id>' OK` with a `[ok ] section id parity` line. **If any profile FAILS the new parity check, STOP and report it** (it indicates real md⇔index drift in a bundled profile) — do not weaken the check; the controller decides whether to fix the profile data.

- [ ] **Step 4: Run the suite + commit**

Run: `cargo test 2>&1 | tail -4` — all green (was 142 after Task 1; +2 new). NO cargo fmt.

```bash
git add src/profile.rs
git commit -m "harden(validate): assert _index section ids match the .md {#anchor} ids"
```

---

### Task 3: `web.rs` + `app.js` — graceful degrade, offset-0 fallback, drop selector coupling

**Files:** Modify `src/web.rs` (`profile_json`, `create_profile`), `src/web/app.js` (`docRow` override toggle). Test inline (web.rs).

- [ ] **Step 1: `profile_json` degrades on a broken `extends`** (replace the fn, web.rs ~583-595)

```rust
fn profile_json(id: &str) -> Result<serde_json::Value, String> {
    let kn = knowledge_dir()?;
    match crate::inherit::resolve_chain(&kn, id) {
        Ok(chain) => Ok(json!({
            "id": id,
            "extends": crate::inherit::read_extends(&kn, id),
            "chain": chain,
            "conventions": jv(&crate::inherit::merged_conventions_provenance(&kn, id)?),
            "recipes": jv(&crate::inherit::merged_recipes_provenance(&kn, id)?),
            "fact_families": crate::indexer::fact_families(&kn, id).unwrap_or_default(),
            "flows": jv(&flows(&kn, id).unwrap_or_default()),
        })),
        // A broken `extends` (cycle / missing parent / too deep) must not 500 the
        // whole profile view — degrade to this profile's own docs + a warning so
        // the user can still open and fix it in the console.
        Err(e) => Ok(json!({
            "id": id,
            "extends": crate::inherit::read_extends(&kn, id),
            "chain": [id],
            "warning": format!("inheritance chain error: {e}"),
            "conventions": jv(&crate::knowledge::conventions(&kn, id).unwrap_or_default()),
            "recipes": jv(&crate::knowledge::recipes(&kn, id).unwrap_or_default()),
            "fact_families": crate::indexer::fact_families(&kn, id).unwrap_or_default(),
            "flows": jv(&flows(&kn, id).unwrap_or_default()),
        })),
    }
}
```

- [ ] **Step 2: `create_profile` — handle `title:`/`languages:` at file offset 0** (web.rs, the title + languages replace blocks ~445-461)

Add a small local helper just before the title/languages replaces, and use it for both:

```rust
        // Replace the whole `key:` line whether it is the first line (offset 0) or
        // mid-file (preceded by a newline). Returns false if the key is absent.
        fn replace_line(raw: &mut String, key: &str, new_line: &str) -> bool {
            let at = if raw.starts_with(key) {
                Some(0usize)
            } else {
                raw.find(&format!("\n{key}")).map(|s| s + 1)
            };
            if let Some(line_start) = at {
                let line_end = raw[line_start..].find('\n').map(|i| line_start + i).unwrap_or(raw.len());
                raw.replace_range(line_start..line_end, new_line);
                true
            } else {
                false
            }
        }
```

Then replace the existing title block with:
```rust
        if !np.title.is_empty() {
            replace_line(&mut raw, "title:", &format!("title: \"{}\"", np.title.replace('"', "'")));
        }
```
and the languages block with:
```rust
        if !np.languages.is_empty() {
            replace_line(&mut raw, "languages:", &format!("languages: [{}]", np.languages.join(", ")));
        }
```

- [ ] **Step 3: Drop the `CSS.escape`/`esc` coupling in `app.js` `docRow`** (the `isInherited` override toggle, ~708-718)

Replace the `if (isInherited) { ... }` block with a closure-held host reference (no class-name escaping at all):

```js
  if (isInherited) {
    let host = null;
    row.querySelector(".doc-override").onclick = () => {
      if (host) { host.remove(); host = null; return; } // toggle off
      host = h(`<div class="doc-override-host"></div>`);
      const form = kind === "convention" ? addConventionForm(profileId, meta.id) : addRecipeForm(profileId, meta.id);
      host.appendChild(form);
      row.insertAdjacentElement("afterend", host);
    };
  } else {
    row.querySelector(".doc-edit").onclick = () => editDoc(profileId, kind, meta.id, row);
  }
```

- [ ] **Step 4: Test the degrade** (web.rs tests, env-var pattern like `profile_json_exposes_extends_chain_and_provenance`)

```rust
    #[test]
    fn profile_json_degrades_on_broken_extends() {
        let kn = tempfile::tempdir().unwrap();
        // a profile that extends a non-existent base
        let d = kn.path().join("profiles").join("kid");
        std::fs::create_dir_all(d.join("conventions")).unwrap();
        std::fs::create_dir_all(d.join("recipes")).unwrap();
        std::fs::write(d.join("profile.yaml"),
            "id: kid\nextends: ghost\nfact_families:\n  - { id: symbol, symbol: true }\n").unwrap();
        std::fs::write(d.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
        std::fs::write(d.join("conventions/_index.json"), r#"{"topics":[]}"#).unwrap();
        std::fs::write(d.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();

        std::env::set_var("PALUGADA_KNOWLEDGE", kn.path());
        let v = profile_json("kid");
        std::env::remove_var("PALUGADA_KNOWLEDGE");

        let v = v.expect("profile_json must NOT error on a broken extends");
        assert_eq!(v["chain"][0], "kid");
        assert!(v["warning"].as_str().unwrap_or("").contains("ghost"), "warning names the bad base: {v}");
    }
```

- [ ] **Step 5: Verify + commit**

Run: `node --check src/web/app.js && cargo build 2>&1 | tail -2 && cargo test 2>&1 | tail -4` — node OK, warning-clean, all green (+1 new). NO cargo fmt.

```bash
git add src/web.rs src/web/app.js
git commit -m "harden(web): profile_json degrades on broken extends; offset-0 line replace; drop selector coupling"
```

---

## Self-Review

**Coverage:** all six deferred minors → Task 1 (I/O propagation [#1], render_merged tags [#2]); Task 2 (md⇔index parity [#3]); Task 3 (profile_json degrade [#4], offset-0 fallback [#5], CSS.escape/esc coupling [#6]).

**Placeholder scan:** complete Rust/JS in every step; exact commands + expected output. The one judgment point (Task 2 Step 3) is explicit: if a bundled profile fails the new parity check, STOP and report rather than weakening it.

**Type consistency:** `render_merged` gains a `tags: &str` param; its sole caller (`resolve_convention_raw`) is updated in the same task. `replace_line` is a local fn in `create_profile`. New validate Check name `"section id parity"` is referenced consistently in the tests. `profile_json` keeps returning `Result<serde_json::Value, String>` (now `Ok` on the degrade path).

**Risk:** Task 2's new check could fail a bundled profile if real md⇔index drift exists — Step 3 verifies all four and gates on it. Task 3's env-var test mutates global `PALUGADA_KNOWLEDGE` (same accepted pattern as the existing web test).
