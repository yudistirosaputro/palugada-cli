# Generic symbol index Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement task-by-task. Steps use `- [ ]`.

**Goal:** Index *all* definitions (class/object/function/method/property) per file via a per-language tree-sitter tags query, so `palugada symbol` finds functions (with kind/scope/signature), not just classes.

**Architecture:** A language-driven generic pass in `indexer::run` parses each known-language file once and runs an embedded tags query, writing rich `SymbolDef`s to `index/symbols.json`. Curated fact families keep their per-family files; `fact` reads those directly. `symbol`/`module.info`/`brief symbol.find` read the generic index.

**Tech Stack:** Rust, `tree_sitter` + `tree_sitter_kotlin_ng` (already deps), `serde`.

**Reference spec:** `docs/superpowers/specs/2026-06-14-generic-symbol-index-design.md`

**Verified Kotlin nodes (do not re-derive):** `class_declaration name:(identifier)` (also interface/enum), `object_declaration name:(identifier)`, `function_declaration name:(identifier)` (methods too), `property_declaration (variable_declaration (identifier))`. Bodies: `function_body` / `class_body` / `enum_class_body`.

**Test:** `cargo test` · **Build:** `cargo build`

---

## Task 1: language/ext + tags-query registries + kotlin.scm

**Files:** Create `src/tags/kotlin.scm`; modify `src/indexer.rs` (+ test).

- [ ] **Step 1:** Create `src/tags/kotlin.scm`:

```scheme
(class_declaration name: (identifier) @class)
(object_declaration name: (identifier) @object)
(function_declaration name: (identifier) @function)
(property_declaration (variable_declaration (identifier) @property))
```

- [ ] **Step 2: failing test** in `indexer::tests`:

```rust
    #[test]
    fn tags_registry_resolves_kotlin() {
        assert_eq!(language_for_ext("kt"), Some("kotlin"));
        assert_eq!(language_for_ext("kts"), Some("kotlin"));
        assert_eq!(language_for_ext("txt"), None);
        let q = tags_query("kotlin").unwrap();
        let lang = language_for("kotlin").unwrap();
        assert!(tree_sitter::Query::new(&lang, q).is_ok(), "kotlin.scm must compile");
    }
```

- [ ] **Step 3: implement** — in `src/indexer.rs`, near `language_for`:

```rust
const KOTLIN_TAGS: &str = include_str!("tags/kotlin.scm");

/// Map a file extension to a language with a generic tags query.
pub fn language_for_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "kt" | "kts" => Some("kotlin"),
        _ => None,
    }
}

/// The embedded tree-sitter tags query for a language, if any.
pub fn tags_query(lang: &str) -> Option<&'static str> {
    match lang {
        "kotlin" => Some(KOTLIN_TAGS),
        _ => None,
    }
}
```

- [ ] **Step 4:** `cargo test tags_registry_resolves_kotlin` → pass. **Step 5: commit** `feat(indexer): kotlin tags query + language/ext registries`.

---

## Task 2: `SymbolDef` + `extract_symbols` (the tags pass)

**Files:** Modify `src/indexer.rs` (+ test).

- [ ] **Step 1: failing test** in `indexer::tests`:

```rust
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
        assert!(login.signature.contains("fun login"));
        let tl = by("function", "topLevel").expect("topLevel is a function");
        assert_eq!(tl.scope, "");
        assert!(by("property", "title").is_some());
        // a fun inside a comment is never captured
        assert!(out.iter().all(|s| s.name != "ghost"));
    }
```

- [ ] **Step 2: run → fail** (`extract_symbols`/`SymbolDef` undefined).

- [ ] **Step 3: implement** — add to `src/indexer.rs`:

```rust
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
    let q_src = match tags_query(lang_name) { Some(q) => q, None => return };
    let lang = match language_for(lang_name) { Ok(l) => l, Err(_) => return };
    let query = match tree_sitter::Query::new(&lang, q_src) { Ok(q) => q, Err(_) => return };
    let mut parser = tree_sitter::Parser::new();
    if parser.set_language(&lang).is_err() {
        return;
    }
    let tree = match parser.parse(text, None) { Some(t) => t, None => return };
    let names = query.capture_names();
    let bytes = text.as_bytes();
    let mut cur = tree_sitter::QueryCursor::new();
    let mut it = cur.matches(&query, tree.root_node(), bytes);
    while let Some(m) = it.next() {
        for c in m.captures {
            let kind0 = names[c.index as usize];
            let name = match c.node.utf8_text(bytes) { Ok(s) => s.to_string(), Err(_) => continue };
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
```

