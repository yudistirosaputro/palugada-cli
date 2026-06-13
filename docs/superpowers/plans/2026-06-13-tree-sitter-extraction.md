# tree-sitter extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a fact family extract via a tree-sitter `.scm` query (structural) instead of only regex, bundle the Kotlin grammar, and migrate the `android-mvvm` structural families to queries.

**Architecture:** `CompiledFamily` gains an `Extractor` enum (`Regex` | `TreeSitter`). The file walk is unchanged; per file, regex families run as before and tree-sitter families run against a tree parsed once per language. `symbols.json` output is unchanged, so `symbol`/`fact`/`module.info`/`brief` are unaffected.

**Tech Stack:** Rust, `tree-sitter = "0.25"` (resolves 0.25.10), `tree-sitter-kotlin-ng = "1.1"`, `regex`, `serde`/`serde_yaml`, `walkdir`, `tempfile` (dev).

**Reference spec:** `docs/superpowers/specs/2026-06-13-tree-sitter-extraction-design.md`

**Verified API facts (from a throwaway spike, do not re-derive):**
- Grammar: `let lang: tree_sitter::Language = tree_sitter_kotlin_ng::LANGUAGE.into();`
- Query: `tree_sitter::Query::new(&lang, src)`, `query.capture_index_for_name("name") -> Option<u32>`.
- Walk matches: `let mut cur = tree_sitter::QueryCursor::new(); let mut it = cur.matches(&query, tree.root_node(), text.as_bytes());` — `it` is a **streaming** iterator: `use tree_sitter::StreamingIterator;` then `while let Some(m) = it.next() { for c in m.captures { if c.index == idx { c.node.utf8_text(text.as_bytes()) ; c.node.start_position().row + 1 } } }`.
- **`#match?` predicates ARE auto-applied** by `QueryCursor` in 0.25 — the suffix filter lives in the `.scm`, no Rust predicate code needed.
- Kotlin node shape (verified): a class **or interface** is `(class_declaration name: (identifier))`; the name is the field `name:` of type `identifier` (NOT `type_identifier`). Comments are `line_comment` and never match `class_declaration`.

**Test command:** `cargo test` · **Build:** `cargo build` · **Locked build (CI parity):** `cargo build --locked`

---

## File structure

| File | Responsibility | Change |
|---|---|---|
| `Cargo.toml` / `Cargo.lock` | deps | add `tree-sitter`, `tree-sitter-kotlin-ng` |
| `src/indexer.rs` | extraction engine | `Extractor` enum, `language_for`, `extract_file` (parse-once dispatch), extended `compile_families` (validation + `.scm` load) |
| `src/brief.rs` | (test call site only) | update `compile_families(&cfg)` → `compile_families(&cfg, dir)` in one test |
| `knowledge/profiles/android-mvvm/extractors.yaml` | profile data | `viewmodel`/`service`/`repository` → `query`+`language`; `route`/`i18n` stay regex |
| `knowledge/profiles/android-mvvm/extractors/*.scm` | query data | three new files |
| `README.md` | docs | note tree-sitter extraction |

---

