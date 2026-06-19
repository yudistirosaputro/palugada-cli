# Rust (`rust-cli`) and Flutter (`flutter-bloc`) knowledge profiles

**Date:** 2026-06-19
**Status:** approved (design)
**Branch:** `feat/rust-flutter-profiles`

## Problem

palugada ships exactly one profile — `android-mvvm` (Kotlin, with a bundled
tree-sitter grammar). The tool is profile-agnostic, but a project written in
Rust or Flutter has nothing to bind to: no conventions, no fact families, no
symbol extraction. The user wants a profile for **Rust** (modeled on palugada
itself — a single-binary `clap` CLI) and one for **Flutter** (modeled on the
bloc/cubit Clean-Architecture monorepo at
`/Users/septiandwisaputro/Documents/learn/private project/flutter`).

Authoring a profile means: a `profile.yaml` (flows + `review_map` + declared
fact families), an `extractors.yaml` (regex or tree-sitter `.scm` per family),
`conventions/` + `recipes/` (markdown + `_index.json`), and — for real symbol
search (`palugada symbol`) — a tree-sitter grammar registered in the palugada
binary. android-mvvm bundles `tree-sitter-kotlin-ng`; the generated agent skills
enforce "use `palugada symbol`/`fact` before grep", so a profile without symbol
extraction undercuts that rule.

## Goal

Add two profiles with **full parity** to android-mvvm — including bundled
tree-sitter grammars (`tree-sitter-rust`, `tree-sitter-dart`) so `palugada index`
builds a real `symbols.json` for `.rs` and `.dart`, and `brief`/`fact`/`symbol`
work. Each profile gets curated conventions, recipes, fact families, flows, and a
`review_map`.

Non-goals (YAGNI): per-profile custom skills (the standard generated set
suffices); a `web-react` profile (still deferred); migrating android-mvvm; new
flow *kinds* (reuse `code.recent`/`symbol.find`/`module.info`/`diff.scan`/
`convention`/`recipe`/`prd.context`).

## Approach

Two layers:

1. **Shared engine wiring** (palugada source) — register Rust and Dart grammars
   so the generic tags-query symbol index works for `.rs`/`.dart`. This is the
   only Rust *code* change; everything else is profile data files.
2. **Profile data** — author `knowledge/profiles/rust-cli/` and
   `knowledge/profiles/flutter-bloc/` following the android-mvvm layout exactly.

### Dart grammar risk + fallback

`tree-sitter-dart` is a community crate (v0.2.0). If it fails to compile against
the bundled `tree-sitter = "0.25"` or its tags query won't load, the
`flutter-bloc` profile **degrades to regex fact families** (no `symbols.json` for
`.dart`) and still ships; Rust is unaffected. The implementation verifies the
Dart grammar builds/parses before committing to the `.scm` path; on failure it
drops the Dart registry arms and the `flutter-bloc` extractors use `regex`
instead of `query` for the symbol-bearing families. This decision is made at
implementation time with evidence (a fixture parse), not assumed now.

## Components

### 1. Engine: tree-sitter wiring (`Cargo.toml`, `src/indexer.rs`, `src/tags/`)

- `Cargo.toml`: add `tree-sitter-rust = "0.24"` and `tree-sitter-dart = "0.2"`.
- `src/tags/rust.scm` — generic Rust tags (one capture per kind):
  ```scheme
  (struct_item name: (type_identifier) @struct)
  (enum_item name: (type_identifier) @enum)
  (trait_item name: (type_identifier) @trait)
  (function_item name: (identifier) @function)
  (function_signature_item name: (identifier) @function)
  (const_item name: (identifier) @const)
  (static_item name: (identifier) @const)
  (macro_definition name: (identifier) @macro)
  (mod_item name: (identifier) @module)
  (type_item name: (type_identifier) @type)
  ```
- `src/tags/dart.scm` — generic Dart tags:
  ```scheme
  (class_definition name: (identifier) @class)
  (mixin_declaration (identifier) @class)
  (extension_declaration name: (identifier) @class)
  (enum_declaration name: (identifier) @enum)
  (function_signature name: (identifier) @function)
  (method_signature (function_signature name: (identifier) @method))
  (declaration (constant_constructor_signature (identifier) @constructor))
  ```
  (Exact node names verified against `tree-sitter-dart`'s `node-types.json`
  during implementation — adjusted to whatever the grammar actually exposes; the
  test in §4 is the gate.)
- `src/indexer.rs` — add arms to the three registries:
  - `language_for("rust")` → `tree_sitter_rust::LANGUAGE.into()`;
    `language_for("dart")` → `tree_sitter_dart::LANGUAGE.into()`.
  - `language_for_ext`: `"rs" => Some("rust")`, `"dart" => Some("dart")`.
  - `tags_query`: `"rust" => Some(RUST_TAGS)`, `"dart" => Some(DART_TAGS)` with
    `const RUST_TAGS: &str = include_str!("tags/rust.scm");` (and `DART_TAGS`).
  - Update the `unsupported language` error message to list kotlin, rust, dart.

