# Rust + Flutter Profiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship two new knowledge profiles — `rust-cli` (modeled on palugada itself) and `flutter-bloc` (modeled on the bloc/cubit Clean-Architecture Flutter monorepo) — with bundled tree-sitter grammars for Rust and Dart so `palugada symbol`/`index` work for `.rs`/`.dart`.

**Architecture:** One engine change (register `tree-sitter-rust` + `tree-sitter-dart` in `src/indexer.rs`'s three registries + two `src/tags/*.scm` files); everything else is profile data files under `knowledge/profiles/{rust-cli,flutter-bloc}/` following the android-mvvm layout. Dart grammar carries a regex fallback if the community crate won't build/parse.

**Tech Stack:** Rust, `tree-sitter` 0.25, `tree-sitter-rust` 0.24, `tree-sitter-dart` 0.2, `serde_yaml`/`serde_json`, profile data (YAML + JSON + markdown).

## Global Constraints

- Profiles mirror the android-mvvm on-disk layout: `profile.yaml`, `extractors.yaml`, `conventions/{_index.json,*.md}`, `recipes/{_index.json,*.md}`, optional `extractors/*.scm`.
- `profile.yaml` schema: `schema_version: "1.0"`, `id`, `title`, `description`, `languages`, `fact_families: [{id, symbol}]`, `flows`, `review_map`.
- Profile ids: **`rust-cli`** and **`flutter-bloc`**.
- Every flow's `convention(X)`/`recipe(X)` reference and every `review_map` convention id MUST correspond to an authored doc (verified by `brief`/`profile validate` smoke — no dangling refs).
- Adding a language = Cargo dep + `src/tags/<lang>.scm` + arms in `language_for`/`language_for_ext`/`tags_query` (no schema change).
- Dart fallback: if `tree-sitter-dart` won't compile against `tree-sitter 0.25` or its tags query won't load, drop the Dart registry arms and make `flutter-bloc`'s symbol-bearing families use `regex` instead of `query` (profile still ships; `.dart` `symbols.json` is empty). Decide with evidence (the parse test), not assumption.
- CI parity: `cargo build --release` + `cargo test --release` stay green, no new warnings.
- No version bump / npm release.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.

---

### Task 1: Register the Rust tree-sitter grammar

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tags/rust.scm`
- Modify: `src/indexer.rs` (`language_for`, `language_for_ext`, `tags_query`, + test)

**Interfaces:**
- Produces: `language_for("rust")` → Ok; `language_for_ext("rs") == Some("rust")`; `tags_query("rust") == Some(RUST_TAGS)`.

- [ ] **Step 1: Add the dependency**

In `Cargo.toml` under `[dependencies]`, after `tree-sitter-kotlin-ng = "1.1"`:
```toml
tree-sitter-rust = "0.24"
```

- [ ] **Step 2: Create the tags query**

Create `src/tags/rust.scm`:
```scheme
; Generic symbol tags for Rust — one capture per definition kind.
; The capture name is the symbol kind; the captured node is the symbol name.
(struct_item name: (type_identifier) @struct)
(enum_item name: (type_identifier) @enum)
(union_item name: (type_identifier) @struct)
(trait_item name: (type_identifier) @trait)
(function_item name: (identifier) @function)
(function_signature_item name: (identifier) @function)
(const_item name: (identifier) @const)
(static_item name: (identifier) @const)
(type_item name: (type_identifier) @type)
(mod_item name: (identifier) @module)
(macro_definition name: (identifier) @macro)
```

- [ ] **Step 3: Write the failing test**

In `src/indexer.rs` test module (after `tags_registry_resolves_kotlin`):
```rust
#[test]
fn tags_registry_resolves_rust() {
    assert_eq!(language_for_ext("rs"), Some("rust"));
    let q = tags_query("rust").unwrap();
    let lang = language_for("rust").unwrap();
    assert!(tree_sitter::Query::new(&lang, q).is_ok(), "rust.scm must compile");
}

#[test]
fn extract_symbols_finds_rust_defs() {
    let src = "pub struct Config { pub a: u32 }\npub fn run(x: u32) -> u32 { x }\npub trait Host { fn ping(&self); }\n// fn ghost() {}\n";
    let mut out = Vec::new();
    extract_symbols(src, "lib.rs", "rust", &mut out);
    let by = |k: &str, n: &str| out.iter().find(|s| s.kind == k && s.name == n).cloned();
    assert!(by("struct", "Config").is_some());
    assert!(by("function", "run").is_some());
    assert!(by("trait", "Host").is_some());
    assert!(out.iter().all(|s| s.name != "ghost"), "comment fn must not be captured");
}
```

- [ ] **Step 4: Run, verify it fails**

Run: `cargo test --bin palugada tags_registry_resolves_rust extract_symbols_finds_rust`
Expected: FAIL — `unsupported language 'rust'` / `language_for_ext("rs")` returns None.

- [ ] **Step 5: Register the grammar**

In `src/indexer.rs`:
```rust
fn language_for(name: &str) -> Result<tree_sitter::Language, String> {
    match name {
        "kotlin" => Ok(tree_sitter_kotlin_ng::LANGUAGE.into()),
        "rust" => Ok(tree_sitter_rust::LANGUAGE.into()),
        other => Err(format!("unsupported language '{other}' (supported: kotlin, rust)")),
    }
}

const KOTLIN_TAGS: &str = include_str!("tags/kotlin.scm");
const RUST_TAGS: &str = include_str!("tags/rust.scm");

pub fn language_for_ext(ext: &str) -> Option<&'static str> {
    match ext {
        "kt" | "kts" => Some("kotlin"),
        "rs" => Some("rust"),
        _ => None,
    }
}

