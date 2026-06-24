# Profile Inheritance (`extends`) — Core CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an optional `extends: <profile-id>` field that lets a child profile live-inherit a parent's conventions and recipes — section-granular for conventions, whole-doc for recipes — resolved through one shared `src/inherit.rs` layer used by `q`, `for`, `s`, `brief`, and `validate`.

**Architecture:** A new `src/inherit.rs` owns the entire notion of an `extends` chain: it walks `child → parent → grandparent` (with cycle detection + depth limit), merges convention sections by anchor id, and produces merged convention/recipe views. Every existing reader in `knowledge.rs` reroutes through it; when a profile has no `extends`, its chain is `[self]` and behaviour is byte-identical to today. `brief.rs` needs no change because its two knowledge entrypoints (`convention_outline` via `effective::convention_outline_overlaid`, and `recipe_body`) become chain-aware internally.

**Tech Stack:** Rust 2021, `clap` derive, `serde`/`serde_yaml`/`serde_json` (all already deps), inline `#[cfg(test)]` tests with `tempfile`.

**Scope note:** This is **Plan A (core CLI)** from the spec `docs/superpowers/specs/2026-06-24-profile-inheritance-design.md`. The web-console integration (spec §5 web parts: `create_profile --extends`, merged `/api/profile` + per-section provenance, `app.js` form/Doc-Reader labels) is **Plan B**, written separately after this lands. This plan produces a fully working, tested CLI feature on its own.

## Global Constraints

- Language: **Rust 2021**; every fallible fn returns **`Result<T, String>`** (codebase convention), no `unwrap()`/`expect()`/`panic!` outside `#[cfg(test)]`.
- **No new dependencies** — `serde`, `serde_yaml`, `serde_json`, `tempfile` are already in `Cargo.toml`.
- Tests are **inline `#[cfg(test)] mod tests`** in the same file as the code (house style), using `tempfile::tempdir()`.
- `cargo build` and `cargo test` stay green with **no new warnings**; run `cargo fmt` before each commit.
- Manifest is **YAML** (`profile.yaml`); convention/recipe catalogs are **JSON** (`_index.json`). Do not change those formats.
- Inheritance covers **conventions + recipes only**. The manifest (`flows`/`review_map`/`fact_families`/`exec`) and `extractors.yaml` are **not** inherited.
- Section identity = the `{#anchor}` id (falls back to `slug(title)` when absent). Override order: spine = most-distant ancestor that defines the topic; descendants replace matching anchors in place and append new ones.
- `MAX_DEPTH = 8` for the `extends` chain.

---

### Task 1: Chain resolution + `extends` plumbing

**Files:**
- Create: `src/inherit.rs`
- Modify: `src/main.rs:7-20` (module declarations — add `mod inherit;`)
- Test: inline in `src/inherit.rs`

**Interfaces:**
- Produces: `pub const MAX_DEPTH: usize = 8;`
- Produces: `pub fn read_extends(kn: &std::path::Path, id: &str) -> Option<String>`
- Produces: `pub fn resolve_chain(kn: &std::path::Path, id: &str) -> Result<Vec<String>, String>` — ordered most-derived first: `[child, parent, grandparent, ...]`.

- [ ] **Step 1: Declare the module in `src/main.rs`**

Insert `mod inherit;` into the module list (keep alphabetical, after `mod indexer;`):

```rust
mod indexer;
mod inherit;
mod knowledge;
```

- [ ] **Step 2: Create `src/inherit.rs` with the chain resolver**

```rust
//! Profile inheritance (`extends`): resolve a child profile's `extends` chain
//! and merge inherited conventions/recipes. A profile with no `extends` has a
//! chain of `[self]`, so every resolver here is a no-op for flat profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Maximum length of an `extends` chain (cycle/runaway backstop).
pub const MAX_DEPTH: usize = 8;

/// Read just the `extends:` scalar from `<id>/profile.yaml` (None if absent,
/// empty, unreadable, or unparseable — a malformed parent simply ends the chain
/// and is surfaced separately by `profile validate`).
pub fn read_extends(kn: &Path, id: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct E {
        #[serde(default)]
        extends: Option<String>,
    }
    let p = kn.join("profiles").join(id).join("profile.yaml");
    let raw = std::fs::read_to_string(&p).ok()?;
    let e: E = serde_yaml::from_str(&raw).ok()?;
    e.extends.filter(|s| !s.is_empty())
}

/// The `extends` chain for `id`, most-derived first: `[id, parent, ...]`.
/// Errors on a missing profile in the chain, a cycle, or a chain deeper than
/// `MAX_DEPTH`.
pub fn resolve_chain(kn: &Path, id: &str) -> Result<Vec<String>, String> {
    let mut chain: Vec<String> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut cur = id.to_string();
    loop {
        if !seen.insert(cur.clone()) {
            let mut path = chain.clone();
            path.push(cur.clone());
            return Err(format!("inheritance cycle: {}", path.join(" \u{2192} ")));
        }
        if !kn.join("profiles").join(&cur).join("profile.yaml").is_file() {
            return match chain.last() {
                Some(child) => Err(format!("profile '{child}' extends '{cur}' which does not exist")),
                None => Err(format!(
                    "profile '{cur}' has no profile.yaml at {}",
                    kn.join("profiles").join(&cur).display()
                )),
            };
        }
        chain.push(cur.clone());
        if chain.len() > MAX_DEPTH {
            return Err(format!("inheritance chain too deep (> {MAX_DEPTH}) starting at '{id}'"));
        }
        match read_extends(kn, &cur) {
            Some(parent) => cur = parent,
            None => break,
        }
    }
    Ok(chain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Write a minimal `profile.yaml` for `id` with an optional `extends`.
    fn profile(kn: &Path, id: &str, extends: Option<&str>) {
        let dir = kn.join("profiles").join(id);
        std::fs::create_dir_all(&dir).unwrap();
        let mut y = format!("id: {id}\n");
        if let Some(e) = extends {
            y.push_str(&format!("extends: {e}\n"));
        }
        std::fs::write(dir.join("profile.yaml"), y).unwrap();
    }

    #[test]
    fn chain_for_flat_profile_is_self_only() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "rust-cli", None);
        assert_eq!(resolve_chain(kn.path(), "rust-cli").unwrap(), vec!["rust-cli".to_string()]);
        assert_eq!(read_extends(kn.path(), "rust-cli"), None);
    }

    #[test]
    fn chain_walks_parents_most_derived_first() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-base", None);
        profile(kn.path(), "android-mvvm", Some("android-base"));
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        assert_eq!(
            resolve_chain(kn.path(), "android-mvi").unwrap(),
            vec!["android-mvi".to_string(), "android-mvvm".to_string(), "android-base".to_string()]
        );
    }

    #[test]
    fn missing_parent_errors_with_child_name() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvi", Some("android-foo"));
        let err = resolve_chain(kn.path(), "android-mvi").unwrap_err();
        assert!(err.contains("android-mvi") && err.contains("android-foo"), "{err}");
    }

    #[test]
    fn cycle_is_detected() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "a", Some("b"));
        profile(kn.path(), "b", Some("a"));
        let err = resolve_chain(kn.path(), "a").unwrap_err();
        assert!(err.contains("cycle"), "{err}");
    }

    #[test]
    fn over_max_depth_errors() {
        let kn = tempfile::tempdir().unwrap();
        // p0 (root) <- p1 <- ... <- p9  => chain of 10 > MAX_DEPTH (8)
        profile(kn.path(), "p0", None);
        for i in 1..=9 {
            profile(kn.path(), &format!("p{i}"), Some(&format!("p{}", i - 1)));
        }
        let err = resolve_chain(kn.path(), "p9").unwrap_err();
        assert!(err.contains("too deep"), "{err}");
    }
}
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test --lib inherit::tests -- --nocapture`
Expected: 5 tests pass (`chain_for_flat_profile_is_self_only`, `chain_walks_parents_most_derived_first`, `missing_parent_errors_with_child_name`, `cycle_is_detected`, `over_max_depth_errors`).

