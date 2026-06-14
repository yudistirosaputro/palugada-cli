# Design — Generic symbol index (functions + symbols, every file)

> **Status:** Approved for planning · **Date:** 2026-06-14 · Sub-project A of the
> "indexing + skill-model" rework. (B = skill-integration model is a later cycle.)

## 1. Problem

`palugada index` only captures **class-level fact families** (viewmodel / service /
repository / route / i18n). There is no function- or symbol-level extraction, so
`palugada symbol` finds **0 of 70** functions in a real benchmark — grep beats it
outright for the most common need ("find functions across files"). The fact-family
model also can't generalize: it needs a hand-authored family per kind.

## 2. Goal

A **generic symbol index** of *all* definitions in every file — classes, objects,
functions, methods, properties — built from a per-language tree-sitter **tags
query**, so `palugada symbol` (and `module.info` / `brief`) can locate any
definition with structure grep can't give: kind, enclosing scope, signature, and
no comment/string false positives. Curated fact families stay for the
knowledge/brief layer.

## 3. Honest value vs grep

For "list *all* functions" the token cost is ~grep. The win is **targeted,
structured** retrieval: filter by kind (`--kind function`), see enclosing scope +
signature, and list a module's functions (`module.info`) without scanning — with
zero false positives from comments/strings. We do not claim to beat grep on raw
line count; we beat it on precision and targeted queries.

## 4. Data model + storage

A new generic index, separate from curated fact families:

- `index/symbols.json` becomes the **generic** symbol list:
  `{ name, kind, file, line, scope, signature }`.
  - `kind` ∈ `class` | `object` | `function` | `method` | `property`.
  - `scope` = enclosing class/object name, or `""` for top-level.
  - `signature` = the declaration header (up to the body), whitespace-collapsed,
    capped (~160 chars).
- Curated **fact families** keep their per-family files (`viewmodel.json` …).
  `fact <family>` now reads `<family>.json` directly (decoupled from
  `symbols.json`); fact families remain in the index manifest counts.
- Readers: `symbol`, `module_report`, and `brief`'s `symbol.find` read
  `symbols.json` (generic → now find functions). `fact_report` reads
  `<family>.json`.

## 5. Tags queries (language-level, embedded)

A per-language tree-sitter tags query, embedded in the binary (these are
**language** assets, independent of any profile):

- `src/tags/kotlin.scm` via `include_str!`, plus registries beside `language_for`:
  - `tags_query(lang) -> Option<&'static str>` (`"kotlin"` → the kotlin scm).
  - `language_for_ext(ext) -> Option<&'static str>` (`kt`/`kts` → `kotlin`).
- Adding a language later = grammar crate + a `language_for` arm + a `language_for_ext`
  arm + a `tags_query` arm + one `.scm`.

**Verified Kotlin node names** (from a grammar spike — do not re-derive):
`class_declaration name: (identifier)` (also covers interface + enum class),
`object_declaration name: (identifier)`, `function_declaration name: (identifier)`
(methods share this node), `property_declaration (variable_declaration (identifier))`.

`kotlin.scm` captures one named capture per kind so the kind is known per match:
```scheme
(class_declaration  name: (identifier) @class)
(object_declaration name: (identifier) @object)
(function_declaration name: (identifier) @function)
(property_declaration (variable_declaration (identifier) @property))
```
Kind comes from the firing capture name. Limitations (v1, documented): interface
and enum class report as `class` (the grammar doesn't distinguish them at the node
level).

## 6. Extraction

A generic pass in `indexer::run`, language-driven (not profile-driven): for each
file whose extension maps to a known language **with** a tags query, parse once and
run the tags query. Per match:
- `name` = the captured node's text; `kind` from the capture name; `line` =
  start row + 1.
- **scope + method-ness:** walk the name node's ancestors; if a
  `class_declaration`/`object_declaration` encloses it, `scope` = that type's
  `name:` text. A `function` with a non-empty scope becomes `kind = "method"`.
- **signature:** slice the source from the declaration node's start to its body
  child's start (`function_body` / `class_body` / `enum_class_body`), or the whole
  node if no body; collapse whitespace to single spaces; cap to ~160 chars.

The existing fact-family pass is unchanged. A file may be parsed twice (fact pass +
generic pass) in v1 — acceptable; unifying the parse is a noted future optimization.

## 7. CLI

- `palugada symbol <query> [--kind <k>] [--repo <path>]` — searches the generic
  index across all kinds; `--kind function` filters; `--repo` resolves the repo
  (fixes the current missing flag). Output line:
  `function login(user, pass): Boolean  ·  LoginViewModel  ·  Login.kt:6`.
- `brief symbol.find` and `module.info` improve automatically (generic source).
- `fact` behavior unchanged.

## 8. Non-goals

- Call graphs / references / usages (definitions only).
- Cross-file rename, signature beyond the header line, doc extraction.
- Distinguishing interface/enum from class at the node level (v1 reports `class`).
- Languages other than Kotlin (the registry makes adding them mechanical).

## 9. Testing

- `kotlin.scm` over a fixture (interface, object, class with method + property,
  top-level fun, enum) yields the expected `{name, kind, scope}` set: top-level
  `fun` → `function` scope ""; method → `method` scope = class; `val` → `property`;
  class/object captured.
- A `fun` inside a `//` comment or a string is **not** matched.
- `symbol --kind function` filters to functions/methods.
- `fact <family>` still returns curated families from per-family files.
- Round-trip: `index` the real `android-mvvm` fixture → `symbols.json` contains
  functions with scope/signature; `palugada profile validate android-mvvm` stays OK.

## 10. Affected files

| File | Change |
|---|---|
| `src/tags/kotlin.scm` | new — Kotlin tags query |
| `src/indexer.rs` | `language_for_ext` + `tags_query` registries; generic tags pass + rich `Symbol` fields (`scope`, `signature`); `fact_report` reads `<family>.json`; `symbol`/`module_report` over generic index |
| `src/main.rs` | `symbol` gains `--kind` + `--repo` |
| `README.md` | document the symbol index |
