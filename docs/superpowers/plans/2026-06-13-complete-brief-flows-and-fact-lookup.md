# Complete `brief` flows + generic `fact` lookup — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the four stubbed `brief` steps (`prd.context`, `module.info`, `diff.scan`, `convention(by-file-kind)`), replace the naive budget with priority-fill + truncation, and add a stack-agnostic `fact <family> [name]` command.

**Architecture:** Pure, unit-testable helpers do the work (`families_for_path`, `budget_packs`, `truncate_to_tokens`, `module_report`, `fact_report`, `mapped_topics`, `format_issue_pack`); `brief::run` and `cmd_*` are thin IO shells that call them. `prd.context` is the only networked step — its `IssueTracker` is built lazily and every failure degrades to an inline `(…)` note so the pack never aborts.

**Tech Stack:** Rust, `clap` (derive), `serde`/`serde_yaml`/`serde_json`, `regex`, `walkdir`, `std::process::Command` (git), `tempfile` (dev). No new dependencies.

**Reference spec:** `docs/superpowers/specs/2026-06-13-complete-brief-flows-and-fact-lookup-design.md`

**Test command (whole crate):** `cargo test` · **Single test:** `cargo test <name> -- --nocapture` · **Build:** `cargo build`

---

## File structure

| File | Responsibility | Change |
|---|---|---|
| `src/indexer.rs` | code scanning + index reads | extract `load_families`/`family_matches`/`families_for_path`; add `module_report`, `fact_report`, `fact_families` |
| `src/brief.rs` | flow context-pack assembly | `BriefContext`, priority-fill budget, four step handlers, `review_map` parse, connector plumbing |
| `src/main.rs` | clap dispatch + command shells | `Fact` subcommand + `cmd_fact`; `cmd_brief` resolves connectors + passes `insecure` |
| `knowledge/profiles/android-mvvm/profile.yaml` | bundled profile data | add `review_map` |
| `README.md` | docs | document `fact`; update roadmap line |

---

## Task 1: Extract reusable family-matching in the indexer