- [ ] **Step 4: Build clean and commit**

Run: `cargo fmt && cargo build 2>&1 | tail -5`
Expected: builds with no warnings (an `unused` warning on `read_extends`/`resolve_chain` is acceptable until Task 2 consumes them — or add `#[allow(dead_code)]` temporarily; it will be removed when wired up).

```bash
git add src/inherit.rs src/main.rs
git commit -m "feat(inherit): resolve extends chain (cycle + depth guards)"
```

---

### Task 2: Anchor-aware section parser + merge core

**Files:**
- Modify: `src/inherit.rs` (add parser + merge helpers)
- Test: inline in `src/inherit.rs`

**Interfaces:**
- Consumes: `crate::knowledge::slug` (already `pub`).
- Produces: `pub struct MergedSection { pub anchor: String, pub title: String, pub body: String }`
- Produces: `pub struct ParsedConvention { pub preamble: String, pub sections: Vec<MergedSection> }`
- Produces: `pub fn parse_convention(body: &str) -> ParsedConvention`
- Produces: `pub fn parse_sections(body: &str) -> Vec<MergedSection>`
- Produces: `fn merge_section_lists(levels: &[Vec<MergedSection>]) -> Vec<MergedSection>` (ancestor→descendant order)

- [ ] **Step 1: Add the parser + merge code to `src/inherit.rs`** (above the `#[cfg(test)]` module)

```rust
/// One `## Heading {#anchor}` section of a convention body.
#[derive(Clone, Debug, PartialEq)]
pub struct MergedSection {
    /// The `{#anchor}` id; falls back to `slug(title)` when no explicit anchor.
    pub anchor: String,
    pub title: String,
    /// Body text after the heading, up to the next `## ` (trailing newline kept).
    pub body: String,
}

/// A convention body split into its leading preamble (everything before the
/// first `## ` — typically the `# H1` + any intro prose) and its sections.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedConvention {
    pub preamble: String,
    pub sections: Vec<MergedSection>,
}

/// Parse a (front-matter-stripped) convention body. Fence-aware: `## ` lines
/// inside ``` fences are body text, not headings.
pub fn parse_convention(body: &str) -> ParsedConvention {
    let mut preamble = String::new();
    let mut sections: Vec<MergedSection> = Vec::new();
    let mut cur: Option<MergedSection> = None;
    let mut in_fence = false;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
        }
        if !in_fence {
            if let Some(rest) = line.strip_prefix("## ") {
                if let Some(s) = cur.take() {
                    sections.push(s);
                }
                let title = rest.split("{#").next().unwrap_or(rest).trim().to_string();
                let anchor = rest
                    .find("{#")
                    .and_then(|i| rest[i + 2..].find('}').map(|j| rest[i + 2..i + 2 + j].trim().to_string()))
                    .filter(|a| !a.is_empty())
                    .unwrap_or_else(|| crate::knowledge::slug(&title));
                cur = Some(MergedSection { anchor, title, body: String::new() });
                continue;
            }
        }
        match cur.as_mut() {
            Some(s) => {
                s.body.push_str(line);
                s.body.push('\n');
            }
            None => {
                preamble.push_str(line);
                preamble.push('\n');
            }
        }
    }
    if let Some(s) = cur.take() {
        sections.push(s);
    }
    ParsedConvention { preamble, sections }
}

/// Just the sections of a convention body (anchor-aware).
pub fn parse_sections(body: &str) -> Vec<MergedSection> {
    parse_convention(body).sections
}

/// Merge per-section across chain levels given ancestor→descendant.
/// `levels[0]` is the most-distant ancestor that defines the topic, the last is
/// the most-derived. Spine = `levels[0]`'s order; later levels replace matching
/// anchors in place and append new anchors (in that level's file order).
fn merge_section_lists(levels: &[Vec<MergedSection>]) -> Vec<MergedSection> {
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, MergedSection> = BTreeMap::new();
    for lvl in levels {
        for s in lvl {
            if by_id.contains_key(&s.anchor) {
                by_id.insert(s.anchor.clone(), s.clone());
            } else {
                order.push(s.anchor.clone());
                by_id.insert(s.anchor.clone(), s.clone());
            }
        }
    }
    order.into_iter().filter_map(|id| by_id.remove(&id)).collect()
}
```

- [ ] **Step 2: Add tests to the `#[cfg(test)] mod tests` block in `src/inherit.rs`**

```rust
    #[test]
    fn parse_captures_anchor_or_slugs_title() {
        let body = "# Architecture\n> intro\n\n## UI State {#uistate}\nsealed state\n\n## Data Flow\nflow\n";
        let pc = parse_convention(body);
        assert!(pc.preamble.contains("> intro"));
        assert!(pc.preamble.contains("# Architecture"));
        assert_eq!(pc.sections.len(), 2);
        assert_eq!(pc.sections[0].anchor, "uistate"); // explicit {#uistate}
        assert_eq!(pc.sections[0].title, "UI State");
        assert_eq!(pc.sections[1].anchor, "data-flow"); // slug(title)
        assert!(pc.sections[0].body.contains("sealed state"));
    }

    #[test]
    fn parse_ignores_headings_in_fences() {
        let body = "## Real {#real}\ntext\n```\n## fake {#fake}\n```\nmore\n";
        let secs = parse_sections(body);
        assert_eq!(secs.len(), 1);
        assert_eq!(secs[0].anchor, "real");
        assert!(secs[0].body.contains("## fake"));
    }

    fn sec(anchor: &str, title: &str, body: &str) -> MergedSection {
        MergedSection { anchor: anchor.into(), title: title.into(), body: body.into() }
    }

    #[test]
    fn merge_overrides_in_place_and_appends_new() {
        // ancestor: layers, uistate, data-flow ; child: overrides data-flow, adds reducer
        let parent = vec![sec("layers", "Layers", "L"), sec("uistate", "UI State", "U"), sec("data-flow", "Data Flow", "live")];
        let child = vec![sec("data-flow", "Data Flow", "stateflow"), sec("reducer", "Reducer", "R")];
        let merged = merge_section_lists(&[parent, child]);
        let order: Vec<&str> = merged.iter().map(|s| s.anchor.as_str()).collect();
        assert_eq!(order, vec!["layers", "uistate", "data-flow", "reducer"]); // spine kept, new appended
        let df = merged.iter().find(|s| s.anchor == "data-flow").unwrap();
        assert_eq!(df.body, "stateflow"); // child wins in place
    }

    #[test]
    fn merge_three_levels_most_derived_wins() {
        let root = vec![sec("a", "A", "root-a")];
        let mid = vec![sec("a", "A", "mid-a"), sec("b", "B", "mid-b")];
        let child = vec![sec("b", "B", "child-b")];
        let merged = merge_section_lists(&[root, mid, child]);
        assert_eq!(merged.iter().find(|s| s.anchor == "a").unwrap().body, "mid-a");
        assert_eq!(merged.iter().find(|s| s.anchor == "b").unwrap().body, "child-b");
    }
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib inherit::tests -- --nocapture`
Expected: the 4 new tests pass alongside Task 1's.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/inherit.rs
git commit -m "feat(inherit): anchor-aware section parser + per-section merge"
```