pub fn tags_query(lang: &str) -> Option<&'static str> {
    match lang {
        "kotlin" => Some(KOTLIN_TAGS),
        "rust" => Some(RUST_TAGS),
        _ => None,
    }
}
```

- [ ] **Step 6: Run, verify pass**

Run: `cargo test --bin palugada tags_registry_resolves_rust extract_symbols_finds_rust`
Expected: PASS. If a tags-query node name is wrong (`Query::new` errors), fix `rust.scm` to match `tree-sitter-rust`'s grammar (the error names the bad node) and re-run.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/tags/rust.scm src/indexer.rs
git commit -m "$(printf 'feat(indexer): bundle tree-sitter-rust grammar + generic Rust tags\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 2: Register the Dart tree-sitter grammar (with fallback gate)

**Files:**
- Modify: `Cargo.toml`
- Create: `src/tags/dart.scm`
- Modify: `src/indexer.rs` (registries + test)

**Interfaces:**
- Produces (on success): `language_for("dart")` → Ok; `language_for_ext("dart") == Some("dart")`; `tags_query("dart") == Some(DART_TAGS)`.
- On fallback: none of the above; Task 5 uses regex extractors for `flutter-bloc`.

- [ ] **Step 1: Probe the crate API + build**

Add to `Cargo.toml`:
```toml
tree-sitter-dart = "0.2"
```
Run `cargo build 2>&1 | tail -20`. If it fails to compile (incompatible tree-sitter version, missing `cc`, etc.): **FALLBACK** — remove the dep, skip Steps 2-7, note the fallback in the commit/PR, and Task 5 will use regex. If it builds, determine the language accessor: check whether the crate exposes `tree_sitter_dart::LANGUAGE` (LanguageFn) or `tree_sitter_dart::language()` (older API):
```bash
cargo doc -p tree-sitter-dart --no-deps 2>/dev/null; ls ~/.cargo/registry/src/*/tree-sitter-dart-*/src/lib.rs && grep -n "pub fn language\|pub static LANGUAGE\|pub const LANGUAGE" ~/.cargo/registry/src/*/tree-sitter-dart-*/src/lib.rs
```
Use whichever the crate exports in Step 5.

- [ ] **Step 2: Create the tags query**

Create `src/tags/dart.scm` (node names verified against the crate's `node-types.json` — adjust in Step 6 if `Query::new` errors):
```scheme
; Generic symbol tags for Dart.
(class_definition name: (identifier) @class)
(mixin_declaration (identifier) @class)
(extension_declaration name: (identifier) @class)
(enum_declaration name: (identifier) @enum)
(function_signature name: (identifier) @function)
```

- [ ] **Step 3: Write the failing test**

In `src/indexer.rs` test module:
```rust
#[test]
fn tags_registry_resolves_dart() {
    assert_eq!(language_for_ext("dart"), Some("dart"));
    let q = tags_query("dart").unwrap();
    let lang = language_for("dart").unwrap();
    assert!(tree_sitter::Query::new(&lang, q).is_ok(), "dart.scm must compile");
}