### 2. Profile `rust-cli` (`knowledge/profiles/rust-cli/`)

**profile.yaml**
```yaml
schema_version: "1.0"
id: rust-cli
title: "Rust · single-binary CLI (clap)"
description: >
  Conventions for a project-agnostic single-binary Rust CLI: clap-derive command
  surface, module-per-concern, a pure-core / thin-I/O-shell split, Result<T,String>
  error propagation, and inline #[cfg(test)] tests with tempfile. Modeled on
  palugada itself.
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

**extractors.yaml**
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
`extractors/trait.scm`: `(trait_item name: (type_identifier) @name)`.

**conventions/** (4, each markdown + `_index.json` entry; ~3-4 sections each):
- `architecture` — single binary; `mod`-per-concern; pure functions (testable, no
  I/O) separated from thin I/O wrappers; `Result<T, String>` threaded with `?`;
  clap-derive `Commands` enum + `cmd_*` handlers; small focused files.
- `errorhandling` — `Result<T, String>`; `.map_err(|e| format!("context: {e}"))`;
  no `unwrap()`/`expect()`/`panic!` on non-test paths; surface clear user-facing
  messages; degrade-to-note for best-effort steps.
- `testing` — inline `#[cfg(test)] mod tests` next to the code; `tempfile::tempdir`
  for filesystem; test pure transforms directly, tempdir for I/O; no mocking,
  prefer real fixtures; assert on outcomes.
- `style` — `cargo fmt` + `cargo clippy` clean; snake_case fns/modules,
  CamelCase types; doc-comments (`///`) on public items; prefer `&str`/slices in
  signatures; keep `pub` surface minimal.