- [ ] **Step 4:** `cargo test extract_symbols_finds_defs` → pass. **Step 5: commit** `feat(indexer): generic symbol extraction (SymbolDef + tags pass)`.

---

## Task 3: wire the generic pass into `run`

**Files:** Modify `src/indexer.rs`.

- [ ] **Step 1:** In `run`, replace the symbol-collection + write section so it builds **both** fact families (`facts`) and generic symbols (`defs`) in one walk, writes `symbols.json` from `defs`, and per-family files from `facts`.

Replace the loop's collection setup and per-file body:

```rust
    let mut facts: Vec<Symbol> = Vec::new();
    let mut defs: Vec<SymbolDef> = Vec::new();

    for entry in WalkDir::new(repo).into_iter().filter_entry(|e| !is_ignored(e, &ignore)) {
        let entry = match entry { Ok(e) => e, Err(_) => continue };
        if !entry.file_type().is_file() { continue; }
        let path = entry.path();
        let ext = path.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let path_str = path.to_string_lossy();

        let applicable: Vec<&CompiledFamily> =
            families.iter().filter(|f| family_matches(f, &path_str, &ext)).collect();
        let lang = language_for_ext(&ext).filter(|l| tags_query(l).is_some());
        if applicable.is_empty() && lang.is_none() {
            continue;
        }
        let text = match fs::read_to_string(path) { Ok(t) => t, Err(_) => continue };
        let rel = path.strip_prefix(repo).unwrap_or(path).to_string_lossy().to_string();

        if !applicable.is_empty() {
            extract_file(&text, &rel, &applicable, &mut facts);
        }
        if let Some(l) = lang {
            extract_symbols(&text, &rel, l, &mut defs);
        }
    }
```

Then replace the write/manifest block:

```rust
    let out = repo.join(".palugada").join("index");
    if out.exists() {
        fs::remove_dir_all(&out).map_err(|e| format!("clear {}: {e}", out.display()))?;
    }
    fs::create_dir_all(&out).map_err(|e| format!("create {}: {e}", out.display()))?;

    // generic symbol index
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
```

Add `symbols: usize` to the `Manifest` struct:

```rust
#[derive(Serialize)]
struct Manifest {
    indexed_at: String,
    git_sha: String,
    total: usize,
    symbols: usize,
    counts: BTreeMap<String, usize>,
}
```

- [ ] **Step 2:** `cargo test && cargo build` — existing indexer tests still pass (`reindex_clears_stale_family_files` writes `viewmodel.json` from the fact pass; `tree_sitter_extracts_and_skips_comments` finds `LoginViewModel` in `symbols.json` via the generic class capture and not `GhostViewModel`).

- [ ] **Step 3: commit** `feat(indexer): build generic symbols.json + fact per-family files in one pass`.

---

## Task 4: `fact_report` reads per-family files

**Files:** Modify `src/indexer.rs` (+ update its test).

- [ ] **Step 1:** Change `fact_report` to read `<family>.json` instead of filtering `symbols.json`. Replace its body after the unknown-family check:

```rust
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
```

(`<family>.json` only contains that family, so the `s.kind != family` filter is gone.)

- [ ] **Step 2:** Update the test `fact_report_filters_by_kind_and_name` to write the per-family file instead of `symbols.json`:

```rust
        let idx = repo.path().join(".palugada").join("index");
        fs::create_dir_all(&idx).unwrap();
        fs::write(idx.join("viewmodel.json"),
            r#"[{"name":"LoginViewModel","kind":"viewmodel","file":"a.kt","line":1},
                {"name":"PaymentViewModel","kind":"viewmodel","file":"b.kt","line":2}]"#).unwrap();
        let all = fact_report(repo.path(), kn.path(), "p", "viewmodel", None).unwrap();
        assert!(all.contains("LoginViewModel") && all.contains("PaymentViewModel"));
        let one = fact_report(repo.path(), kn.path(), "p", "viewmodel", Some("login")).unwrap();
        assert!(one.contains("LoginViewModel") && !one.contains("PaymentViewModel"));
```

(Drop the `AuthService`/`service` lines — they were testing the kind filter against a mixed `symbols.json`, which no longer applies.)

- [ ] **Step 3:** `cargo test fact_report` → pass. **Step 4: commit** `feat(indexer): fact lookups read per-family index files`.

---

## Task 5: `symbol` CLI (--kind, --repo) + rich output; module.info over generic