#[test]
fn extract_symbols_finds_dart_defs() {
    let src = "class CounterCubit extends Cubit<int> {\n  CounterCubit() : super(0);\n}\nclass HomePage extends StatelessWidget {}\n";
    let mut out = Vec::new();
    extract_symbols(src, "home.dart", "dart", &mut out);
    assert!(out.iter().any(|s| s.name == "CounterCubit"));
    assert!(out.iter().any(|s| s.name == "HomePage"));
}
```

- [ ] **Step 4: Run, verify it fails**

Run: `cargo test --bin palugada tags_registry_resolves_dart extract_symbols_finds_dart`
Expected: FAIL — `unsupported language 'dart'`.

- [ ] **Step 5: Register the grammar**

In `src/indexer.rs`, extend the three registries (use the accessor found in Step 1 — shown here as `LANGUAGE`):
```rust
        "dart" => Ok(tree_sitter_dart::LANGUAGE.into()),
```
(error message → `"(supported: kotlin, rust, dart)"`)
```rust
const DART_TAGS: &str = include_str!("tags/dart.scm");
```
```rust
        "dart" => Some("dart"),   // in language_for_ext
        "dart" => Some(DART_TAGS), // in tags_query
```

- [ ] **Step 6: Run, verify pass**

Run: `cargo test --bin palugada tags_registry_resolves_dart extract_symbols_finds_dart`
Expected: PASS. If `Query::new` errors on a node name, fix `dart.scm` against the grammar's actual node types (the error names the offending node — e.g. `class_definition` may be `class_declaration`). If the grammar parses but the two symbol assertions can't be satisfied with any query (grammar too limited), trigger the FALLBACK from Step 1.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/tags/dart.scm src/indexer.rs
git commit -m "$(printf 'feat(indexer): bundle tree-sitter-dart grammar + generic Dart tags\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

If FALLBACK was taken, instead commit only the (reverted) Cargo.toml state with a note, and record in the spec that `flutter-bloc` uses regex families.

---

### Task 3: Scaffold the `rust-cli` profile skeleton

**Files:**
- Create: `knowledge/profiles/rust-cli/profile.yaml`
- Create: `knowledge/profiles/rust-cli/extractors.yaml`
- Create: `knowledge/profiles/rust-cli/extractors/trait.scm`
- Create: `knowledge/profiles/rust-cli/conventions/_index.json` (empty topics for now)
- Create: `knowledge/profiles/rust-cli/recipes/_index.json` (empty recipes for now)

- [ ] **Step 1: Write profile.yaml**

Create `knowledge/profiles/rust-cli/profile.yaml`:
```yaml
schema_version: "1.0"
id: rust-cli
title: "Rust · single-binary CLI (clap)"
description: >
  Conventions for a project-agnostic single-binary Rust CLI: clap-derive command
  surface, module-per-concern, a pure-core / thin-I/O-shell split, Result<T,String>
  error propagation, and inline #[cfg(test)] tests with tempfile. Modeled on palugada.
languages: [rust]

fact_families:
  - { id: command, symbol: true }
  - { id: trait,   symbol: true }

flows:
  bugfix:   [code.recent, symbol.find, convention(errorhandling), convention(testing)]
  feature:  [recipe(feature), module.info, convention(architecture)]
  refactor: [module.info, convention(architecture), convention(style), recipe(refactor)]
  review:   [diff.scan, convention(by-file-kind)]

