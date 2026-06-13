# Design — tree-sitter extraction (hybrid, Kotlin first)

> **Status:** Approved for planning · **Date:** 2026-06-13
> **Scope:** PRD §4.3 — replace coarse regex extraction with structural
> tree-sitter queries, declaratively per fact-family, starting with Kotlin
> (the `android-mvvm` profile). Regex stays available for the long tail.

## 1. Problem

The indexer extracts fact families with line-oriented regexes
(`extractors.yaml`). Regex reads source as raw text, so it cannot tell a real
declaration from the same words in a comment or string — e.g. the current
`viewmodel` rule `class\s+(?P<name>\w+)[^\n]*\bViewModel\b` matches
`// class FooViewModel removed` and `val s = "class ViewModel"`. It also can't
follow structure (a class's supertypes, a function's enclosing type). PRD §4.3
calls for structural extraction via tree-sitter queries, declarative-first, with
regex/XML kept for the long tail.

## 2. Goals

- A **hybrid** extraction engine: a fact family declares **either** a regex
  **or** a tree-sitter query; both coexist in one `extractors.yaml`.
- Bundle the **Kotlin** grammar and migrate the `android-mvvm` families that are
  structural (`viewmodel`, `service`, `repository`) to `.scm` queries.
- Keep `symbols.json` output byte-compatible in shape (`name`/`kind`/`file`/
  `line`) so `symbol`, `fact`, `module.info`, and `brief` are unaffected.
- Adding a new language later = add one grammar crate + one registry line; no
  engine rewrite, no profile-schema change.

## 3. Non-goals

- **Other grammars** (Swift, TS, Go, Python …) — Kotlin only in v1; the engine
  is built to extend, but no other grammar ships now.
- **Migrating `route` and `i18n`** off regex — annotation-argument and XML-string
  extraction are exactly the long tail regex serves well; they stay regex.
- **Removing regex** — regex remains a first-class family kind, not deprecated.
- **The `plugin:<name>` (WASM/subprocess) escape hatch** from PRD §4.3 — deferred.
- Changing `symbols.json` schema, `fact`/`brief` behaviour, or the CLI surface.

## 4. Architecture

### 4.1 Hybrid engine, parse-once-per-file

The file walk is unchanged in shape: walk the repo, skip `ignore_dirs`, and for
each file compute the **applicable** families with the existing
`family_matches` (ext + `path_contains`). The change is what runs per file:

- **Regex families** run as today: read the file text, run each regex, capture
  the `name` group.
- **tree-sitter families**: if any applicable family for this file uses
  tree-sitter, the file is **parsed once** with the grammar for its declared
  `language`, and every such family's query runs against that one tree. (A file
  is a single language by extension, so at most one parse per file.)

A file with both regex and tree-sitter families gets one text read and (if
needed) one parse; results from both merge into the same `symbols` vector.

### 4.2 `CompiledFamily` carries an extractor enum

`CompiledFamily` keeps `id` / `ext` / `path_contains` (so `family_matches` and
`families_for_path` are **unchanged**) and replaces the bare `re: Regex` with:

```rust
enum Extractor {
    Regex(regex::Regex),
    TreeSitter { language: String, query: tree_sitter::Query },
}
```

### 4.3 Language registry

A small function maps a `language` string to a grammar:

```rust
fn language_for(name: &str) -> Result<tree_sitter::Language, String> {
    match name {
        "kotlin" => Ok(tree_sitter_kotlin_ng::LANGUAGE.into()),
        other => Err(format!("unsupported language '{other}' (supported: kotlin)")),
    }
}
```

Adding a language later = add its crate + one arm here. No profile-schema change.

### 4.4 Capture convention

A tree-sitter query **must** contain a capture named `@name` (validated at
load). Per match: the text of the `@name` node is the symbol name, its
start row + 1 is the line, and `kind` is the family `id` — identical to the
regex `(?P<name>…)` convention. Output `symbols.json` is unchanged, so all
downstream consumers (`symbol`, `fact`, `module.info`, `brief`) keep working.

## 5. `extractors.yaml` schema

Each family declares **exactly one** extractor — `regex`, or `query` + `language`
— alongside the existing `ext` / `path_contains` filters:

```yaml
families:
  - id: viewmodel              # tree-sitter
    ext: [kt]
    language: kotlin
    query: extractors/viewmodel.scm   # path relative to the profile dir
  - id: route                  # regex (unchanged)
    ext: [kt]
    regex: '@Route\s*\(\s*path\s*=\s*"(?P<name>[^"]+)"'
```

Validation at load (in `compile_families`), each a hard error like an invalid
regex today:
- neither `regex` nor `query` set → error;
- both `regex` and `query` set → error;
- `query` set without `language` (or unknown `language`) → error;
- `query` file missing / query fails to compile / has no `@name` capture → error.