---

### Task 3: Merged convention read + `q` inheritance + `#anchor` addressing

**Files:**
- Modify: `src/knowledge.rs` — make `strip_frontmatter`/`frontmatter_field` `pub`; reroute `query`, `convention_outline`, `list_topics`; add `Sel` enum; update `parse_topic_arg`.
- Modify: `src/inherit.rs` — add `resolve_convention_raw`.
- Test: inline in `src/inherit.rs` and `src/knowledge.rs`.

**Interfaces:**
- Consumes: `crate::knowledge::strip_frontmatter`, `crate::knowledge::frontmatter_field` (made `pub` here), `parse_convention`/`merge_section_lists` (Task 2).
- Produces: `pub fn crate::inherit::resolve_convention_raw(kn, profile, topic) -> Result<Option<String>, String>` — a synthetic, front-matter-bearing markdown string of the merged convention, or `None` if the topic is absent in the whole chain. A topic present at exactly one level is returned **verbatim** (zero behaviour change for flat profiles).

- [ ] **Step 1: Expose two helpers in `src/knowledge.rs`**

Change the two private fns to `pub` (signatures otherwise unchanged):

```rust
/// Return the markdown body with the leading YAML front-matter removed.
pub fn strip_frontmatter(raw: &str) -> &str {
```

```rust
/// Read a single scalar field out of the YAML front-matter (best-effort).
pub fn frontmatter_field(raw: &str, key: &str) -> Option<String> {
```

- [ ] **Step 2: Add `resolve_convention_raw` to `src/inherit.rs`** (above the test module)

```rust
/// Resolve a convention `topic` for `profile` across its `extends` chain into a
/// single synthetic markdown string (front-matter + `# H1`/preamble + merged
/// `## ` sections). A topic defined at exactly one level is returned verbatim.
pub fn resolve_convention_raw(kn: &Path, profile: &str, topic: &str) -> Result<Option<String>, String> {
    let chain = resolve_chain(kn, profile)?;
    // Collect the raw .md from each chain level that defines the topic, child first.
    let mut present: Vec<String> = Vec::new();
    for p in &chain {
        let md = kn.join("profiles").join(p).join("conventions").join(format!("{topic}.md"));
        if let Ok(raw) = std::fs::read_to_string(&md) {
            present.push(raw);
        }
    }
    if present.is_empty() {
        return Ok(None);
    }
    if present.len() == 1 {
        return Ok(Some(present.remove(0))); // verbatim — no merge, no regression
    }
    // Merge ancestor→descendant (present is child-first, so iterate reversed).
    let parsed: Vec<ParsedConvention> =
        present.iter().rev().map(|raw| parse_convention(crate::knowledge::strip_frontmatter(raw))).collect();
    let levels: Vec<Vec<MergedSection>> = parsed.iter().map(|p| p.sections.clone()).collect();
    let merged = merge_section_lists(&levels);
    let preamble = parsed.last().map(|p| p.preamble.clone()).unwrap_or_default();
    // Metadata from the most-derived (child-first => present[0]) definition.
    let child_raw = &present[0];
    let title = crate::knowledge::frontmatter_field(child_raw, "title").unwrap_or_default();
    let description = crate::knowledge::frontmatter_field(child_raw, "description").unwrap_or_default();
    Ok(Some(render_merged(topic, &title, &description, &preamble, &merged)))
}

/// Reassemble a merged convention into a front-matter-bearing markdown string.
fn render_merged(topic: &str, title: &str, description: &str, preamble: &str, secs: &[MergedSection]) -> String {
    let mut out = format!("---\nid: {topic}\ntitle: {title}\ndescription: {description}\n---\n\n");
    out.push_str(preamble.trim_end());
    out.push('\n');
    for s in secs {
        out.push_str(&format!("\n## {} {{#{}}}\n{}\n", s.title, s.anchor, s.body.trim_end()));
    }
    out
}
```

- [ ] **Step 3: Reroute `query`, `convention_outline`, `list_topics` and update `parse_topic_arg` in `src/knowledge.rs`**

Replace `parse_topic_arg` and the `query` fn with:

```rust
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
            let s = secs
                .get(n.saturating_sub(1))
                .ok_or_else(|| format!("section {n} not found in '{name}' (it has {})", secs.len()))?;
            println!("## {}\n\n{}", s.title, s.body.trim());
        }
        Some(Sel::Anchor(a)) => {
            let secs = crate::inherit::parse_sections(body);
            let s = secs.iter().find(|s| s.anchor == a).ok_or_else(|| {
                format!(
                    "section '#{a}' not found in '{name}' (sections: {})",
                    secs.iter().map(|s| s.anchor.as_str()).collect::<Vec<_>>().join(", ")
                )
            })?;
            println!("## {}\n\n{}", s.title, s.body.trim());
        }
        None => println!("{}", body.trim()),
    }
    Ok(())
}
```

Reroute `convention_outline` (so `brief` becomes chain-aware via `effective::convention_outline_overlaid`'s profile branch):

```rust
pub fn convention_outline(kn: &Path, profile: &str, name: &str) -> Result<String, String> {
    let raw = crate::inherit::resolve_convention_raw(kn, profile, name)?
        .ok_or_else(|| format!("no convention '{name}' in profile '{profile}'"))?;
    Ok(convention_outline_str(&raw, name))
}
```

Reroute `list_topics` to show the merged (inherited + own) set:

```rust
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
```

> Note: `list_topics` references `crate::inherit::merged_conventions`, added in Task 4. Implement Task 4 immediately after Task 3 (they compile together); if you build between them, temporarily keep the old `read_conv_index` body of `list_topics` and switch it in Task 4. The recommended order is to do Steps 1–2 + the `query`/`convention_outline`/`parse_topic_arg` edits here, then Task 4, then the `list_topics` swap.

- [ ] **Step 4: Add `resolve_convention_raw` tests to `src/inherit.rs` tests**

```rust
    /// Author a convention `<topic>.md` directly under a profile (no index needed
    /// for resolve_convention_raw, which reads the .md files).
    fn conv(kn: &Path, profile: &str, topic: &str, md: &str) {
        let dir = kn.join("profiles").join(profile).join("conventions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{topic}.md")), md).unwrap();
    }

    #[test]
    fn flat_profile_convention_returned_verbatim() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        let md = "---\nid: arch\ntitle: Arch\n---\n\n# Arch\n> intro\n\n## Layers {#layers}\nL\n";
        conv(kn.path(), "p", "arch", md);
        assert_eq!(resolve_convention_raw(kn.path(), "p", "arch").unwrap().as_deref(), Some(md));
    }

    #[test]
    fn child_overrides_one_section_inherits_rest() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvvm", None);
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        conv(kn.path(), "android-mvvm", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Layers {#layers}\nlayers body\n\n## Data Flow {#data-flow}\nLiveData wiring\n");
        conv(kn.path(), "android-mvi", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nStateFlow + reducer\n");
        let raw = resolve_convention_raw(kn.path(), "android-mvi", "architecture").unwrap().unwrap();
        assert!(raw.contains("## Layers {#layers}"), "inherited section kept: {raw}");
        assert!(raw.contains("layers body"));
        assert!(raw.contains("StateFlow + reducer"), "child override wins: {raw}");
        assert!(!raw.contains("LiveData wiring"), "parent's data-flow body replaced: {raw}");
        // order: Layers before Data Flow (spine preserved)
        assert!(raw.find("{#layers}").unwrap() < raw.find("{#data-flow}").unwrap());
    }

    #[test]
    fn inherited_only_topic_is_verbatim_from_parent() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "android-mvvm", None);
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        let md = "---\nid: testing\ntitle: Testing\n---\n\n# Testing\n## Unit {#unit}\nx\n";
        conv(kn.path(), "android-mvvm", "testing", md);
        assert_eq!(resolve_convention_raw(kn.path(), "android-mvi", "testing").unwrap().as_deref(), Some(md));
    }

    #[test]
    fn absent_topic_is_none() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "p", None);
        assert_eq!(resolve_convention_raw(kn.path(), "p", "nope").unwrap(), None);
    }
