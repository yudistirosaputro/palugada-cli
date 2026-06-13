# Design — Fill android-mvvm knowledge content

> **Status:** Approved for planning · **Date:** 2026-06-14
> **Scope:** Author the conventions and recipe the `brief` flows reference but
> that don't exist yet, so `bugfix`/`refactor`/`review` produce real content
> instead of `(no convention …)` / `(no recipe …)` notes.

## 1. Problem

The `brief` engine and all four flows are wired, but the `android-mvvm` profile
only ships two knowledge docs: `conventions/architecture.md` and
`recipes/feature.md`. The flow step lists reference more:

- `bugfix`: `convention(errorhandling)`, `convention(testing)` — both missing.
- `refactor`: `convention(style)`, `recipe(refactor)` — both missing.
- `review`: `convention(by-file-kind)` resolves via `review_map`, which today
  maps every family to `architecture` only.

So `brief bugfix`/`refactor` render degraded notes for the missing steps, and
`brief review` always surfaces the same single convention. This is pure content
work — no engine change.

## 2. Goals

- Author three conventions — `errorhandling`, `testing`, `style` — and one
  recipe — `refactor` — matching the existing file format exactly.
- Register them in `conventions/_index.json` and `recipes/_index.json` so they
  appear in `q --list`, `for --list`, and `s` search.
- Enrich `review_map` so `brief review` surfaces conventions relevant to each
  changed fact-family.