review_map:
  command: [architecture, errorhandling]
  trait:   [architecture, testing]
```

- [ ] **Step 2: Write extractors.yaml + trait.scm**

`knowledge/profiles/rust-cli/extractors.yaml`:
```yaml
schema_version: "1.0"
ignore_dirs: [".git", "target", ".palugada", "node_modules", "dist"]
families:
  - id: command
    ext: [rs]
    regex: 'fn\s+(?P<name>cmd_\w+)'
  - id: trait
    ext: [rs]
    language: rust
    query: extractors/trait.scm
```
`knowledge/profiles/rust-cli/extractors/trait.scm`:
```scheme
(trait_item name: (type_identifier) @name)
```

- [ ] **Step 3: Write empty index files**

`knowledge/profiles/rust-cli/conventions/_index.json`:
```json
{
  "schema_version": "1.0",
  "topics": []
}
```
`knowledge/profiles/rust-cli/recipes/_index.json`:
```json
{
  "schema_version": "1.0",
  "recipes": []
}
```

- [ ] **Step 4: Validate the skeleton**

Run: `cargo run -q -- profile validate rust-cli`
Expected: passes (profile.yaml parses, extractors compile incl. the tree-sitter trait query, indexes are valid JSON). If the trait `.scm` fails to compile, fix it against the Rust grammar.

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/rust-cli
git commit -m "$(printf 'feat(profiles): scaffold rust-cli profile (yaml + extractors)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 4: Author `rust-cli` conventions + recipes

**Files:** (all under `knowledge/profiles/rust-cli/`)
- Create: `conventions/{architecture,errorhandling,testing,style}.md`
- Create: `recipes/{feature,refactor}.md`
- Modify: `conventions/_index.json`, `recipes/_index.json` (fill entries)

**Interfaces:**
- Consumes: profile.yaml flows/review_map reference `architecture`, `errorhandling`, `testing`, `style` (conventions) and `feature`, `refactor` (recipes).

- [ ] **Step 1: Author the four conventions**

Each file uses the android-mvvm front-matter format (`--- id/title/description/sections/tags --- # Title` then `## Section {#id}` bodies). Content (write real, positive best-practice prose — these are concrete, not placeholders):

`architecture.md` — sections: `overview` (single binary; CLI = thin `main` dispatch over a `Commands` clap-derive enum; each subcommand = a `cmd_*` handler), `modules` (module-per-concern: one `mod` per capability/domain; `pub` surface minimal; files stay focused), `core-shell` (split pure logic from I/O — pure functions take/return data and are unit-tested directly; thin wrappers do fs/network/process and are exercised with tempdir/e2e), `results` (`Result<T, String>` threaded with `?`; data flows in, packs/values out). tags: `[rs, rust, cli, clap, architecture, module]`.

`errorhandling.md` — sections: `result-type` (`Result<T, String>`; user-facing strings), `context` (`.map_err(|e| format!("doing X: {e}"))` to add context at each boundary), `no-panic` (no `unwrap()`/`expect()`/`panic!`/`unreachable!` on non-test paths; use `?` or `ok_or_else`), `degrade` (best-effort steps degrade to an inline note instead of aborting the whole command). tags: `[rs, rust, error, result, anyhow]`.

`testing.md` — sections: `inline-tests` (inline `#[cfg(test)] mod tests` beside the code), `tempfile` (`tempfile::tempdir()` for filesystem isolation), `pure-first` (test pure transforms directly with literals; reserve tempdir/e2e for I/O), `no-mocks` (prefer real fixtures/fakes over mocking; assert on outcomes). tags: `[rs, rust, testing, cargo-test, tempfile]`.

`style.md` — sections: `fmt-clippy` (`cargo fmt` + `cargo clippy` clean; treat clippy as the linter of record), `naming` (snake_case fns/modules, CamelCase types/traits, SCREAMING_SNAKE consts), `docs` (`///` doc-comments on public items; module-level `//!`), `signatures` (prefer `&str`/`&[T]`/`impl Trait`; keep `pub` minimal). tags: `[rs, rust, style, rustfmt, clippy, naming]`.