**recipes/** (2):
- `feature` — add a subcommand: add a `Commands` variant, a `cmd_*` handler, a new
  `mod`, and inline tests; wire into the dispatch `match`.
- `refactor` — extract a pure helper out of an I/O function (or split a module
  that grew too large) while keeping tests green; TDD the extracted unit.

**review_map** → see profile.yaml (command, trait).

### 3. Profile `flutter-bloc` (`knowledge/profiles/flutter-bloc/`)

**profile.yaml**
```yaml
schema_version: "1.0"
id: flutter-bloc
title: "Flutter · bloc/cubit · Clean Architecture (melos)"
description: >
  Conventions for a feature-first Clean-Architecture Flutter monorepo: workspace
  feature packages with data/domain/presentation layers, flutter_bloc Cubits with
  sealed-style State classes, GetIt DI registered per feature, and GoRouter. Tests
  via bloc_test + mocktail.
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

**extractors.yaml** — symbol-bearing families use tree-sitter when Dart is
bundled; `route` stays regex. (If the Dart grammar fallback triggers, the
symbol-bearing families switch to `regex` on the class-name suffix.)
```yaml
schema_version: "1.0"
ignore_dirs: [".git", ".dart_tool", "build", ".palugada", "node_modules", ".idea"]
families:
  - id: cubit
    ext: [dart]
    path_contains: "cubit"
    language: dart
    query: extractors/cubit.scm        # class_definition name @name (suffix Cubit)
  - id: state
    ext: [dart]
    path_contains: "cubit"
    language: dart
    query: extractors/state.scm        # class_definition name @name (suffix State/Initial/Loading/Loaded/Error)
  - id: page
    ext: [dart]
    path_contains: "ui"
    language: dart
    query: extractors/page.scm         # class_definition name @name (suffix Page)
  - id: repository
    ext: [dart]
    path_contains: "repositories"
    language: dart
    query: extractors/repository.scm
  - id: datasource
    ext: [dart]
    path_contains: "datasources"
    language: dart
    query: extractors/datasource.scm
  - id: route
    ext: [dart]
    path_contains: "routes"
    regex: 'static\s+const\s+(?:String\s+)?(?P<name>\w+)\s*='
```
The `.scm` files each capture `(class_definition name: (identifier) @name)`; the
`path_contains` + family id encode the kind. (Suffix-precise filtering — e.g. only
`*Cubit` — is a refinement; v1 relies on path + class capture. A predicate like
`(#match? @name "Cubit$")` can tighten later.) If the Dart grammar is dropped, each
symbol-bearing family becomes `regex: 'class\s+(?P<name>\w+Cubit)\b'` etc.

**conventions/** (5):
- `architecture` — feature-first Clean Architecture in a workspace monorepo; each
  feature is a package with `data/` (datasources, repository impls), `domain/`
  (repository abstractions), `presentation/` (`ui/`, `cubit/`); GetIt DI with a
  per-feature `register<Feature>()`; GoRouter via a shared `named_routes.dart`;
  barrel `.dart` exports per feature; `libraries/{dependencies,shared}` for
  cross-cutting deps and utilities.
- `statemanagement` — `flutter_bloc` Cubit; model screen state as sealed-style
  `State` classes (`*Initial`/`*Loading`/`*Loaded`/`*Error`) extending an abstract
  base with `Equatable`; `emit()` transitions; consume via `BlocProvider` +
  `BlocBuilder`; keep UI logic-free.
- `errorhandling` — `try/catch` in the Cubit → `emit(*Error(message))`; never let
  exceptions reach the widget tree; repositories throw typed/wrapped errors,
  Cubits translate to states.
- `testing` — `bloc_test` for Cubits (seed/act/expect), `mocktail` for
  repositories/datasources, prefer fakes; widget tests for pages; keep framework
  out of pure-logic tests.
- `style` — `flutter_lints`; `snake_case` filenames; role suffixes
  (`Page`/`View`/`Cubit`/`State`/`Repository`/`RepositoryImpl`/`DataSource`);
  package-per-feature; barrel exports; const constructors where possible.

**recipes/** (2):
- `feature` — scaffold a feature package: add a workspace member `pubspec.yaml`,
  `data/domain/presentation` layers, a `Cubit` + `State`, a `Page`, a
  `register<Feature>()` in DI, and a route constant + `GoRoute`.
- `refactor` — extract a widget from a page, or split a Cubit whose state grew too
  large, keeping `bloc_test`s green.

### 4. `_index.json` files

Each profile's `conventions/_index.json` and `recipes/_index.json` list every
authored doc with `id`/`title`/`file`/`description`/`tags` (+ `sections` for
conventions). These are authored alongside the markdown so `palugada q --list`,
keyword search, and the web console see correct metadata. (Convention `tags`
include the file-kind/family tokens used by search.)

## Testing

- **Engine unit (`src/indexer.rs`)**: `language_for("rust")`/`language_for("dart")`
  return Ok; `language_for_ext("rs")=="rust"`, `("dart")=="dart"`; `tags_query`
  returns the embedded `.scm` for both. A test parses a tiny in-memory `.rs`
  source and asserts `symbols.json`-style extraction finds a known `fn`/`struct`;
  same for a tiny `.dart` source finds a `class` (this is the **gate** that
  confirms the grammars actually work — Dart fallback decision hinges on it).
- **Profile validation**: `palugada profile validate rust-cli` and
  `flutter-bloc` succeed (extractors compile, `.scm` queries load, `_index.json`
  parse, `fact_families` declared). Add a Rust test invoking `profile::validate`
  on each new profile dir.
- **Manual e2e — rust-cli**: in the palugada repo, `palugada init --profile
  rust-cli` (in a scratch copy or with cleanup), `palugada index`, then
  `palugada symbol cmd_brief` finds the handler, `palugada fact command` lists
  `cmd_*`, `palugada fact trait` lists capability traits, `palugada brief refactor
  config` renders the architecture/style conventions. Clean up afterward.
- **Manual e2e — flutter-bloc**: register the flutter project, `init --profile
  flutter-bloc`, `index`, `palugada symbol SampleCubit` / `fact cubit` / `fact
  page` return hits, `brief feature detail` renders the feature recipe +
  architecture. Clean up afterward (restore the project's `.palugada/`).
- **CI parity**: `cargo build --release` + `cargo test --release` stay green; no
  new warnings.

## File structure

| Path | Action | Responsibility |
|---|---|---|
| `Cargo.toml` | Modify | add tree-sitter-rust + tree-sitter-dart deps |
| `src/tags/rust.scm` | Create | generic Rust tags query |
| `src/tags/dart.scm` | Create | generic Dart tags query |
| `src/indexer.rs` | Modify | register rust/dart in 3 registries (+ test) |
| `knowledge/profiles/rust-cli/**` | Create | profile.yaml, extractors.yaml, conventions(4)+recipes(2)+_index.json, extractors/trait.scm |
| `knowledge/profiles/flutter-bloc/**` | Create | profile.yaml, extractors.yaml, conventions(5)+recipes(2)+_index.json, extractors/*.scm |

## Risk / notes

- **Dart grammar compat** is the only real risk; mitigated by the parse-gate test
  and the regex fallback (flutter-bloc still ships either way).
- `tree-sitter-rust`/`tree-sitter-dart` expose `LANGUAGE: LanguageFn` via the
  version-independent `tree-sitter-language` crate (same pattern as
  `tree-sitter-kotlin-ng`), so they should link against `tree-sitter = "0.25"`;
  the build is the proof.
- Binary grows by two grammars (~hundreds of KB each) — acceptable; matches the
  android-mvvm precedent.
- These are the first profiles beyond android-mvvm; if anything in the engine
  assumes a single profile, surface it (none expected — profile is a config flip).
- No version bump / npm release as part of this work (per the user).