```

- [ ] **Step 5: Run inherit tests** (knowledge tests run after Task 4 wiring compiles)

Run: `cargo test --lib inherit:: -- --nocapture`
Expected: all inherit tests pass. (`cargo build` will fail until Task 4 adds `merged_conventions`; that's expected — proceed to Task 4 before the full build.)

- [ ] **Step 6: Commit** (after Task 4 makes it build; or commit Steps 1–2,3-edits and Task 4 together)

```bash
cargo fmt
git add src/knowledge.rs src/inherit.rs
git commit -m "feat(q): resolve conventions across the extends chain + #anchor addressing"
```

---

### Task 4: Merged indexes + `s` search + `for --list`

**Files:**
- Modify: `src/inherit.rs` — add `merged_conventions`, `merged_recipes`, `merge_section_metas`.
- Modify: `src/knowledge.rs` — reroute `search`, `list_recipes`.
- Test: inline in `src/inherit.rs`.

**Interfaces:**
- Consumes: `crate::knowledge::conventions`, `crate::knowledge::recipes` (existing, single-profile), `crate::knowledge::TopicMeta`, `crate::knowledge::RecipeMeta`, `crate::knowledge::SectionMeta`.
- Produces: `pub fn crate::inherit::merged_conventions(kn, profile) -> Result<Vec<TopicMeta>, String>` — union over the chain, child wins by id, sections merged by id.
- Produces: `pub fn crate::inherit::merged_recipes(kn, profile) -> Result<Vec<RecipeMeta>, String>` — union over the chain, child wins whole-recipe by id.

- [ ] **Step 1: Add merged-index helpers to `src/inherit.rs`**

```rust
use crate::knowledge::{RecipeMeta, SectionMeta, TopicMeta};

/// Merge two section-meta lists by id (descendant overrides in place, appends new).
fn merge_section_metas(base: &[SectionMeta], over: &[SectionMeta]) -> Vec<SectionMeta> {
    let mut order: Vec<String> = base.iter().map(|s| s.id.clone()).collect();
    let mut by: BTreeMap<String, SectionMeta> = base.iter().map(|s| (s.id.clone(), s.clone())).collect();
    for s in over {
        if !by.contains_key(&s.id) {
            order.push(s.id.clone());
        }
        by.insert(s.id.clone(), s.clone());
    }
    order.into_iter().filter_map(|id| by.remove(&id)).collect()
}

/// Merged convention catalog across the chain: union by topic id (child wins
/// metadata), with each topic's sections merged by section id.
pub fn merged_conventions(kn: &Path, profile: &str) -> Result<Vec<TopicMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, TopicMeta> = BTreeMap::new();
    for p in chain.iter().rev() {
        // root → child
        for t in crate::knowledge::conventions(kn, p)? {
            match by_id.get_mut(&t.id) {
                Some(existing) => {
                    existing.sections = merge_section_metas(&existing.sections, &t.sections);
                    existing.title = t.title;
                    existing.description = t.description;
                    existing.tags = t.tags;
                    existing.related = t.related;
                }
                None => {
                    order.push(t.id.clone());
                    by_id.insert(t.id.clone(), t);
                }
            }
        }
    }
    Ok(order.into_iter().filter_map(|id| by_id.remove(&id)).collect())
}

/// Merged recipe catalog across the chain: whole-recipe override by id.
pub fn merged_recipes(kn: &Path, profile: &str) -> Result<Vec<RecipeMeta>, String> {
    let chain = resolve_chain(kn, profile)?;
    let mut order: Vec<String> = Vec::new();
    let mut by_id: BTreeMap<String, RecipeMeta> = BTreeMap::new();
    for p in chain.iter().rev() {
        for r in crate::knowledge::recipes(kn, p)? {
            if !by_id.contains_key(&r.id) {
                order.push(r.id.clone());
            }
            by_id.insert(r.id.clone(), r);
        }
    }
    Ok(order.into_iter().filter_map(|id| by_id.remove(&id)).collect())
}
```

> `TopicMeta` does not derive `Clone`; this code never clones it (it moves owned values from `conventions(...)`). `SectionMeta` and `RecipeMeta` are fine (`SectionMeta` derives `Clone`; `RecipeMeta` is moved, not cloned).

- [ ] **Step 2: Reroute `search` and `list_recipes` in `src/knowledge.rs`**

```rust
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
```

```rust
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
            t.sections.iter().map(|s| s.title.as_str()).collect::<Vec<_>>().join(" ")
        )
        .to_lowercase();
        if hay.contains(&needle) {
            println!("[convention] {:<16} {}", t.id, t.description);
            hits += 1;
        }
    }
    for r in crate::inherit::merged_recipes(kn, profile).unwrap_or_default() {
        let hay = format!("{} {} {} {}", r.id, r.title, r.description, r.tags.join(" ")).to_lowercase();
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
```

> The now-unused private `read_conv_index`/`read_recipe_index` may trigger dead-code warnings if no other caller remains. `read_recipe_index` is still used by `recipes()`; `read_conv_index` was used by `list_topics`/`search`. If `cargo build` warns it is unused, delete `read_conv_index` (and its now-unused import paths) — `conventions()` uses `read_conv_index_in`, not `read_conv_index`.

- [ ] **Step 3: Add merged-index tests to `src/inherit.rs` tests**

```rust
    fn conv_indexed(kn: &Path, profile: &str, topic: &str, sections: &[(&str, &str)]) {
        // Writes <topic>.md AND a matching _index.json entry via knowledge writers.
        let dir = kn.join("profiles").join(profile).join("conventions");
        let specs: Vec<crate::knowledge::SectionSpec> = sections
            .iter()
            .map(|(_, title)| crate::knowledge::SectionSpec { title: (*title).into(), body: "b".into(), code: false })
            .collect();
        crate::knowledge::add_convention_in(
            &dir,
            &crate::knowledge::ConventionSpec {
                id: topic.into(),
                title: topic.into(),
                description: format!("{topic} desc"),
                tags: vec![],
                sections: specs,
            },
        )
        .unwrap();
    }

    #[test]
    fn merged_conventions_union_child_wins_and_merges_sections() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        conv_indexed(kn.path(), "base", "architecture", &[("layers", "Layers"), ("data-flow", "Data Flow")]);
        conv_indexed(kn.path(), "base", "testing", &[("unit", "Unit")]);
        conv_indexed(kn.path(), "child", "architecture", &[("data-flow", "Data Flow"), ("reducer", "Reducer")]);

        let merged = merged_conventions(kn.path(), "child").unwrap();
        let ids: Vec<&str> = merged.iter().map(|t| t.id.as_str()).collect();
        assert!(ids.contains(&"architecture") && ids.contains(&"testing"));
        let arch = merged.iter().find(|t| t.id == "architecture").unwrap();
        let secs: Vec<&str> = arch.sections.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(secs, vec!["layers", "data-flow", "reducer"]); // spine + appended override-set
    }

    #[test]
    fn merged_recipes_union_child_overrides_whole() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        let bdir = kn.path().join("profiles/base/recipes");
        let cdir = kn.path().join("profiles/child/recipes");
        crate::knowledge::add_recipe_from_markdown(&bdir, "---\nid: feature\ntitle: Base Feature\n---\n# F\nbase\n").unwrap();
        crate::knowledge::add_recipe_from_markdown(&bdir, "---\nid: refactor\ntitle: Refactor\n---\n# R\nr\n").unwrap();
        crate::knowledge::add_recipe_from_markdown(&cdir, "---\nid: feature\ntitle: Child Feature\n---\n# F\nchild\n").unwrap();

        let merged = merged_recipes(kn.path(), "child").unwrap();
        assert_eq!(merged.len(), 2);
        let feature = merged.iter().find(|r| r.id == "feature").unwrap();
        assert_eq!(feature.title, "Child Feature"); // child wins
    }