- [ ] **Step 2: Author the two recipes**

`recipes/feature.md` — front-matter (`id/title/description/references/tags`) + body "Recipe: Add a subcommand": numbered steps — (1) add a variant to the `Commands` clap enum with its args; (2) write a `cmd_<verb>(...) -> Result<(), String>` handler in the owning module (or a new `mod`); (3) wire it into the dispatch `match` in `main`; (4) add inline `#[cfg(test)]` for the pure parts; (5) `cargo test` + `cargo run -- <verb> --help`. tags: `[feature, subcommand, clap]`.

`recipes/refactor.md` — body "Recipe: Extract a pure helper": (1) identify the pure logic tangled inside an I/O function; (2) lift it into a `fn` taking/returning data; (3) write a unit test against it first (TDD); (4) call it from the original site; (5) `cargo test` stays green; note splitting an overgrown module into focused ones. tags: `[refactor, extract, tdd]`.

- [ ] **Step 3: Fill the index files**

`conventions/_index.json` — `topics` array with one entry per convention: `{ id, title, file: "<id>.md", description, tags, sections: [{id, title, tokens}] }` matching each file's front-matter. `recipes/_index.json` — `recipes` array: `{ id, title, description, file: "<id>.md", tags }`.

- [ ] **Step 4: Validate + smoke (no dangling refs)**

Run:
```bash
cargo run -q -- profile validate rust-cli
cargo run -q -- q --list --profile rust-cli
```
Expected: validate passes; `q --list` shows the 4 conventions. Then smoke a brief that pulls them (run from the palugada repo, which is Rust):
```bash
cargo run -q -- brief refactor config --profile rust-cli 2>/dev/null | grep -E "convention: architecture|convention: style"
```
Expected: renders the `architecture` and `style` convention outlines (proves flow refs resolve — no "(no convention 'x')").

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/rust-cli
git commit -m "$(printf 'feat(profiles): author rust-cli conventions + recipes\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 5: Scaffold the `flutter-bloc` profile skeleton

**Files:**
- Create: `knowledge/profiles/flutter-bloc/profile.yaml`
- Create: `knowledge/profiles/flutter-bloc/extractors.yaml`
- Create: `knowledge/profiles/flutter-bloc/extractors/{cubit,state,page,repository,datasource}.scm` (only if Dart grammar bundled in Task 2; else skip and use regex in extractors.yaml)
- Create: `knowledge/profiles/flutter-bloc/conventions/_index.json`, `recipes/_index.json` (empty)

- [ ] **Step 1: Write profile.yaml**

Create `knowledge/profiles/flutter-bloc/profile.yaml`:
```yaml
schema_version: "1.0"
id: flutter-bloc
title: "Flutter · bloc/cubit · Clean Architecture (melos)"
description: >
  Conventions for a feature-first Clean-Architecture Flutter monorepo: workspace
  feature packages with data/domain/presentation layers, flutter_bloc Cubits with
  sealed-style State classes, GetIt DI per feature, and GoRouter. Tests via
  bloc_test + mocktail.
languages: [dart]

fact_families:
  - { id: cubit,      symbol: true }
  - { id: state,      symbol: true }
  - { id: page,       symbol: true }
  - { id: repository, symbol: true }
  - { id: datasource, symbol: true }
  - { id: route,      symbol: false }

flows:
  bugfix:   [code.recent, symbol.find, convention(statemanagement), convention(testing)]
  feature:  [recipe(feature), module.info, convention(architecture)]
  refactor: [module.info, convention(architecture), convention(style), recipe(refactor)]
  review:   [diff.scan, convention(by-file-kind)]

review_map:
  cubit:      [architecture, statemanagement, testing]
  state:      [statemanagement]
  page:       [architecture, style]
  repository: [architecture, errorhandling, testing]
  datasource: [architecture, errorhandling]
  route:      [architecture]
```

- [ ] **Step 2: Write extractors.yaml (+ .scm if Dart bundled)**