`.scm` files live under `<profile>/extractors/` and are read relative to the
profile directory (resolved from the same `kn` knowledge dir the loader already
uses).

## 6. Dependencies & build/CI

Add `tree-sitter` (core) and `tree-sitter-kotlin-ng`, with the core **pinned to
the version the grammar crate's ABI targets** (resolved at implementation by
trying the build; the grammar's `LANGUAGE` constant ties them). `cc` is already
in the dependency tree (via `ring`/`aws-lc-sys`), and the release workflow
builds **each target natively on its own runner** (no musl/static-cross
container), so C compilation of the grammar needs no new toolchain and the
prebuilt-binary pipeline is structurally unchanged. Costs: longer build and a
larger binary (~hundreds of KB for the Kotlin grammar). `Cargo.lock` is committed
so CI's `--locked` build stays reproducible.

## 7. `android-mvvm` migration

Move the structural families to `.scm` under
`knowledge/profiles/android-mvvm/extractors/`; `route` and `i18n` stay regex in
`extractors.yaml`.

| Family | Old regex | New `.scm` (intent) |
|---|---|---|
| `viewmodel` | `class\s+(?P<name>\w+)[^\n]*\bViewModel\b` | class whose name ends `ViewModel` |
| `service` | `interface\s+(?P<name>\w+Service)\b` | class/interface whose name ends `Service` |
| `repository` | `class\s+(?P<name>\w+RepositoryImpl)\b` | class whose name ends `RepositoryImpl` |

Example `extractors/viewmodel.scm`:

```scheme
(class_declaration (type_identifier) @name (#match? @name "ViewModel$"))
```

**Semantic shift (intentional):** the queries filter on the **declared name**
suffix rather than matching the whole declaration line. This is cleaner and
avoids comment/string false positives; in practice MVVM types follow the
`XxxViewModel` / `XxxService` / `XxxRepositoryImpl` naming, so coverage is
equivalent or better.

**Node-name verification:** exact grammar node/field names (`class_declaration`,
`type_identifier`, whether a `name:` field exists, how interfaces are
represented) are confirmed at implementation against `tree-sitter-kotlin-ng`'s
`node-types.json` / `tree-sitter parse` output, then the queries are written to
match. Kotlin interfaces may parse as a `class_declaration` variant; the
`service` query is adjusted accordingly once verified.

## 8. Error handling

- Load-time (above) → hard error, naming the family, like a bad regex today.
- A file tree-sitter fails to fully parse → tree-sitter is error-recovering; the
  query still runs over the partial tree, valid matches are kept, and indexing
  does **not** fail (mirrors today's "skip unreadable file" tolerance).
- A regex family with a malformed regex → unchanged existing behaviour.

## 9. Testing

Following the `#[cfg(test)]` + `tempfile` pattern in `indexer.rs`:

- **tree-sitter extraction:** a fixture profile with a Kotlin `.scm` query +
  a fixture `.kt` file → the indexer emits the expected symbol (name + line),
  and does **not** match the same identifier inside a comment/string (the regex
  false-positive case, proving the structural win).
- **Schema validation:** a family with both `regex` and `query` is rejected; a
  family with `query` but no `language` is rejected; an unknown `language` is
  rejected; a `query` with no `@name` capture is rejected.
- **Regression:** an existing regex-only family (e.g. `route`) still extracts
  unchanged.
- **Mixed file set:** a profile mixing a regex family and a tree-sitter family
  produces both kinds of symbols from one index run.

## 10. Affected files

| File | Change |
|---|---|
| `Cargo.toml` / `Cargo.lock` | add `tree-sitter` + `tree-sitter-kotlin-ng` (pinned) |
| `src/indexer.rs` | `Extractor` enum, `language_for`, parse-once dispatch, extended `compile_families` validation, `.scm` file loading |
| `knowledge/profiles/android-mvvm/extractors.yaml` | `viewmodel`/`service`/`repository` → `query`+`language`; `route`/`i18n` unchanged |
| `knowledge/profiles/android-mvvm/extractors/*.scm` | new query files |
| `README.md` | note tree-sitter extraction in the indexer/roadmap |

## 11. Risks

- **Grammar ABI mismatch:** `tree-sitter-kotlin-ng` must be compatible with the
  pinned `tree-sitter` core. Mitigation: pin core to the grammar's required
  version; verified by a green build.
- **Grammar node names differ from assumptions:** mitigated by verifying against
  `node-types.json` before writing `.scm` (queries are data, no Rust change).
- **Binary-size / build-time growth:** accepted; quantified in the PR.