- Content philosophy: current Android best practice, UI-toolkit agnostic, no
  legacy/migration content (matches the profile's stated description).

## 3. Non-goals

- Conventions not referenced by any flow (`di`, `networking`, `concurrency` are
  linked as `related` from `architecture.md` but stay unwritten — out of scope).
- Engine/CLI/Rust changes of any kind. `cargo test` count is unchanged (43).
- A `bugfix` recipe — the `bugfix` flow uses conventions, not a recipe.

## 4. File format (must match existing docs)

**Convention `.md`** (mirror `conventions/architecture.md`): YAML front-matter
with `id`, `title`, `description`, `layer`, `sections: [{id,title,tokens,code}]`,
`related: [{topic,why}]`, `tags`. Body: a `# Title`, an intro blockquote, then
one `## Section Title {#section-id}` per declared section. Anchors must match the
front-matter `sections[].id`. `brief`/`q` read the `.md` directly; `sections()`
splits on `##` and ignores code-fence contents.

**Recipe `.md`** (mirror `recipes/feature.md`): front-matter with `id`, `title`,
`description`, `references: { conventions: [{topic,section,why}] }`,
`related_recipes`, `tags`. Body: `# Recipe: …`, then `## When to use this`,
`## What you'll produce`, `## Steps` (numbered). `for`/`brief` strip front-matter
and print the body.

**Indexes** are JSON; a malformed edit breaks `q --list`/`for --list`/`s`, so
they are validated by running those commands.

## 5. Content outline

### 5.1 `conventions/errorhandling.md`
| Section id | Title | code | Gist |
|---|---|---|---|
| `result` | Modeling failures | no | Model failures explicitly (sealed result / `kotlin.Result`); never swallow silently |
| `coroutines` | Errors in coroutines | yes | try/catch around suspend, `Flow.catch`, **rethrow `CancellationException`**, `CoroutineExceptionHandler` |
| `surfacing` | Surfacing to the UI | no | Map errors to `UiState.Error`; separate user-facing message from logged detail |
| `network` | Network errors | yes | Map Retrofit/HTTP failures to domain errors; timeout/retry guidance |

`related`: architecture (uistate), testing (error-path tests), networking.
`tags`: [error, exception, result, coroutines, flow, retrofit].

### 5.2 `conventions/testing.md`
| Section id | Title | code | Gist |
|---|---|---|---|
| `scope` | What to test | no | Test pyramid; unit-test ViewModels/repositories/mappers; skip framework-only code |
| `viewmodel` | Testing ViewModels | yes | `runTest`, `StandardTestDispatcher`, a Main dispatcher rule, **Turbine** for `StateFlow` |
| `repository` | Testing repositories | yes | Prefer fakes over mocks for data sources |
| `practices` | Practices | no | given-when-then naming; avoid Robolectric for pure logic |

`related`: architecture (data-flow), concurrency, errorhandling.
`tags`: [testing, junit, coroutines-test, turbine, flow, fake].

### 5.3 `conventions/style.md`
| Section id | Title | code | Gist |
|---|---|---|---|
| `formatting` | Formatting | no | Official Kotlin style + ktlint/detekt; trailing commas; import order |
| `naming` | Naming | no | `XxxViewModel`/`XxxRepositoryImpl`/`XxxService` suffixes; no Hungarian |
| `idioms` | Idioms | yes | Prefer `val`, data classes, expression bodies; avoid `!!` |
| `structure` | File structure | no | Package-by-feature; small functions; one focused top-level type per file |

`related`: architecture (layers).
`tags`: [style, kotlin, ktlint, naming, idioms, formatting].

### 5.4 `recipes/refactor.md`
Behaviour-preserving change. Body:
- **When to use:** changing structure/readability without changing behaviour.
- **What you'll produce:** the same behaviour, cleaner code, tests still green.
- **Steps:** (1) characterize current behaviour with tests first; (2) make small,
  reversible steps; (3) use IDE rename/extract over manual edits; (4) keep tests
  green after each step; (5) verify against `testing#viewmodel`.

`references.conventions`: architecture#layers, style#idioms, testing#viewmodel.
`related_recipes`: [feature]. `tags`: [refactor, cleanup, tests].

## 6. Index + review_map changes

Add three topic objects to `conventions/_index.json` (id, title, file,
description, tags, `sections` mirroring each doc's front-matter, `related`), and
one recipe object to `recipes/_index.json` (id, title, description, file,
`convention_refs`, `related_recipes`, tags).

`profile.yaml` `review_map` becomes:
```yaml
review_map:
  viewmodel:  [architecture, testing]
  repository: [architecture, errorhandling, testing]
  service:    [architecture, errorhandling]
  route:      [architecture]
  i18n:       [style]
```

## 7. Verification (CLI smoke — this is data, not code)

- `palugada q --list` lists `architecture`, `errorhandling`, `style`, `testing`.
- `palugada q errorhandling`, `q testing`, `q style` print full bodies;
  `q testing.2` prints one section (proves anchors parse).
- `palugada for --list` lists `feature`, `refactor`; `for refactor` prints the body.
- `palugada brief bugfix src/foo --profile android-mvvm` shows real
  `errorhandling` + `testing` sections (no `(no convention …)`).
- `palugada brief refactor src/foo --profile android-mvvm` shows `style` +
  `recipe: refactor` content.
- `palugada s turbine` and `s coroutine` return hits (proves `_index.json` tags).
- Both `_index.json` files are valid JSON (the `--list`/`s` commands above fail
  loudly otherwise).
- `cargo test` still reports 43 passed (no code touched).

## 8. Affected files

| File | Change |
|---|---|
| `knowledge/profiles/android-mvvm/conventions/errorhandling.md` | new |
| `knowledge/profiles/android-mvvm/conventions/testing.md` | new |
| `knowledge/profiles/android-mvvm/conventions/style.md` | new |
| `knowledge/profiles/android-mvvm/recipes/refactor.md` | new |
| `knowledge/profiles/android-mvvm/conventions/_index.json` | add 3 topics |
| `knowledge/profiles/android-mvvm/recipes/_index.json` | add 1 recipe |
| `knowledge/profiles/android-mvvm/profile.yaml` | enrich `review_map` |

No `README.md` change: the CLI surface and feature list are unchanged — only the
bundled profile's content grows.