**If Task 2 bundled Dart:** create `extractors.yaml`:
```yaml
schema_version: "1.0"
ignore_dirs: [".git", ".dart_tool", "build", ".palugada", "node_modules", ".idea"]
families:
  - { id: cubit,      ext: [dart], path_contains: cubit,        language: dart, query: extractors/cubit.scm }
  - { id: state,      ext: [dart], path_contains: cubit,        language: dart, query: extractors/state.scm }
  - { id: page,       ext: [dart], path_contains: ui,           language: dart, query: extractors/page.scm }
  - { id: repository, ext: [dart], path_contains: repositories, language: dart, query: extractors/repository.scm }
  - { id: datasource, ext: [dart], path_contains: datasources,  language: dart, query: extractors/datasource.scm }
  - id: route
    ext: [dart]
    path_contains: routes
    regex: 'static\s+const\s+(?:String\s+)?(?P<name>\w+)\s*='
```
Each of `extractors/{cubit,state,page,repository,datasource}.scm` contains:
```scheme
(class_definition name: (identifier) @name)
```
(The `path_contains` + family id encode the kind; v1 captures all classes in that path. A `(#match? @name "Cubit$")` predicate can tighten later.)

**If Task 2 took the FALLBACK (no Dart grammar):** instead use regex per family — replace each `language: dart, query: …` with a class-suffix regex:
```yaml
  - { id: cubit,      ext: [dart], regex: 'class\s+(?P<name>\w+Cubit)\b' }
  - { id: state,      ext: [dart], regex: 'class\s+(?P<name>\w+(State|Initial|Loading|Loaded|Error))\b' }
  - { id: page,       ext: [dart], regex: 'class\s+(?P<name>\w+Page)\b' }
  - { id: repository, ext: [dart], regex: 'class\s+(?P<name>\w+Repository(Impl)?)\b' }
  - { id: datasource, ext: [dart], regex: 'class\s+(?P<name>\w+DataSource(Impl)?)\b' }
  - { id: route,      ext: [dart], path_contains: routes, regex: 'static\s+const\s+(?:String\s+)?(?P<name>\w+)\s*=' }
```

- [ ] **Step 3: Write empty index files**

`conventions/_index.json` → `{ "schema_version": "1.0", "topics": [] }`;
`recipes/_index.json` → `{ "schema_version": "1.0", "recipes": [] }`.

- [ ] **Step 4: Validate the skeleton**

Run: `cargo run -q -- profile validate flutter-bloc`
Expected: passes (extractors compile — tree-sitter queries load or regex compiles; indexes valid).

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/flutter-bloc
git commit -m "$(printf 'feat(profiles): scaffold flutter-bloc profile (yaml + extractors)\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 6: Author `flutter-bloc` conventions + recipes

**Files:** (under `knowledge/profiles/flutter-bloc/`)
- Create: `conventions/{architecture,statemanagement,errorhandling,testing,style}.md`
- Create: `recipes/{feature,refactor}.md`
- Modify: `conventions/_index.json`, `recipes/_index.json`

**Interfaces:**
- Consumes: profile.yaml flows/review_map reference conventions `architecture`, `statemanagement`, `errorhandling`, `testing`, `style` and recipes `feature`, `refactor`.

- [ ] **Step 1: Author the five conventions**