```

- [ ] **Step 4: Build, run all tests**

Run: `cargo fmt && cargo build 2>&1 | tail -5 && cargo test --lib 2>&1 | tail -25`
Expected: clean build (resolve any dead-code warning per Step 2's note); all `inherit`/`knowledge` tests pass.

- [ ] **Step 5: Commit** (folds in Task 3's pending edits)

```bash
git add src/inherit.rs src/knowledge.rs
git commit -m "feat(s/list): search & list across the merged extends chain"
```

---

### Task 5: Recipe inheritance for `for` / `brief`

**Files:**
- Modify: `src/inherit.rs` — add `resolve_recipe_raw`.
- Modify: `src/knowledge.rs` — reroute `recipe_body`.
- Test: inline in `src/inherit.rs`.

**Interfaces:**
- Produces: `pub fn crate::inherit::resolve_recipe_raw(kn, profile, task) -> Result<Option<String>, String>` — raw `.md` of the nearest chain level (child first) that defines `task`, else `None`.

- [ ] **Step 1: Add `resolve_recipe_raw` to `src/inherit.rs`**

```rust
/// Resolve a recipe's raw markdown across the chain: the nearest level (child
/// first) that defines `<task>.md` wins whole. `None` if absent everywhere.
pub fn resolve_recipe_raw(kn: &Path, profile: &str, task: &str) -> Result<Option<String>, String> {
    let chain = resolve_chain(kn, profile)?;
    for p in &chain {
        let path = kn.join("profiles").join(p).join("recipes").join(format!("{task}.md"));
        if let Ok(raw) = std::fs::read_to_string(&path) {
            return Ok(Some(raw));
        }
    }
    Ok(None)
}
```

- [ ] **Step 2: Reroute `recipe_body` in `src/knowledge.rs`**

```rust
pub fn recipe_body(kn: &Path, profile: &str, task: &str) -> Result<String, String> {
    let raw = crate::inherit::resolve_recipe_raw(kn, profile, task)?
        .ok_or_else(|| format!("no recipe '{task}' in profile '{profile}' or its parents"))?;
    Ok(strip_frontmatter(&raw).trim().to_string())
}
```

> `recipe()` (the `for` CLI) and `brief.rs`'s recipe step both call `recipe_body`, so both become chain-aware with no further edits.

- [ ] **Step 3: Add tests to `src/inherit.rs`**

```rust
    #[test]
    fn recipe_inherited_then_overridden() {
        let kn = tempfile::tempdir().unwrap();
        profile(kn.path(), "base", None);
        profile(kn.path(), "child", Some("base"));
        let bdir = kn.path().join("profiles/base/recipes");
        std::fs::create_dir_all(&bdir).unwrap();
        std::fs::write(bdir.join("feature.md"), "---\nid: feature\n---\n# F\nbase steps\n").unwrap();
        // inherited
        assert!(resolve_recipe_raw(kn.path(), "child", "feature").unwrap().unwrap().contains("base steps"));
        // overridden by child
        let cdir = kn.path().join("profiles/child/recipes");
        std::fs::create_dir_all(&cdir).unwrap();
        std::fs::write(cdir.join("feature.md"), "---\nid: feature\n---\n# F\nchild steps\n").unwrap();
        assert!(resolve_recipe_raw(kn.path(), "child", "feature").unwrap().unwrap().contains("child steps"));
        // absent
        assert_eq!(resolve_recipe_raw(kn.path(), "child", "nope").unwrap(), None);
    }