**Files:**
- Modify: `src/indexer.rs` (struct `CompiledFamily` + `run` body, ~`src/indexer.rs:49-145`)
- Test: `src/indexer.rs` (`#[cfg(test)]` module, ~`src/indexer.rs:248`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/indexer.rs`:

```rust
    #[test]
    fn families_for_path_matches_by_ext_and_path() {
        let cfg: Extractors = serde_yaml::from_str(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n  - id: i18n\n    ext: [xml]\n    path_contains: values\n    regex: '<string\\s+name=\"(?P<name>[^\"]+)\"'\n",
        ).unwrap();
        let fams = compile_families(&cfg).unwrap();
        assert_eq!(families_for_path("app/Login.kt", "kt", &fams), vec!["viewmodel".to_string()]);
        assert_eq!(families_for_path("app/values/strings.xml", "xml", &fams), vec!["i18n".to_string()]);
        // xml outside a values/ dir does not match i18n
        assert!(families_for_path("app/other/x.xml", "xml", &fams).is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test families_for_path_matches_by_ext_and_path`
Expected: FAIL — `compile_families` and `families_for_path` are not defined.

- [ ] **Step 3: Extract the helpers**

In `src/indexer.rs`, make `CompiledFamily` fields `pub` and add two public helpers + a private predicate. Replace the inline compile loop (currently `src/indexer.rs:68-88`) and the inline `applicable` filter (currently `src/indexer.rs:111-117`) to use them.

Add near `CompiledFamily`:

```rust
struct CompiledFamily {
    pub id: String,
    pub ext: Vec<String>,
    pub path_contains: String,
    pub re: Regex,
}

/// Compile every family's regex and validate its id (ids become file names).
pub(crate) fn compile_families(cfg: &Extractors) -> Result<Vec<CompiledFamily>, String> {
    let mut families: Vec<CompiledFamily> = Vec::new();
    for f in &cfg.families {
        let id_ok = !f.id.is_empty()
            && f.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
        if !id_ok {
            return Err(format!(
                "family id '{}' is invalid — use only [a-z0-9_-] (ids become index file names)",
                f.id
            ));
        }
        let re = Regex::new(&f.regex).map_err(|e| format!("family '{}': invalid regex: {e}", f.id))?;
        families.push(CompiledFamily {
            id: f.id.clone(),
            ext: f.ext.clone(),
            path_contains: f.path_contains.clone(),
            re,
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
```

In `run`, replace the old compile loop (`src/indexer.rs:68-88`) with:

```rust
    let families = compile_families(&cfg)?;
    if families.is_empty() {
        return Err(format!("profile '{profile}' declares no extraction families"));
    }
```

(Delete the now-duplicated `if cfg.families.is_empty()` check above it.) Then replace the `applicable` filter (`src/indexer.rs:111-117`) with:

```rust
        let applicable: Vec<&CompiledFamily> =
            families.iter().filter(|f| family_matches(f, &path_str, &ext)).collect();
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test`
Expected: PASS — `families_for_path_matches_by_ext_and_path`, plus the existing `rejects_path_traversal_family_id` (validation now lives in `compile_families`, still reached via `run`) and `reindex_clears_stale_family_files`.

- [ ] **Step 5: Commit**

```bash
git add src/indexer.rs
git commit -m "refactor(indexer): extract families_for_path + compile_families

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `fact <family> [name]` command

**Files:**
- Modify: `src/indexer.rs` (add `fact_families`, `fact_report`; reuse `Symbol`)
- Modify: `src/main.rs` (add `Fact` variant + dispatch + `cmd_fact`)
- Test: `src/indexer.rs` (`tests` module)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/indexer.rs`:

```rust
    #[test]
    fn fact_report_rejects_unknown_family() {
        let (kn, repo) = fixture(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'class\\s+(?P<name>\\w+)'\n",
        );
        // also need a profile.yaml with fact_families for validation
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
        fs::write(prof.join("profile.yaml"), "fact_families:\n  - { id: viewmodel, symbol: true }\n  - { id: service, symbol: true }\n").unwrap();
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(idx.join("symbols.json"),
            r#"[{"name":"LoginViewModel","kind":"viewmodel","file":"a.kt","line":1},
                {"name":"PaymentViewModel","kind":"viewmodel","file":"b.kt","line":2},
                {"name":"AuthService","kind":"service","file":"c.kt","line":3}]"#).unwrap();
        let all = fact_report(repo.path(), kn.path(), "p", "viewmodel", None).unwrap();
        assert!(all.contains("LoginViewModel") && all.contains("PaymentViewModel"));
        assert!(!all.contains("AuthService"), "must not include other families");
        let one = fact_report(repo.path(), kn.path(), "p", "viewmodel", Some("login")).unwrap();
        assert!(one.contains("LoginViewModel") && !one.contains("PaymentViewModel"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test fact_report`
Expected: FAIL — `fact_report` is not defined.

- [ ] **Step 3: Implement `fact_families` + `fact_report`**

In `src/indexer.rs`, add (the `Symbol` struct already has the needed fields):

```rust
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
    let p = repo.join(".palugada").join("index").join("symbols.json");
    let data = match fs::read_to_string(&p) {
        Ok(d) => d,
        Err(_) => return Ok(format!("(no index at {} — run `palugada index`)", p.display())),
    };
    let symbols: Vec<Symbol> =
        serde_json::from_str(&data).map_err(|e| format!("parse {}: {e}", p.display()))?;
    let needle = name.map(|n| n.to_lowercase());
    let mut out = String::new();
    let mut hits = 0;
    for s in &symbols {
        if s.kind != family {
            continue;
        }
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
        out.push_str(&format!("(no '{family}' facts{})", name.map(|n| format!(" matching '{n}'")).unwrap_or_default()));
    }
    Ok(out)
}
```

- [ ] **Step 4: Add the `Fact` command to `src/main.rs`**

Add a variant to `enum Commands` (after `Symbol`, ~`src/main.rs:109`):

```rust
    /// Look up indexed facts of a profile-declared family: `fact <family> [name]`.
    Fact {
        /// Fact family id declared in the profile (e.g. viewmodel, route).
        family: String,
        /// Optional name substring filter.
        name: Option<String>,
        /// Profile override.
        #[arg(long)]
        profile: Option<String>,
    },
```

Add to the dispatch `match` in `run` (after the `Symbol` arm, ~`src/main.rs:261`):

```rust
        Commands::Fact { family, name, profile } => cmd_fact(family, name, profile, project),
```

Add the handler next to `cmd_symbol` (~`src/main.rs:390`):

```rust
fn cmd_fact(
    family: String,
    name: Option<String>,
    profile: Option<String>,
    project: Option<&str>,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    let report = indexer::fact_report(&repo, &kn, &prof, &family, name.as_deref())?;
    println!("{}", report.trim_end());
    Ok(())
}
```

- [ ] **Step 5: Run tests + build**

Run: `cargo test fact_report && cargo build`
Expected: both tests PASS; build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/indexer.rs src/main.rs
git commit -m "feat(fact): generic 'fact <family> [name]' lookup over the index

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Priority-fill + truncation budget

**Files:**
- Modify: `src/brief.rs` (replace the budget render loop `src/brief.rs:84-98`; add `Pack.kind`/`Pack.rerun`, `Render`, `priority`, `est_tokens`, `truncate_to_tokens`, `budget_packs`)
- Test: `src/brief.rs` (new `#[cfg(test)]` module)

- [ ] **Step 1: Write the failing tests**

Add a test module at the end of `src/brief.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn pack(kind: &str, content: &str) -> Pack {
        Pack { step: kind.into(), kind: kind.into(), title: kind.into(), content: content.into(), rerun: "x".into() }
    }

    #[test]
    fn truncate_keeps_at_least_one_line_and_counts_dropped() {
        let (kept, dropped) = truncate_to_tokens("a\nb\nc\nd", 0);
        assert_eq!(kept, "a");
        assert_eq!(dropped, 3);
    }

    #[test]
    fn budget_prefers_high_priority_and_omits_low() {
        // big strings so each pack costs ~25 tokens; budget only fits one.
        let big = "x".repeat(100);
        let packs = vec![pack("code.recent", &big), pack("prd.context", &big)];
        let r = budget_packs(&packs, 30);
        // prd.context (priority 5) kept full; code.recent (priority 1) omitted.
        assert!(matches!(r[1], Render::Full));
        assert!(matches!(r[0], Render::Omitted));
    }

    #[test]
    fn top_priority_pack_never_omitted_even_over_budget() {
        let huge = "y".repeat(10_000);
        let packs = vec![pack("prd.context", &huge)];
        let r = budget_packs(&packs, 10);
        assert!(matches!(r[0], Render::Truncated { .. }));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib brief`
Expected: FAIL — `Pack` has no `kind`/`rerun`, and `Render`/`budget_packs`/`truncate_to_tokens` are undefined.

- [ ] **Step 3: Extend `Pack`, add budget machinery**

In `src/brief.rs`, replace the `Pack` struct (`src/brief.rs:27-32`) with:

```rust
#[derive(Serialize)]
struct Pack {
    step: String,
    #[serde(skip)]
    kind: String,
    title: String,
    content: String,
    #[serde(skip)]
    rerun: String,
}

enum Render {
    Full,
    Truncated { kept: String, dropped: usize },
    Omitted,
}

fn est_tokens(s: &str) -> usize {
    s.len() / 4 + 8
}

/// Higher = more valuable, kept first under a tight budget.
fn priority(kind: &str) -> u8 {
    match kind {
        "prd.context" => 5,
        "symbol.find" | "module.info" | "diff.scan" => 4,
        "convention" => 3,
        "recipe" => 2,
        "code.recent" => 1,
        _ => 0,
    }
}

/// Keep whole lines while the running token estimate stays within `max_tokens`;
/// always keep at least the first line. Returns (kept_text, dropped_line_count).
fn truncate_to_tokens(content: &str, max_tokens: usize) -> (String, usize) {
    let lines: Vec<&str> = content.lines().collect();
    let mut kept: Vec<&str> = Vec::new();
    let mut used = 0usize;
    for line in &lines {
        let cost = line.len() / 4 + 1;
        if !kept.is_empty() && used + cost > max_tokens {
            break;
        }
        kept.push(line);
        used += cost;
    }
    (kept.join("\n"), lines.len() - kept.len())
}

/// Decide each pack's fate by descending priority: full while it fits, then
/// truncate the one that overflows, then omit the rest. The top-priority pack
/// is always included (truncated if it alone exceeds the budget).
fn budget_packs(packs: &[Pack], budget: usize) -> Vec<Render> {
    let mut order: Vec<usize> = (0..packs.len()).collect();
    order.sort_by(|&a, &b| priority(&packs[b].kind).cmp(&priority(&packs[a].kind)).then(a.cmp(&b)));
    let mut renders: Vec<Render> = packs.iter().map(|_| Render::Omitted).collect();
    let mut used = 0usize;
    for (rank, &i) in order.iter().enumerate() {
        let cost = est_tokens(&packs[i].content);
        if used + cost <= budget {
            renders[i] = Render::Full;
            used += cost;
        } else {
            let remaining = budget.saturating_sub(used);
            if remaining > 0 || rank == 0 {
                let (kept, dropped) = truncate_to_tokens(&packs[i].content, remaining);
                renders[i] = Render::Truncated { kept, dropped };
                used = budget;
            }
        }
    }
    renders
}
```

- [ ] **Step 4: Rewrite the render loop in `run`**

In `src/brief.rs`, the steps loop currently builds `packs` with `Pack { step, title, content }` (`src/brief.rs:75`). Update that struct literal to also set `kind` and `rerun`:

```rust
        packs.push(Pack {
            step: step.clone(),
            kind: kind.clone(),
            title,
            content,
            rerun: rerun_hint(&kind, &arg, &opts.target),
        });
```

Replace the budget render block (`src/brief.rs:88-98`) with:

```rust
    let renders = budget_packs(&packs, opts.budget);
    let mut used = 0usize;
    for (p, r) in packs.iter().zip(&renders) {
        match r {
            Render::Full => {
                println!("## {}\n{}\n", p.title, p.content.trim());
                used += est_tokens(&p.content);
            }
            Render::Truncated { kept, dropped } => {
                println!("## {}\n{}", p.title, kept.trim());
                println!("(+{dropped} lines truncated — run `{}` for the rest)\n", p.rerun);
                used += est_tokens(kept);
            }
            Render::Omitted => {
                println!("## {}\n(omitted — over budget; run `{}`)\n", p.title, p.rerun);
            }
        }
    }
    println!("(~{used} tokens)");
    Ok(())
```

Add the pointer helper near `parse_step` (`src/brief.rs:104`):

```rust
/// The command an agent runs to get a step's full content (shown when truncated/omitted).
fn rerun_hint(kind: &str, arg: &str, target: &str) -> String {
    match kind {
        "convention" => format!("palugada q {arg}"),
        "recipe" => format!("palugada for {arg}"),
        "symbol.find" => format!("palugada symbol {target}"),
        "module.info" => "palugada index".to_string(),
        "diff.scan" => format!("git diff {}", if target.is_empty() { "HEAD" } else { target }),
        "prd.context" => format!("palugada issue view {target}"),
        "code.recent" => format!("git log -- {target}"),
        _ => "palugada q --list".to_string(),
    }
}
```

- [ ] **Step 5: Run tests + verify existing flow still works**

Run: `cargo test --lib brief && cargo build`
Expected: the three budget tests PASS; build succeeds. (JSON output keeps its `{step,title,content}` shape because `kind`/`rerun` are `#[serde(skip)]`.)

- [ ] **Step 6: Commit**

```bash
git add src/brief.rs
git commit -m "feat(brief): priority-fill budget with truncation + rerun pointers

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `module.info` step

**Files:**
- Modify: `src/indexer.rs` (add `module_report`)
- Modify: `src/brief.rs` (add the `module.info` match arm)
- Test: `src/indexer.rs` (`tests` module)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src/indexer.rs`:

```rust
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
    fn module_report_needs_a_target() {
        let repo = tempfile::tempdir().unwrap();
        assert!(module_report(repo.path(), "").contains("needs a target"));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test module_report`
Expected: FAIL — `module_report` is not defined.

- [ ] **Step 3: Implement `module_report`**

In `src/indexer.rs`, add:

```rust
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
    let symbols: Vec<Symbol> = match serde_json::from_str(&data) {
        Ok(s) => s,
        Err(e) => return format!("(parse {}: {e})", p.display()),
    };
    let in_module: Vec<&Symbol> = symbols
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
```

- [ ] **Step 4: Wire the step in `src/brief.rs`**

Add a match arm in `run`'s step loop, before the catch-all `other =>` (currently `src/brief.rs:70`):

```rust
            "module.info" => (
                format!("module info for '{}'", opts.target),
                indexer::module_report(repo, &opts.target),
            ),
```

- [ ] **Step 5: Run tests + build**

Run: `cargo test module_report && cargo build`
Expected: both tests PASS; build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/indexer.rs src/brief.rs
git commit -m "feat(brief): module.info step summarises facts under a module prefix

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: `diff.scan` + `convention(by-file-kind)` + `review_map`

**Files:**
- Modify: `src/brief.rs` (add `BriefContext`, `review_map` parse, `classify_files`, `mapped_topics`, `diff.scan` + `by-file-kind` arms; thread `&mut ctx` through the loop)
- Modify: `knowledge/profiles/android-mvvm/profile.yaml` (add `review_map`)
- Test: `src/brief.rs` (`tests` module)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/brief.rs`:

```rust
    #[test]
    fn mapped_topics_dedupes_across_touched_families() {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        map.insert("viewmodel".into(), vec!["architecture".into(), "testing".into()]);
        map.insert("route".into(), vec!["architecture".into()]);
        let mut touched = std::collections::BTreeSet::new();
        touched.insert("viewmodel".to_string());
        touched.insert("route".to_string());
        let topics = mapped_topics(&map, &touched);
        assert_eq!(topics, vec!["architecture".to_string(), "testing".to_string()]);
    }

    #[test]
    fn classify_files_groups_and_collects_families() {
        let cfg: indexer::Extractors = serde_yaml::from_str(
            "families:\n  - id: viewmodel\n    ext: [kt]\n    regex: 'x'\n  - id: i18n\n    ext: [xml]\n    path_contains: values\n    regex: 'x'\n",
        ).unwrap();
        let fams = indexer::compile_families(&cfg).unwrap();
        let files = vec!["a/Login.kt".to_string(), "a/values/strings.xml".to_string(), "README.md".to_string()];
        let (groups, touched) = classify_files(&files, &fams);
        assert!(touched.contains("viewmodel") && touched.contains("i18n"));
        assert!(groups.get("(unclassified)").map(|v| v.contains(&"README.md".to_string())).unwrap_or(false));
    }
```

Note: this requires `indexer::Extractors` and `indexer::compile_families` to be reachable. Make `Extractors` `pub` and `compile_families` `pub` in `src/indexer.rs` (change `pub(crate)` from Task 1 to `pub`, and add `pub` to `struct Extractors` and its fields `families`/`ignore_dirs`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib brief`
Expected: FAIL — `mapped_topics`, `classify_files`, and the `indexer` re-exports are undefined.

- [ ] **Step 3: Add `BriefContext`, parsing, and pure helpers**

In `src/brief.rs`, rename the flows struct to also carry `review_map`:

```rust
#[derive(Deserialize, Default)]
struct ProfileMeta {
    #[serde(default)]
    flows: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    review_map: BTreeMap<String, Vec<String>>,
}
```

Update `run` to deserialize `ProfileMeta` instead of `ProfileFlows` (the `pf.flows.get(...)` usage is unchanged). Add a context type + helpers:

```rust
use std::collections::BTreeSet;

#[derive(Default)]
struct BriefContext {
    touched_families: BTreeSet<String>,
}

/// Group changed files by their fact family (unmatched → "(unclassified)") and
/// collect the set of families touched.
fn classify_files(
    files: &[String],
    families: &[indexer::CompiledFamily],
) -> (BTreeMap<String, Vec<String>>, BTreeSet<String>) {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut touched: BTreeSet<String> = BTreeSet::new();
    for f in files {
        let ext = Path::new(f).extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let ids = indexer::families_for_path(f, &ext, families);
        if ids.is_empty() {
            groups.entry("(unclassified)".to_string()).or_default().push(f.clone());
        } else {
            for id in ids {
                touched.insert(id.clone());
                groups.entry(id).or_default().push(f.clone());
            }
        }
    }
    (groups, touched)
}

/// Deduped, sorted convention topics for every touched family present in the map.
fn mapped_topics(review_map: &BTreeMap<String, Vec<String>>, touched: &BTreeSet<String>) -> Vec<String> {
    let mut topics: BTreeSet<String> = BTreeSet::new();
    for fam in touched {
        if let Some(ts) = review_map.get(fam) {
            for t in ts {
                topics.insert(t.clone());
            }
        }
    }
    topics.into_iter().collect()
}

/// `git -C <repo> diff --name-only <ref>` → changed file paths.
fn git_changed_files(repo: &Path, gitref: &str) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .arg("-C").arg(repo)
        .args(["diff", "--name-only", gitref])
        .output()
        .map_err(|e| format!("git diff: {e}"))?;
    if !out.status.success() {
        return Err("git diff failed".to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(|l| l.to_string()).filter(|l| !l.is_empty()).collect())
}
```

- [ ] **Step 4: Thread `&mut ctx` and add the two arms**

In `run`, create the context before the step loop and pass it to a per-step builder. Change the loop body so the `match` arms can mutate `ctx`. Replace the loop opening (`src/brief.rs:50-53`) so it reads:

```rust
    let mut packs: Vec<Pack> = Vec::new();
    let mut ctx = BriefContext::default();
    for step in steps {
        let (kind, arg) = parse_step(step);
        let (title, content) = match kind.as_str() {
```

Add the `by-file-kind` arm **immediately before the existing `"convention" =>` arm** (`src/brief.rs:54`) — a guarded arm must precede the general one or it is never reached. Add the `diff.scan` arm anywhere before the catch-all `other =>`:

```rust
            "diff.scan" => {
                let gitref = if opts.target.is_empty() { "HEAD" } else { opts.target.as_str() };
                let content = match (indexer::load_families(kn, profile), git_changed_files(repo, gitref)) {
                    (Ok((_ignore, families)), Ok(files)) if !files.is_empty() => {
                        let (groups, touched) = classify_files(&files, &families);
                        ctx.touched_families = touched;
                        let mut s = String::new();
                        for (fam, fs) in &groups {
                            s.push_str(&format!("{fam}: {}\n", fs.join(", ")));
                        }
                        s
                    }
                    (_, Ok(files)) if files.is_empty() => format!("(no changed files vs {gitref})"),
                    (Err(e), _) => format!("({e})"),
                    (_, Err(e)) => format!("({e})"),
                };
                (format!("changed files vs {gitref}"), content)
            }
            "convention" if arg == "by-file-kind" => {
                let content = if ctx.touched_families.is_empty() {
                    "(run diff.scan first — no touched families recorded)".to_string()
                } else {
                    let topics = mapped_topics(&pf.review_map, &ctx.touched_families);
                    if topics.is_empty() {
                        "(no review_map entries for the touched families)".to_string()
                    } else {
                        topics
                            .iter()
                            .map(|t| format!("### {t}\n{}", knowledge::convention_outline(kn, profile, t).unwrap_or_else(|e| format!("({e})"))))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    }
                };
                ("conventions by file kind".to_string(), content)
            }
```

This requires a `load_families` that also reads `extractors.yaml` from disk. Add it to `src/indexer.rs`:

```rust
/// Read + compile a profile's extractors.yaml. Returns (ignore_dirs, families).
pub fn load_families(kn: &Path, profile: &str) -> Result<(Vec<String>, Vec<CompiledFamily>), String> {
    let ext_path = kn.join("profiles").join(profile).join("extractors.yaml");
    let raw = fs::read_to_string(&ext_path)
        .map_err(|e| format!("no extractors.yaml for profile '{profile}' ({}): {e}", ext_path.display()))?;
    let cfg: Extractors = serde_yaml::from_str(&raw).map_err(|e| format!("parse {}: {e}", ext_path.display()))?;
    let families = compile_families(&cfg)?;
    Ok((cfg.ignore_dirs, families))
}
```

Refactor `indexer::run` to use `load_families` for its own load (replace `src/indexer.rs:57-66` + the compile call from Task 1):

```rust
    let (ignore, families) = load_families(kn, profile)?;
    if families.is_empty() {
        return Err(format!("profile '{profile}' declares no extraction families"));
    }
```

(`ignore` replaces the later `let ignore = cfg.ignore_dirs.clone();` at `src/indexer.rs:90` — delete that line.)

- [ ] **Step 5: Add `review_map` to the bundled profile**

Append to `knowledge/profiles/android-mvvm/profile.yaml`:

```yaml

# Maps each fact family to the convention topics a reviewer should check when a
# file of that kind changed. Drives `brief review` (diff.scan → by-file-kind).
review_map:
  viewmodel:  [architecture]
  repository: [architecture]
  service:    [architecture]
  route:      [architecture]
  i18n:       [architecture]
```

- [ ] **Step 6: Run tests + build + manual review smoke**

Run: `cargo test && cargo build`
Expected: all tests PASS (incl. the two new brief tests + existing indexer tests); build succeeds.

Manual smoke (from this repo, which is a git checkout):
Run: `./target/debug/palugada brief review HEAD~1 --profile android-mvvm`
Expected: a `## changed files vs HEAD~1` section grouping files, then `## conventions by file kind` (likely "(no review_map entries…)" since this repo is Rust, not Kotlin — that is correct behavior, not a failure).

- [ ] **Step 7: Commit**

```bash
git add src/brief.rs src/indexer.rs knowledge/profiles/android-mvvm/profile.yaml
git commit -m "feat(brief): diff.scan + convention(by-file-kind) via profile review_map

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: `prd.context` step + connector plumbing

**Files:**
- Modify: `src/brief.rs` (add `BriefConnectors`, `format_issue_pack`, `prd_context_content`, the `prd.context` arm; extend `run` signature)
- Modify: `src/main.rs` (`cmd_brief` resolves connectors best-effort + passes `insecure`)
- Test: `src/brief.rs` (`tests` module)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/brief.rs`:

```rust
    #[test]
    fn prd_context_degrades_without_connectors() {
        assert!(prd_context_content(None, "PROJ-1").contains("no project"));
    }

    #[test]
    fn prd_context_notes_empty_target() {
        let c = BriefConnectors {
            pc: crate::config::ProjectConfig::default(),
            auth: crate::config::AuthProfile::default(),
            insecure: false,
        };
        assert!(prd_context_content(Some(&c), "").contains("no target ticket"));
    }

    #[test]
    fn prd_context_notes_missing_tracker() {
        // default ProjectConfig has no issue_tracker → factory errors, degraded to a note.
        let c = BriefConnectors {
            pc: crate::config::ProjectConfig::default(),
            auth: crate::config::AuthProfile::default(),
            insecure: false,
        };
        let out = prd_context_content(Some(&c), "PROJ-1");
        assert!(out.starts_with('(') && out.contains("issue_tracker"));
    }

    #[test]
    fn format_issue_pack_includes_key_and_excerpt() {
        let i = crate::clients::Issue {
            key: "T-1".into(), summary: "Add export".into(), status: "Open".into(),
            issue_type: "Story".into(), assignee: "me".into(), description: "spec body".into(),
        };
        let s = format_issue_pack(&i);
        assert!(s.contains("T-1 — Add export") && s.contains("spec body"));
    }
```

Note: `ProjectConfig` and `AuthProfile` must derive `Default` (verify in `src/config.rs`; `AuthProfile` already does — `src/config.rs:119`. If `ProjectConfig` does not, add `#[derive(Default)]` to it at `src/config.rs:200`).

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib brief`
Expected: FAIL — `prd_context_content`, `BriefConnectors`, `format_issue_pack` are undefined.

- [ ] **Step 3: Implement the connector type, helpers, and step**

In `src/brief.rs`, add imports and types:

```rust
use crate::clients;
use crate::config::{AuthProfile, ProjectConfig};

pub struct BriefConnectors {
    pub pc: ProjectConfig,
    pub auth: AuthProfile,
    pub insecure: bool,
}

fn format_issue_pack(i: &clients::Issue) -> String {
    let excerpt: String = i.description.chars().take(600).collect();
    format!(
        "{} — {}\nStatus: {} · Type: {} · Assignee: {}\nSpec excerpt: {}",
        i.key, i.summary, i.status, i.issue_type, i.assignee, excerpt
    )
}

/// Fetch the target ticket via the project's IssueTracker. Every failure path
/// degrades to an inline `(…)` note so `brief` never aborts on the network.
fn prd_context_content(connectors: Option<&BriefConnectors>, target: &str) -> String {
    match connectors {
        None => "(no project/credentials resolved — run brief inside a registered project)".to_string(),
        Some(_) if target.is_empty() => "(no target ticket)".to_string(),
        Some(c) => match clients::issue_tracker(&c.pc, &c.auth, c.insecure) {
            Err(e) => format!("({e})"),
            Ok(tracker) => match tracker.get_issue(target) {
                Err(e) => format!("(could not fetch {target}: {e})"),
                Ok(i) => format_issue_pack(&i),
            },
        },
    }
}
```

Change the `run` signature to accept the connectors:

```rust
pub fn run(
    kn: &Path,
    repo: &Path,
    profile: &str,
    opts: &BriefOptions,
    connectors: Option<&BriefConnectors>,
) -> Result<(), String> {
```

Add the `prd.context` arm (before the catch-all `other =>`):

```rust
            "prd.context" => (
                format!("ticket context for '{}'", opts.target),
                prd_context_content(connectors, &opts.target),
            ),
```

- [ ] **Step 4: Resolve connectors in `cmd_brief`**

In `src/main.rs`, update the dispatch arm (`src/main.rs:262-264`) to pass `insecure`:

```rust
        Commands::Brief { flow, target, budget, json, profile } => {
            cmd_brief(flow, target, budget, json, profile, project, cli.insecure)
        }
```

Update `cmd_brief` (`src/main.rs:394-408`):

```rust
fn cmd_brief(
    flow: String,
    target: String,
    budget: usize,
    json: bool,
    profile: Option<String>,
    project: Option<&str>,
    insecure: bool,
) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let kn = knowledge::knowledge_dir(&global)?;
    let prof = resolve_profile(&global, project, profile.as_deref(), &kn)?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo = config::resolve_repo(&global, project, None, &cwd)?;
    // Best-effort: brief works without a project (local steps); only prd.context needs this.
    let connectors = Secrets::load_or_default()
        .ok()
        .and_then(|s| config::resolve_project(&global, &s, project).ok())
        .map(|(_n, pc, auth)| brief::BriefConnectors { pc, auth, insecure });
    brief::run(&kn, &repo, &prof, &brief::BriefOptions { flow, target, budget, json }, connectors.as_ref())
}
```

- [ ] **Step 5: Run tests + build**

Run: `cargo test && cargo build`
Expected: the four new `prd_context`/`format_issue_pack` tests PASS; all earlier tests still PASS; build succeeds.

- [ ] **Step 6: Commit**

```bash
git add src/brief.rs src/main.rs src/config.rs
git commit -m "feat(brief): prd.context step (lazy IssueTracker, network-safe degrade)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Docs + final verification

**Files:**
- Modify: `README.md` (commands table + roadmap)

- [ ] **Step 1: Document `fact` in the commands table**

In `README.md`, add a row after the `symbol` row (`README.md:168`):

```markdown
| `palugada fact <family> [name]` | look up indexed facts of a profile-declared family (e.g. `fact viewmodel Login`) |
```

- [ ] **Step 2: Update the roadmap + "Done so far" lines**

In `README.md`, replace the first roadmap bullet (`README.md:284-286`) with:

```markdown
- Richer extractors (tree-sitter where regex is too coarse) and typed fact
  aliases over the index.
```

Replace the "Done so far" sentence (`README.md:294-297`) so it states all four flows and `fact` ship:

```markdown
Done so far: connectors (Jira / Confluence / Figma / Jenkins / GitLab / GitHub),
`palugada init` (offline multi-agent scaffolding), knowledge reads
(`q` / `for` / `s`), the project indexer (`index` + `symbol` + `fact`), and flow
context packs (`brief` — all four flows wired: bugfix, feature, refactor, review).
```

- [ ] **Step 3: Full verification**

Run: `cargo test && cargo build --release && ./target/release/palugada brief --help`
Expected: all tests PASS; release build succeeds; `brief` help prints.

Run: `cargo build --release 2>&1 | grep -i warning || echo "no warnings"`
Expected: `no warnings` (or only pre-existing ones).

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: document fact command + mark all brief flows complete

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-review notes

- **Spec coverage:** §4.1 connectors → Task 6; §4.2 `BriefContext` → Task 5; §4.3 `families_for_path` → Task 1; §5 four steps → Tasks 4/5/6; §6 priority-fill → Task 3; §7 `fact` → Task 2; §8 `review_map` → Task 5; §9 tests → distributed across tasks.
- **Ordering:** Tasks are bottom-up — each leaves the crate compiling with green tests. Task 1 refactor keeps existing indexer tests passing; Task 3 keeps JSON output schema stable via `#[serde(skip)]`.
- **Type consistency:** `compile_families`/`load_families`/`families_for_path`/`CompiledFamily`/`Extractors` are made `pub` in `src/indexer.rs` (Task 1 + Task 5) before `src/brief.rs` consumes them. `BriefConnectors`/`prd_context_content`/`format_issue_pack` names match between Task 6's tests and implementation. `ProfileMeta` replaces `ProfileFlows` in Task 5 and `run` reads `pf.flows`/`pf.review_map`.
- **Known follow-ups (out of scope, per spec §3):** wiki tie-in for `prd.context`, tree-sitter extraction, typed fact aliases, `stats`/cache.