`architecture.md` — sections: `overview` (feature-first Clean Architecture in a workspace monorepo; each feature is a package), `layers` (`data/` = datasources + repository impls; `domain/` = repository abstractions + entities; `presentation/` = `ui/` + `cubit/`), `di` (GetIt service locator; each feature exposes `register<Feature>()`; root `service_locator.dart` aggregates), `routing` (GoRouter; route names centralized in `libraries/shared` `named_routes.dart`), `barrels` (each feature's `<feature>.dart` barrel exports its public API). tags: `[dart, flutter, architecture, clean-architecture, getit, gorouter, melos]`.

`statemanagement.md` — sections: `cubit` (`flutter_bloc` `Cubit<State>`; one Cubit per screen/feature concern), `state-classes` (sealed-style: abstract `*State` + `*Initial`/`*Loading`/`*Loaded`/`*Error` subclasses with `Equatable`), `emit` (transition via `emit(...)`; immutable states), `consume` (`BlocProvider` to inject, `BlocBuilder`/`BlocListener` in UI; widgets stay logic-free). tags: `[dart, flutter, bloc, cubit, state, equatable]`.

`errorhandling.md` — sections: `cubit-catch` (`try/catch` in the Cubit method → `emit(*Error(message))`), `no-throw-to-ui` (never let exceptions reach the widget tree), `repository-errors` (repositories surface typed/wrapped failures; Cubits translate them to error states), `messages` (user-facing error copy lives in the state, not thrown strings). tags: `[dart, flutter, error, state, exception]`.

`testing.md` — sections: `bloc-test` (`bloc_test` — `build`/`act`/`expect` the state sequence), `mocktail` (`mocktail` for repositories/datasources; register fallbacks), `fakes` (prefer fakes over mocks for value-ish deps), `widget-tests` (golden/widget tests for pages; keep framework out of pure-logic tests). tags: `[dart, flutter, testing, bloc_test, mocktail]`.

`style.md` — sections: `lints` (`flutter_lints` analysis_options; treat warnings as signal), `files` (snake_case filenames; one public class per file typically), `suffixes` (role suffixes: `Page`/`View`/`Cubit`/`State`/`Repository`/`RepositoryImpl`/`DataSource`/`DataSourceImpl`), `structure` (package-per-feature; const constructors where possible). tags: `[dart, flutter, style, flutter_lints, naming]`.

- [ ] **Step 2: Author the two recipes**

`recipes/feature.md` — "Recipe: Scaffold a feature package": (1) add a workspace member `features/<name>/pubspec.yaml`; (2) create `data/{datasources,repositories}`, `domain/repositories`, `presentation/{ui,cubit}`; (3) write the `*Cubit` + `*State` classes; (4) the `*Page` widget using `BlocProvider`/`BlocBuilder`; (5) a `register<Name>()` adding bindings to GetIt; (6) a route constant + `GoRoute`; (7) export from the feature barrel. tags: `[feature, scaffold, clean-architecture]`.

`recipes/refactor.md` — "Recipe: Extract a widget / split a Cubit": (1) pull a sub-tree out of a `*Page` into its own `*View` widget; or (2) split a Cubit whose `State` grew too many variants into focused Cubits; (3) keep `bloc_test`s green; (4) update DI registration. tags: `[refactor, widget, cubit]`.

- [ ] **Step 3: Fill the index files**

Same structure as Task 4 Step 3 — one `topics` entry per convention (with `sections`), one `recipes` entry per recipe, matching each file's front-matter.

- [ ] **Step 4: Validate + smoke (no dangling refs)**

Run:
```bash
cargo run -q -- profile validate flutter-bloc
cargo run -q -- q --list --profile flutter-bloc
```
Expected: validate passes; `q --list` shows the 5 conventions. (A full `brief` render is exercised in Task 7's e2e against the real Flutter repo, since `brief` needs a repo.)

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/flutter-bloc
git commit -m "$(printf 'feat(profiles): author flutter-bloc conventions + recipes\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

### Task 7: End-to-end verification

**Files:** none (verification only; any fixups committed).

- [ ] **Step 1: Full build + test**

Run: `cargo test --release && cargo build --release`
Expected: all tests pass (≥93 + new grammar tests), no warnings, release builds.

- [ ] **Step 2: Live e2e — rust-cli against the palugada repo**

Use a scratch copy so the real repo's `.palugada/` is untouched, OR run in-place and restore. Recommended (scratch):
```bash
BIN="$PWD/target/release/palugada"
cp -R . /private/tmp/rustcli-e2e 2>/dev/null; cd /private/tmp/rustcli-e2e
PALUGADA_KNOWLEDGE="$OLDPWD/knowledge" "$BIN" init --repo . --profile rust-cli --agents claude
PALUGADA_KNOWLEDGE="$OLDPWD/knowledge" "$BIN" index --profile rust-cli
PALUGADA_KNOWLEDGE="$OLDPWD/knowledge" "$BIN" symbol cmd_brief --profile rust-cli   # finds the handler
PALUGADA_KNOWLEDGE="$OLDPWD/knowledge" "$BIN" fact command --profile rust-cli       # lists cmd_* commands
PALUGADA_KNOWLEDGE="$OLDPWD/knowledge" "$BIN" fact trait --profile rust-cli         # lists capability traits
cd "$OLDPWD"; rm -rf /private/tmp/rustcli-e2e
```
Expected: `index` reports symbols + command/trait facts; `symbol cmd_brief` returns a hit with file:line; `fact command` lists `cmd_*`; `fact trait` lists traits (IssueTracker, CiProvider, etc.). (Confirm the exact `init`/`index` flag names against `--help`; adjust.)

- [ ] **Step 3: Live e2e — flutter-bloc against the Flutter repo**

```bash
FLUTTER="/Users/septiandwisaputro/Documents/learn/private project/flutter"
BIN="$PWD/target/release/palugada"
# back up the project's .palugada if present, register + init in a scratch copy or in-place with restore
"$BIN" project add flutter-e2e "$FLUTTER"
"$BIN" init --repo "$FLUTTER" --profile flutter-bloc --agents auto
"$BIN" index --project flutter-e2e
"$BIN" symbol SampleCubit --project flutter-e2e     # or CounterCubit
"$BIN" fact cubit --project flutter-e2e
"$BIN" fact page --project flutter-e2e
"$BIN" brief feature detail --project flutter-e2e 2>/dev/null | head -30
```
Expected: `symbol`/`fact cubit`/`fact page` return hits (or, on Dart fallback, the regex families still return hits and `symbols.json` is empty — note which). `brief feature` renders the feature recipe + architecture convention.
**Cleanup:** `"$BIN" project remove flutter-e2e`; restore/remove the Flutter repo's `.palugada/` and any generated `CLAUDE.md`/`.claude/`/`.agents/` so the user's project is left exactly as found (use `git -C "$FLUTTER" status` to see what was added; remove untracked palugada artifacts).

- [ ] **Step 4: Record Dart outcome**

If the Dart fallback was taken in Task 2, add one line to the spec's Risk section noting `flutter-bloc` ships with regex families (no `.dart` symbols.json) and that bundling `tree-sitter-dart` is deferred. Commit:
```bash
git add docs/superpowers/specs/2026-06-19-rust-flutter-profiles-design.md
git commit -m "$(printf 'docs: record Dart grammar outcome for flutter-bloc\n\nCo-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>')"
```

---

## Self-review

**Spec coverage:**
- Tree-sitter wiring (Rust) → Task 1. (Dart + fallback) → Task 2.
- `rust-cli` profile (yaml/extractors/conventions/recipes) → Tasks 3-4.
- `flutter-bloc` profile → Tasks 5-6.
- Tests (registry resolve, symbol extraction, profile validate, e2e) → each task's steps + Task 7.
- Dart risk + fallback → Task 2 gate + Task 5 alternate extractors + Task 7 Step 4.

**Placeholder scan:** Convention/recipe steps give concrete section ids + substantive content guidance (not "add content"). The only deferred specifics are tree-sitter node names (gated by the `Query::new` compile test) and exact CLI flag names (gated by `--help`) — both flagged with how to resolve, not left blank.

**Type consistency:** `RUST_TAGS`/`DART_TAGS` consts + the three registry fns (`language_for`/`language_for_ext`/`tags_query`) used consistently across Tasks 1-2. Convention/recipe ids referenced in each profile.yaml (Tasks 3,5) exactly match the files authored (Tasks 4,6): rust-cli {architecture,errorhandling,testing,style}+{feature,refactor}; flutter-bloc {architecture,statemanagement,errorhandling,testing,style}+{feature,refactor}. `extract_symbols(text, rel, lang, &mut out)` signature matches the existing function.