**Files:** Modify `src/indexer.rs`, `src/main.rs`.

- [ ] **Step 1:** Rewrite `symbol_report` to read `SymbolDef` from `symbols.json`, take an optional `kind` filter, and print kind + signature + scope:

```rust
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
            if s.kind != k { continue; }
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
```

Update `symbol_search` to pass the filter:

```rust
pub fn symbol_search(repo: &Path, query: &str, kind: Option<&str>) -> Result<(), String> {
    println!("{}", symbol_report(repo, query, kind)?.trim_end());
    Ok(())
}
```

`brief.rs` calls `indexer::symbol_report(repo, &opts.target)` — update that call to `symbol_report(repo, &opts.target, None)`.

- [ ] **Step 2:** Update `module_report` to read `SymbolDef` (change `let symbols: Vec<Symbol>` to `Vec<SymbolDef>`; the row format already uses `s.kind`/`s.name`/`s.file`/`s.line`, all present on `SymbolDef`). It now lists functions/methods under a module.

- [ ] **Step 3:** In `src/main.rs`, give `Symbol` a `--kind` and `--repo`:

```rust
    /// Search indexed project symbols by name: `symbol <query>`.
    Symbol {
        query: String,
        /// Filter by kind (class, object, function, method, property).
        #[arg(long)]
        kind: Option<String>,
        /// Repo to search (default: active project's repo, else current dir).
        #[arg(long)]
        repo: Option<String>,
    },
```

Dispatch + handler:

```rust
        Commands::Symbol { query, kind, repo } => cmd_symbol(query, kind, repo, project),
```

```rust
fn cmd_symbol(query: String, kind: Option<String>, repo: Option<String>, project: Option<&str>) -> Result<(), String> {
    let global = GlobalConfig::load_or_default()?;
    let cwd = std::env::current_dir().map_err(|e| format!("can't determine current dir: {e}"))?;
    let repo_path = config::resolve_repo(&global, project, repo, &cwd)?;
    indexer::symbol_search(&repo_path, &query, kind.as_deref())
}
```

- [ ] **Step 4:** `cargo test && cargo build`. Smoke:
```bash
mkdir -p /tmp/plg-sym && printf 'class LoginViewModel : ViewModel() {\n  fun login(u:String){}\n  fun logout(){}\n}\n' > /tmp/plg-sym/A.kt
./target/debug/palugada index --repo /tmp/plg-sym --profile android-mvvm
./target/debug/palugada symbol login --repo /tmp/plg-sym
./target/debug/palugada symbol "" --kind method --repo /tmp/plg-sym
```
Expected: index reports `symbols 3`; `symbol login` shows `method  fun login(u:String)  ·  LoginViewModel  ·  A.kt:2`.

- [ ] **Step 5: commit** `feat(symbol): generic symbol search with --kind/--repo + scope/signature`.

---

## Task 6: Docs + final verification

**Files:** `README.md`.

- [ ] **Step 1:** README — update the `symbol` row and the indexer bullet to say it indexes **all definitions** (classes, functions, methods, properties) with kind/scope/signature, and document `symbol --kind`.

- [ ] **Step 2:** `cargo test && cargo build --release`. Final smoke on the real repo is N/A (Rust has no tags query yet); use the Kotlin fixture from Task 5 Step 4 and confirm `symbol --kind function`/`method` works and `module.info` (via `brief`) lists functions.

- [ ] **Step 3:** `rm -rf /tmp/plg-sym`; `palugada project remove` any stray test project. **Step 4: commit** `docs: document generic symbol index`.

---

## Self-review notes

- **Spec coverage:** §4 data model → T3 (symbols.json=defs, fact per-family) + T4; §5 tags/registries → T1; §6 extraction (scope/method/signature) → T2; §7 CLI → T5; §9 tests → T1/T2/T4/T5.
- **Type consistency:** `SymbolDef` (T2) is written by `run` (T3) and read by `symbol_report`/`module_report` (T5); `Symbol` stays for fact families (T3/T4). `symbol_report`/`symbol_search` gain the `kind` param consistently across `indexer.rs`, `brief.rs` (call site updated T5), and `main.rs` (T5).
- **Existing tests:** `module_report_summarises_symbols_under_prefix` keeps working (its `symbols.json` lacks scope/signature → `SymbolDef`'s `#[serde(default)]` fills them). `fact_report_filters_by_kind_and_name` updated in T4.
- **Out of scope:** references/call-graph, non-Kotlin tags, interface/enum kind distinction.