```

- [ ] **Step 4: Run tests + commit**

Run: `cargo fmt && cargo test --lib inherit:: -- --nocapture`
Expected: pass.

```bash
git add src/inherit.rs src/knowledge.rs
git commit -m "feat(for): resolve recipes across the extends chain"
```

---

### Task 6: `validate()` chain-aware

**Files:**
- Modify: `src/profile.rs` — `web_render_checks` uses merged conventions/recipes + a chain check + a relaxed on-disk file check.
- Test: inline in `src/profile.rs`.

**Interfaces:**
- Consumes: `crate::inherit::resolve_chain`, `crate::inherit::merged_conventions`, `crate::inherit::merged_recipes`, `crate::knowledge::conventions`, `crate::knowledge::recipes`.

- [ ] **Step 1: Rewrite `web_render_checks` in `src/profile.rs`** (replace the whole fn)

```rust
fn web_render_checks(kn: &Path, id: &str) -> Vec<Check> {
    let dir = kn.join("profiles").join(id);

    // 0. extends chain resolves (cycle / depth / missing parent).
    let chain = match crate::inherit::resolve_chain(kn, id) {
        Ok(c) => c,
        Err(e) => return vec![Check { name: "extends chain".into(), ok: false, detail: e }],
    };
    let chain_detail = if chain.len() > 1 {
        format!("extends: {}", chain[1..].join(" \u{2192} "))
    } else {
        "no extends".into()
    };

    // Merged (inherited + own) sets — cross-refs resolve across the chain.
    let topics = match crate::inherit::merged_conventions(kn, id) {
        Ok(t) => t,
        Err(e) => return vec![Check { name: "conventions render-shape".into(), ok: false, detail: e }],
    };
    let recipes = match crate::inherit::merged_recipes(kn, id) {
        Ok(r) => r,
        Err(e) => return vec![Check { name: "recipes render-shape".into(), ok: false, detail: e }],
    };
    let topic_ids: BTreeSet<&str> = topics.iter().map(|t| t.id.as_str()).collect();
    let recipe_ids: BTreeSet<&str> = recipes.iter().map(|r| r.id.as_str()).collect();
    let mut out: Vec<Check> = Vec::new();

    // 1. render-shape: every topic and section has a non-empty id + title.
    let mut shape: Result<String, String> =
        Ok(format!("{} topics, {} recipes render-ready", topics.len(), recipes.len()));
    'shape: for t in &topics {
        if t.id.trim().is_empty() {
            shape = Err("a topic has an empty id".into());
            break;
        }
        if t.title.trim().is_empty() {
            shape = Err(format!("topic '{}' has an empty title", t.id));
            break;
        }
        for s in &t.sections {
            if s.id.trim().is_empty() {
                shape = Err(format!("topic '{}' has a section with an empty id", t.id));
                break 'shape;
            }
            if s.title.trim().is_empty() {
                shape = Err(format!("topic '{}' section '{}' has an empty title", t.id, s.id));
                break 'shape;
            }
        }
    }
    out.push(check("conventions render-shape", shape));

    // 2. recipe convention_refs resolve to a real topic and (if given) section.
    let mut refs: Result<String, String> =
        Ok(format!("{} recipes: all convention_refs resolve", recipes.len()));
    'refs: for r in &recipes {
        for cr in &r.convention_refs {
            if !topic_ids.contains(cr.topic.as_str()) {
                refs = Err(format!("recipe '{}' references unknown convention '{}'", r.id, cr.topic));
                break 'refs;
            }
            if !cr.section.trim().is_empty() {
                let section_ok = topics
                    .iter()
                    .find(|t| t.id == cr.topic)
                    .map(|t| t.sections.iter().any(|s| s.id == cr.section))
                    .unwrap_or(false);
                if !section_ok {
                    refs = Err(format!(
                        "recipe '{}' references '{}#{}' but that section does not exist",
                        r.id, cr.topic, cr.section
                    ));
                    break 'refs;
                }
            }
        }
    }
    out.push(check("recipe cross-refs resolve", refs));

    // 3. `related` (conventions) and `related_recipes` ids resolve (against merged sets).
    let mut rel: Result<String, String> = Ok("all related ids resolve".into());
    'rel: {
        for t in &topics {
            for rid in &t.related {
                if !topic_ids.contains(rid.as_str()) {
                    rel = Err(format!("convention '{}' lists related '{}' which is not a convention", t.id, rid));
                    break 'rel;
                }
            }
        }
        for r in &recipes {
            for rid in &r.related_recipes {
                if !recipe_ids.contains(rid.as_str()) {
                    rel = Err(format!("recipe '{}' lists related_recipes '{}' which is not a recipe", r.id, rid));
                    break 'rel;
                }
            }
        }
    }
    out.push(check("related ids resolve", rel));

    // 4. every LOCALLY-declared topic/recipe has its `<id>.md` on disk in THIS
    //    profile. Inherited-only docs need not exist locally.
    let local_topics = crate::knowledge::conventions(kn, id).unwrap_or_default();
    let local_recipes = crate::knowledge::recipes(kn, id).unwrap_or_default();
    let mut files: Result<String, String> = Ok("all local convention/recipe files present".into());
    'files: {
        for t in &local_topics {
            if !dir.join("conventions").join(format!("{}.md", t.id)).exists() {
                files = Err(format!("convention '{}' has no conventions/{}.md", t.id, t.id));
                break 'files;
            }
        }
        for r in &local_recipes {
            if !dir.join("recipes").join(format!("{}.md", r.id)).exists() {
                files = Err(format!("recipe '{}' has no recipes/{}.md", r.id, r.id));
                break 'files;
            }
        }
    }
    out.push(check("doc files present", files));

    out.push(Check { name: "extends chain".into(), ok: true, detail: chain_detail });
    out
}
```

- [ ] **Step 2: Add tests to `src/profile.rs` tests**

```rust
    /// Minimal valid profile dir (profile.yaml + extractors.yaml + empty indexes).
    fn base_profile(kn: &Path, id: &str, extends: Option<&str>) {
        let dir = kn.join("profiles").join(id);
        fs::create_dir_all(dir.join("conventions")).unwrap();
        fs::create_dir_all(dir.join("recipes")).unwrap();
        let mut y = format!("id: {id}\nfact_families:\n  - {{ id: symbol, symbol: true }}\n");
        if let Some(e) = extends {
            y.push_str(&format!("extends: {e}\n"));
        }
        fs::write(dir.join("profile.yaml"), y).unwrap();
        fs::write(dir.join("extractors.yaml"), "families:\n  - id: symbol\n    regex: 'x'\n").unwrap();
        fs::write(dir.join("conventions/_index.json"), r#"{"topics":[]}"#).unwrap();
        fs::write(dir.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();
    }

    #[test]
    fn validate_child_resolves_inherited_cross_refs() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "parent", None);
        // parent owns `architecture` with section `data-flow`
        fs::write(
            kn.path().join("profiles/parent/conventions/_index.json"),
            r#"{"topics":[{"id":"architecture","title":"Arch","sections":[{"id":"data-flow","title":"Data Flow","tokens":10}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/parent/conventions/architecture.md"),
            "# Arch\n## Data Flow {#data-flow}\nx\n").unwrap();

        base_profile(kn.path(), "child", Some("parent"));
        // child owns ONLY a recipe that references the INHERITED architecture#data-flow
        fs::write(
            kn.path().join("profiles/child/recipes/_index.json"),
            r#"{"recipes":[{"id":"feature","title":"Feature","convention_refs":[{"topic":"architecture","section":"data-flow"}]}]}"#,
        ).unwrap();
        fs::write(kn.path().join("profiles/child/recipes/feature.md"), "# Feature\n").unwrap();

        let checks = validate(kn.path(), "child");
        for c in &checks {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
    }

    #[test]
    fn validate_fails_on_cycle() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "a", Some("b"));
        base_profile(kn.path(), "b", Some("a"));
        let checks = validate(kn.path(), "a");
        let c = checks.iter().find(|c| c.name == "extends chain").unwrap();
        assert!(!c.ok && c.detail.contains("cycle"), "{}", c.detail);
    }

    #[test]
    fn validate_fails_on_missing_parent() {
        let kn = tempfile::tempdir().unwrap();
        base_profile(kn.path(), "child", Some("ghost"));
        let checks = validate(kn.path(), "child");
        let c = checks.iter().find(|c| c.name == "extends chain").unwrap();
        assert!(!c.ok && c.detail.contains("ghost"), "{}", c.detail);
    }