## Task 1: Add deps + `language_for` registry

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/indexer.rs`
- Test: `src/indexer.rs` (`tests` module)

- [ ] **Step 1: Add the dependencies**

Run:
```bash
cargo add tree-sitter@0.25 tree-sitter-kotlin-ng@1.1
```
Expected: adds `tree-sitter v0.25.10`, `tree-sitter-kotlin-ng v1.1.0`, `tree-sitter-language v0.1.7`, `streaming-iterator v0.1.9` to `Cargo.lock`.

- [ ] **Step 2: Write the failing test**

Add to the `tests` module in `src/indexer.rs`:

```rust
    #[test]
    fn kotlin_grammar_loads_and_unknown_language_errors() {
        let lang = language_for("kotlin").unwrap();
        let q = tree_sitter::Query::new(&lang, r#"(class_declaration name: (identifier) @name)"#).unwrap();
        assert!(q.capture_index_for_name("name").is_some());
        assert!(language_for("klingon").unwrap_err().contains("klingon"));
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test kotlin_grammar_loads`
Expected: FAIL — `language_for` is not defined.

- [ ] **Step 4: Add `language_for`**

In `src/indexer.rs`, add near the top-level functions (e.g. after `families_for_path`):

```rust
/// Map a profile-declared `language` string to its bundled tree-sitter grammar.
/// Adding a language later = add its crate + one arm here (no profile change).
fn language_for(name: &str) -> Result<tree_sitter::Language, String> {
    match name {
        "kotlin" => Ok(tree_sitter_kotlin_ng::LANGUAGE.into()),
        other => Err(format!("unsupported language '{other}' (supported: kotlin)")),
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test kotlin_grammar_loads`
Expected: PASS (first run compiles the Kotlin grammar's C — may take ~30s).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/indexer.rs
git commit -m "feat(indexer): bundle tree-sitter + Kotlin grammar; language_for registry

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `Extractor` enum + parse-once dispatch engine

**Files:**
- Modify: `src/indexer.rs` (`Family`, `CompiledFamily`, `compile_families`, `run` body; add `Extractor`, `extract_file`)
- Modify: `src/brief.rs` (one test call site)
- Test: `src/indexer.rs` (`tests` module)

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/indexer.rs`:

```rust
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
        std::fs::write(dir.path().join("q.scm"), "(class_declaration) @other\n").unwrap();
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
```

The existing `reindex_clears_stale_family_files` test (regex-only) must keep passing — it is the regression guard.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test`
Expected: the four new tests FAIL to compile — `compile_families` takes one arg, `Extractor` undefined.

- [ ] **Step 3: Extend `Family` (add `language`/`query`, make `regex` optional)**

In `src/indexer.rs`, replace the `Family` struct:

```rust
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
```

- [ ] **Step 4: Replace `CompiledFamily.re` with an `Extractor` enum**

Replace the `CompiledFamily` struct:

```rust
enum Extractor {
    Regex(Regex),
    TreeSitter { language: String, query: tree_sitter::Query },
}

pub struct CompiledFamily {
    pub id: String,
    pub ext: Vec<String>,
    pub path_contains: String,
    extractor: Extractor,
}
```

- [ ] **Step 5: Rewrite `compile_families` (new signature, validation, `.scm` load)**

Replace the whole `compile_families` function:

```rust
/// Compile every family into a regex or tree-sitter extractor and validate it.
/// `profile_dir` is where `.scm` query paths resolve from.
pub fn compile_families(cfg: &Extractors, profile_dir: &Path) -> Result<Vec<CompiledFamily>, String> {
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
```

- [ ] **Step 6: Update `load_families` to pass the profile dir**

In `src/indexer.rs`, in `load_families`, the line that calls `compile_families` becomes:

```rust
    let profile_dir = kn.join("profiles").join(profile);
    let families = compile_families(&cfg, &profile_dir)?;
```

(Insert `profile_dir` from the existing `ext_path` parent; `ext_path = profile_dir.join("extractors.yaml")` — you may rewrite the path build as `let profile_dir = kn.join("profiles").join(profile); let ext_path = profile_dir.join("extractors.yaml");`.)

- [ ] **Step 7: Replace the per-file extraction in `run` with `extract_file`**

In `run`, replace the regex extraction block (the `for fam in applicable { for caps in fam.re.captures_iter(&text) { … } }` loop) with a single call:

```rust
        extract_file(&text, &rel, &applicable, &mut symbols);
```

Then add the helper at the top level of `src/indexer.rs`:

```rust
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
```

- [ ] **Step 8: Fix the `compile_families` call in `src/brief.rs` test**

In `src/brief.rs`, the test `classify_files_groups_and_collects_families` calls `indexer::compile_families(&cfg)`. Update it to pass a dummy dir (its families are regex-only, so the dir is unused):

```rust
        let dir = tempfile::tempdir().unwrap();
        let fams = indexer::compile_families(&cfg, dir.path()).unwrap();
```

(`tempfile` is already a dev-dependency.)

- [ ] **Step 9: Run the full suite**

Run: `cargo test`
Expected: all pass — the four new indexer tests, the updated brief test, and every prior test (incl. `reindex_clears_stale_family_files` proving regex still works).

- [ ] **Step 10: Commit**

```bash
git add src/indexer.rs src/brief.rs
git commit -m "feat(indexer): Extractor enum + parse-once tree-sitter dispatch

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: Migrate the android-mvvm profile to tree-sitter

**Files:**
- Create: `knowledge/profiles/android-mvvm/extractors/viewmodel.scm`
- Create: `knowledge/profiles/android-mvvm/extractors/service.scm`
- Create: `knowledge/profiles/android-mvvm/extractors/repository.scm`
- Modify: `knowledge/profiles/android-mvvm/extractors.yaml`

- [ ] **Step 1: Write the three query files**

Create `knowledge/profiles/android-mvvm/extractors/viewmodel.scm`:
```scheme
(class_declaration name: (identifier) @name (#match? @name "ViewModel$"))
```

Create `knowledge/profiles/android-mvvm/extractors/service.scm` (Kotlin interfaces also parse as `class_declaration`):
```scheme
(class_declaration name: (identifier) @name (#match? @name "Service$"))
```

Create `knowledge/profiles/android-mvvm/extractors/repository.scm`:
```scheme
(class_declaration name: (identifier) @name (#match? @name "RepositoryImpl$"))
```

- [ ] **Step 2: Point the three families at their queries**

In `knowledge/profiles/android-mvvm/extractors.yaml`, replace the `viewmodel`, `repository`, and `service` family entries (the regex ones) with query-based ones, leaving `route` and `i18n` unchanged:

```yaml
  - id: viewmodel
    ext: [kt]
    language: kotlin
    query: extractors/viewmodel.scm

  - id: repository
    ext: [kt]
    language: kotlin
    query: extractors/repository.scm

  - id: service
    ext: [kt]
    language: kotlin
    query: extractors/service.scm
```

- [ ] **Step 3: Smoke-test the migrated profile against a Kotlin fixture**

Run:
```bash
mkdir -p /tmp/ktsmoke && printf 'class LoginViewModel : ViewModel()\ninterface AuthService\nclass UserRepositoryImpl\n// class GhostViewModel\n' > /tmp/ktsmoke/A.kt
cargo run -q -- index --repo /tmp/ktsmoke --profile android-mvvm
```
Expected: counts show `viewmodel 1`, `service 1`, `repository 1` (and `GhostViewModel` from the comment is NOT counted). If a count is 0, dump the tree with the spike technique and adjust the `.scm` — the node names are verified (`class_declaration` / `name: (identifier)`), so a 0 means the suffix predicate or fixture differs.

- [ ] **Step 4: Confirm the bundled profile still validates via tests**

Run: `cargo test`
Expected: all pass (no test reads the real profile, but this guards against accidental edits to shared code).

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/android-mvvm/extractors.yaml knowledge/profiles/android-mvvm/extractors/
git commit -m "feat(profile): migrate android-mvvm viewmodel/service/repository to tree-sitter

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Docs + final verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Note tree-sitter extraction in the indexer description**

In `README.md`, the bullet describing the indexer currently reads "A local code indexer — `index` scans your repo …". Append a sentence:

```markdown
  Extraction is per fact-family: structural **tree-sitter** queries (Kotlin
  today) with regex for the long tail.
```

- [ ] **Step 2: Update the roadmap line that mentioned tree-sitter**

In `README.md`, the roadmap bullet currently says "Richer extractors (tree-sitter where regex is too coarse) and typed fact aliases …". Replace it with:

```markdown
- More tree-sitter grammars (Swift, TS, Go, Python) and typed fact aliases
  (`viewmodel` / `service` …) layered over the generic `fact` command.
```

- [ ] **Step 3: Full verification**

Run: `cargo test && cargo build --locked --release`
Expected: all tests pass; locked release build succeeds (proves `Cargo.lock` is consistent for CI).

Run: `./target/release/palugada index --repo /tmp/ktsmoke --profile android-mvvm`
Expected: `viewmodel 1`, `service 1`, `repository 1`.

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: note tree-sitter extraction in indexer + roadmap

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-review notes

- **Spec coverage:** §4.1 parse-once → Task 2 `extract_file`; §4.2 `Extractor` enum → Task 2; §4.3 `language_for` → Task 1; §4.4 `@name` convention → Task 2 (`extract_file` + validation); §5 schema + validation → Task 2 `compile_families`; §6 deps/build → Task 1 + Task 4 `--locked`; §7 migration → Task 3; §8 error handling → Task 2 validation + `extract_file` skip-on-fail; §9 tests → Task 2 (extraction, comment-skip, validation, regression) + Task 3 smoke.
- **Type consistency:** `compile_families(&cfg, profile_dir)` two-arg signature is used identically in Task 2 (definition, `load_families`, tests) and Task 2 Step 8 (brief test). `Extractor`/`CompiledFamily.extractor`/`language_for`/`extract_file` names match across tasks. `families_for_path`/`family_matches` are untouched (they only read `id`/`ext`/`path_contains`, still `pub`).
- **Ordering:** Task 1 lands deps + registry green; Task 2 is the atomic engine refactor (can't be split — `run` must compile) and keeps regex tests green; Task 3 is pure data; Task 4 is docs + CI-parity check.
- **Out of scope (per spec §3):** other grammars, migrating route/i18n, removing regex, the `plugin:` escape hatch.