```

- [ ] **Step 3: Run profile tests**

Run: `cargo test --lib profile::tests -- --nocapture`
Expected: existing tests still pass (note: `validate_flags_dangling_recipe_section_ref` and `new_then_validate_round_trips` must still pass — the merged path is a superset of the single-profile path for flat profiles); the 3 new tests pass.

> If `new_then_validate_round_trips` now fails because `scaffold_new`'s signature is unchanged here, that's fine — Task 7 changes the signature and that test together. If you run profile tests before Task 7, they pass as-is.

- [ ] **Step 4: Commit**

```bash
cargo fmt
git add src/profile.rs
git commit -m "feat(validate): resolve refs across the extends chain; relax local-file rule"
```

---

### Task 7: `profile new --extends` (manifest + extractors copy-seed)

**Files:**
- Modify: `src/profile.rs` — `scaffold_new` gains `extends: Option<&str>`; add `reid_with_extends`.
- Modify: `src/main.rs:297-298` (`ProfileCmd::New`) + `src/main.rs:998-1006` (the `New` arm) + `src/main.rs:1048` area is unaffected.
- Modify: `src/web.rs` — update the `scaffold_new` call site to pass `None` (web `--extends` is Plan B).
- Test: inline in `src/profile.rs`.

**Interfaces:**
- Produces: `pub fn scaffold_new(kn: &Path, id: &str, extends: Option<&str>) -> Result<Vec<PathBuf>, String>`

- [ ] **Step 1: Rewrite `scaffold_new` + add `reid_with_extends` in `src/profile.rs`**

```rust
/// Rewrite a parent's `profile.yaml` text for a child: set `id:` to the child,
/// insert `extends: <parent>` right after it, and retitle. Other lines (flows,
/// review_map, fact_families, exec) are copied verbatim.
fn reid_with_extends(parent_yaml: &str, id: &str, parent: &str) -> String {
    let mut out = String::new();
    let mut did_id = false;
    for line in parent_yaml.lines() {
        let trimmed = line.trim_start();
        if !did_id && trimmed.starts_with("id:") {
            out.push_str(&format!("id: {id}\n"));
            out.push_str(&format!("extends: {parent}\n"));
            did_id = true;
            continue;
        }
        if trimmed.starts_with("title:") {
            out.push_str(&format!("title: \"{id} profile\"\n"));
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !did_id {
        out = format!("id: {id}\nextends: {parent}\n{out}");
    }
    out
}

/// Scaffold a profile under `kn/profiles/<id>/`. With `extends`, the manifest
/// (profile.yaml minus knowledge) and `extractors.yaml` (+ `extractors/`) are
/// copy-seeded from the parent so the child is immediately valid; conventions
/// and recipes are left empty (they are live-inherited). Refuses if `id` exists.
pub fn scaffold_new(kn: &Path, id: &str, extends: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let ok_id = !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !ok_id {
        return Err(format!("invalid profile id '{id}' — use only [a-z0-9_-]"));
    }
    let dir = kn.join("profiles").join(id);
    if dir.exists() {
        return Err(format!("profile '{id}' already exists at {}", dir.display()));
    }

    let default_profile = format!(
        "schema_version: \"1.0\"\nid: {id}\ntitle: \"{id} profile\"\nlanguages: []\n\nfact_families:\n  - {{ id: symbol, symbol: true }}\n\nflows:\n  bugfix:   [code.recent, symbol.find]\n  feature:  [recipe(feature)]\n  refactor: [convention(architecture)]\n  review:   [diff.scan, convention(by-file-kind)]\n\nreview_map:\n  symbol: [architecture]\n"
    );
    let default_extractors =
        "schema_version: \"1.0\"\nignore_dirs: [\".git\", \".palugada\", \"target\", \"node_modules\", \"build\"]\n\nfamilies:\n  - id: symbol\n    regex: 'class\\s+(?P<name>\\w+)'\n".to_string();

    fs::create_dir_all(dir.join("conventions")).map_err(|e| format!("create dirs: {e}"))?;
    fs::create_dir_all(dir.join("recipes")).map_err(|e| format!("create dirs: {e}"))?;

    let mut written: Vec<PathBuf> = Vec::new();

    let (profile_yaml, extractors_yaml) = match extends {
        Some(parent) => {
            let pdir = kn.join("profiles").join(parent);
            if !pdir.join("profile.yaml").is_file() {
                return Err(format!("base profile '{parent}' does not exist at {}", pdir.display()));
            }
            let praw = fs::read_to_string(pdir.join("profile.yaml"))
                .map_err(|e| format!("read parent profile.yaml: {e}"))?;
            let child_yaml = reid_with_extends(&praw, id, parent);
            let pextr = fs::read_to_string(pdir.join("extractors.yaml")).unwrap_or(default_extractors);
            // copy parent extractors/*.scm if present
            let scm_dir = pdir.join("extractors");
            if scm_dir.is_dir() {
                fs::create_dir_all(dir.join("extractors")).map_err(|e| format!("create extractors dir: {e}"))?;
                for e in fs::read_dir(&scm_dir).map_err(|e| format!("read parent extractors/: {e}"))? {
                    let e = e.map_err(|e| e.to_string())?;
                    let from = e.path();
                    if from.is_file() {
                        let to = dir.join("extractors").join(e.file_name());
                        fs::copy(&from, &to).map_err(|e| format!("copy scm: {e}"))?;
                        written.push(to);
                    }
                }
            }
            (child_yaml, pextr)
        }
        None => (default_profile, default_extractors),
    };

    let conv_index = "{\n  \"schema_version\": \"1.0\",\n  \"topics\": []\n}\n";
    let recipe_index = "{\n  \"schema_version\": \"1.0\",\n  \"recipes\": []\n}\n";
    let files = [
        (dir.join("profile.yaml"), profile_yaml),
        (dir.join("extractors.yaml"), extractors_yaml),
        (dir.join("conventions").join("_index.json"), conv_index.to_string()),
        (dir.join("recipes").join("_index.json"), recipe_index.to_string()),
    ];
    for (path, contents) in files {
        fs::write(&path, &contents).map_err(|e| format!("write {}: {e}", path.display()))?;
        written.push(path);
    }
    written.sort();
    Ok(written)
}
```

- [ ] **Step 2: Add `--extends` to the CLI in `src/main.rs`**

Update the `ProfileCmd::New` variant (lines 297-298):

```rust
    /// Scaffold a new profile from a minimal template: `profile new <id>`.
    New {
        id: String,
        /// Inherit conventions/recipes from a base profile: `--extends <id>`.
        #[arg(long)]
        extends: Option<String>,
    },
```

Update the `ProfileCmd::New` match arm in `cmd_profile` (around line 998):

```rust
        ProfileCmd::New { id, extends } => {
            let written = profile::scaffold_new(&kn, &id, extends.as_deref())?;
            println!("scaffolded profile '{id}':");
            for p in written {
                println!("  {}", p.display());
            }
            if let Some(parent) = &extends {
                println!("inherits conventions/recipes from '{parent}' (author only what differs)");
            }
            println!("validate with:  palugada profile validate {id}");
            Ok(())
        }
```

- [ ] **Step 3: Fix the other `scaffold_new` call sites**

In `src/web.rs` (the `create_profile` handler, ~line 399-427) find the `profile::scaffold_new(...)` call and add the `None` argument:

```rust
    profile::scaffold_new(&kn, &id, None)?;
```

In `src/profile.rs` tests, update `new_then_validate_round_trips`:

```rust
        scaffold_new(kn.path(), "fresh", None).unwrap();
```
and
```rust
        assert!(scaffold_new(kn.path(), "fresh", None).is_err());
```

- [ ] **Step 4: Add scaffold tests to `src/profile.rs` tests**

```rust
    #[test]
    fn scaffold_with_extends_seeds_manifest_and_inherits_knowledge() {
        let kn = tempfile::tempdir().unwrap();
        // parent with a real manifest + extractors + a convention
        let p = kn.path().join("profiles").join("android-mvvm");
        fs::create_dir_all(p.join("conventions")).unwrap();
        fs::create_dir_all(p.join("recipes")).unwrap();
        fs::write(p.join("profile.yaml"),
            "schema_version: \"1.0\"\nid: android-mvvm\ntitle: \"MVVM\"\nlanguages: [kotlin]\nfact_families:\n  - { id: viewmodel, symbol: true }\nflows:\n  feature: [recipe(feature)]\nreview_map:\n  viewmodel: [architecture]\n").unwrap();
        fs::write(p.join("extractors.yaml"), "families:\n  - id: viewmodel\n    regex: 'x'\n").unwrap();
        fs::write(p.join("conventions/_index.json"),
            r#"{"topics":[{"id":"architecture","title":"Arch","sections":[{"id":"layers","title":"Layers","tokens":10}]}]}"#).unwrap();
        fs::write(p.join("conventions/architecture.md"), "# Arch\n## Layers {#layers}\nx\n").unwrap();
        fs::write(p.join("recipes/_index.json"), r#"{"recipes":[]}"#).unwrap();

        scaffold_new(kn.path(), "android-mvi", Some("android-mvvm")).unwrap();

        let child_yaml = fs::read_to_string(kn.path().join("profiles/android-mvi/profile.yaml")).unwrap();
        assert!(child_yaml.contains("id: android-mvi"));
        assert!(child_yaml.contains("extends: android-mvvm"));
        assert!(child_yaml.contains("viewmodel"), "manifest fact_families copy-seeded");
        // knowledge left empty (live inheritance)
        let conv_idx = fs::read_to_string(kn.path().join("profiles/android-mvi/conventions/_index.json")).unwrap();
        assert!(conv_idx.contains("\"topics\": []"));
        // child validates AND sees the inherited `architecture` topic via merge
        for c in validate(kn.path(), "android-mvi") {
            assert!(c.ok, "check '{}' failed: {}", c.name, c.detail);
        }
        let merged = crate::inherit::merged_conventions(kn.path(), "android-mvi").unwrap();
        assert!(merged.iter().any(|t| t.id == "architecture"), "inherited topic visible");
    }

    #[test]
    fn scaffold_with_missing_base_errors() {
        let kn = tempfile::tempdir().unwrap();
        assert!(scaffold_new(kn.path(), "child", Some("ghost")).is_err());
    }
```

- [ ] **Step 5: Build, run all tests**

Run: `cargo fmt && cargo build 2>&1 | tail -5 && cargo test --lib 2>&1 | tail -25`
Expected: clean build; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/profile.rs src/main.rs src/web.rs
git commit -m "feat(profile): profile new --extends seeds manifest, inherits knowledge"
```

---

### Task 8: End-to-end integration test (synthetic 3-level chain) + manual `brief` check

**Files:**
- Modify: `src/inherit.rs` — one integration test exercising the full stack via the public resolvers.
- Test: inline in `src/inherit.rs`.

**Interfaces:** consumes everything produced above.

- [ ] **Step 1: Add the end-to-end test to `src/inherit.rs` tests**

```rust
    #[test]
    fn end_to_end_three_level_mvvm_to_mvi() {
        let kn = tempfile::tempdir().unwrap();
        // android-base: architecture (layers) + testing
        profile(kn.path(), "android-base", None);
        conv(kn.path(), "android-base", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Layers {#layers}\nlayer rules\n");
        conv(kn.path(), "android-base", "testing",
            "---\nid: testing\ntitle: Testing\n---\n\n# Testing\n\n## Unit {#unit}\nunit rules\n");
        // android-mvvm extends base: adds data-flow (LiveData)
        profile(kn.path(), "android-mvvm", Some("android-base"));
        conv(kn.path(), "android-mvvm", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nLiveData wiring\n");
        // android-mvi extends mvvm: overrides data-flow (StateFlow)
        profile(kn.path(), "android-mvi", Some("android-mvvm"));
        conv(kn.path(), "android-mvi", "architecture",
            "---\nid: architecture\ntitle: Architecture\n---\n\n# Architecture\n\n## Data Flow {#data-flow}\nStateFlow + reducer\n");

        // chain
        assert_eq!(
            resolve_chain(kn.path(), "android-mvi").unwrap(),
            vec!["android-mvi".to_string(), "android-mvvm".to_string(), "android-base".to_string()]
        );
        // merged architecture: layers (grandparent) + data-flow (child override)
        let arch = resolve_convention_raw(kn.path(), "android-mvi", "architecture").unwrap().unwrap();
        assert!(arch.contains("## Layers {#layers}") && arch.contains("layer rules"));
        assert!(arch.contains("StateFlow + reducer"));
        assert!(!arch.contains("LiveData wiring"));
        assert!(arch.find("{#layers}").unwrap() < arch.find("{#data-flow}").unwrap());
        // testing inherited verbatim from grandparent
        let testing = resolve_convention_raw(kn.path(), "android-mvi", "testing").unwrap().unwrap();
        assert!(testing.contains("unit rules"));
    }
```

- [ ] **Step 2: Run the full suite**

Run: `cargo test --lib 2>&1 | tail -20`
Expected: all tests pass (`end_to_end_three_level_mvvm_to_mvi` included).

- [ ] **Step 3: Manual `q`/`for`/`brief` smoke test against a temp chain**

Run (creates a throwaway chain off an existing profile, then cleans up):

```bash
cargo build 2>&1 | tail -2
KP="$(pwd)/knowledge"
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada profile new mvi-smoke --extends android-mvvm
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada profile validate mvi-smoke
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada --profile mvi-smoke q architecture        # inherited from android-mvvm
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada --profile mvi-smoke q architecture#layers  # by-anchor (if android-mvvm has it)
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada --profile mvi-smoke for feature             # inherited recipe
PALUGADA_KNOWLEDGE="$KP" ./target/debug/palugada --profile mvi-smoke q --list                # shows inherited topics
rm -rf "$KP/profiles/mvi-smoke"
```

Expected: `validate` prints `profile 'mvi-smoke' OK` with an `extends: android-mvvm` line on the `extends chain` check; `q architecture` prints android-mvvm's architecture; `for feature` prints android-mvvm's feature recipe; `q --list` lists the inherited conventions. (Adjust `q architecture#<anchor>` to a real anchor from android-mvvm's `architecture.md` `_index.json`.) Confirm the `mvi-smoke` dir is removed afterward.

- [ ] **Step 4: Commit**

```bash
git add src/inherit.rs
git commit -m "test(inherit): end-to-end MVVM→MVI inheritance integration test"
```

---

## Self-Review

**1. Spec coverage** (against `2026-06-24-profile-inheritance-design.md`):

| Spec section | Task(s) |
|---|---|
| §1 manifest `extends` + `read_extends` | Task 1 |
| §2 `resolve_chain` (cycle/depth) | Task 1 |
| §2 section-merge algorithm + recipe override | Tasks 2, 4, 5 |
| §2 `q topic#section-id` addressing | Task 3 |
| §3 `q` honors inheritance | Task 3 |
| §3 `for` honors inheritance | Task 5 |
| §3 `s` over merged index | Task 4 |
| §3 `brief` (via `convention_outline`/`recipe_body`) | Tasks 3, 5 (no `brief.rs`/`effective.rs` edit; verified in Task 8 Step 3) |
| §4 `validate` chain-aware + relaxed file rule | Task 6 |
| §5 `profile new --extends` (manifest copy-seed, knowledge empty) | Task 7 |
| §6 errors (missing parent / cycle / depth / absent topic) | Tasks 1, 3, 6 |
| §7 backward-compat (flat = `[self]`, verbatim) | Tasks 1, 3 (verbatim single-level), full suite Task 8 |
| §5 web console | **Plan B (out of scope here)** — explicitly deferred |

No core-CLI requirement is left without a task. Web-console requirements are deferred to Plan B by design.

**2. Placeholder scan:** No `TBD`/`TODO`/"handle edge cases"/"similar to Task N". Every code step shows complete code; every run step shows the command + expected result.

**3. Type consistency:** `resolve_chain`, `read_extends`, `resolve_convention_raw`, `resolve_recipe_raw`, `merged_conventions`, `merged_recipes`, `parse_sections`, `parse_convention`, `MergedSection`, `ParsedConvention` are named identically across producing and consuming tasks. `scaffold_new(kn, id, extends)` — the new 3-arg signature is updated at all three call sites (main.rs, web.rs, profile.rs test) in Task 7. `Sel` is private to `knowledge.rs` (only `query` uses it). `strip_frontmatter`/`frontmatter_field` made `pub` in Task 3 before `inherit.rs` consumes them.

**Ordering caveat (locked):** Task 3 and Task 4 must land together for a clean build (Task 3's `list_topics` references Task 4's `merged_conventions`). The plan flags this in Task 3 Step 3 and commits them in Task 4 Step 5.
