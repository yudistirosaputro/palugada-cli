# Fill android-mvvm knowledge content Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Author the three conventions (`errorhandling`, `testing`, `style`) and one recipe (`refactor`) the `brief` flows reference, register them in the indexes, and enrich `review_map` — so all four flows render real content.

**Architecture:** Pure data. Each convention `.md` mirrors `conventions/architecture.md` (front-matter + `## Title {#id}` sections); the recipe mirrors `recipes/feature.md`. `brief`/`q`/`for` read the `.md` directly; `_index.json` feeds `--list`/`s`. No Rust changes.

**Tech Stack:** Markdown + YAML front-matter, JSON indexes, the `palugada` CLI for verification.

**Reference spec:** `docs/superpowers/specs/2026-06-14-android-mvvm-knowledge-content-design.md`

**Conventions for all docs:** current Android best practice, UI-toolkit agnostic, no legacy. Section anchors `{#id}` MUST equal the front-matter `sections[].id`. Build the binary once up front so smoke commands use it: `cargo build` then use `./target/debug/palugada`. The knowledge dir resolves from the repo when run inside it.

---

## Task 1: `errorhandling.md` convention

**Files:**
- Create: `knowledge/profiles/android-mvvm/conventions/errorhandling.md`

- [ ] **Step 1: Write the file**

```markdown
---
id: errorhandling
title: Error Handling
description: Model failures explicitly, handle coroutine cancellation correctly, and surface errors as UiState rather than crashing or swallowing them.
layer: all
sections:
  - { id: result,     title: "Modeling Failures",     tokens: 180, code: false }
  - { id: coroutines, title: "Errors in Coroutines",  tokens: 210, code: true  }
  - { id: surfacing,  title: "Surfacing to the UI",   tokens: 170, code: false }
  - { id: network,    title: "Network Errors",        tokens: 190, code: true  }
related:
  - { topic: architecture, why: "Errors become a UiState.Error case" }
  - { topic: testing,      why: "Cover the error paths, not just the happy path" }
tags: [error, exception, result, coroutines, flow, retrofit]
---

# Error Handling

> Failures are part of the contract. Model them as data, propagate cancellation
> faithfully, and turn errors into state the UI can render — never silent
> catch-all blocks.

## Modeling Failures {#result}

Represent an operation that can fail as an explicit value, not a thrown
exception that callers might forget to catch. Two idiomatic choices:

- A domain `sealed interface` result (e.g. `Ok(value)` / `Err(reason)`) when you
  want typed, enumerated failure reasons the caller must handle in a `when`.
- `kotlin.Result<T>` for thin wrappers where any `Throwable` is acceptable.

Repositories should return modeled results (or typed domain errors) rather than
leaking raw `IOException`/`HttpException` to the ViewModel. Never write an empty
`catch {}` — swallowing an error hides bugs and strands the UI in a stale state.

## Errors in Coroutines {#coroutines}

Wrap suspending work in try/catch at the boundary where you can recover, and use
`Flow.catch` for streams. The one rule you must not break: **let
`CancellationException` propagate** — catching it breaks structured concurrency
and leaks coroutines.

```kotlin
fun load() = viewModelScope.launch {
    repository.getItems()
        .catch { e ->
            if (e is CancellationException) throw e   // never swallow cancellation
            _uiState.value = UiState.Error(e.toUserMessage())
        }
        .collect { _uiState.value = UiState.Success(it) }
}
```

For fire-and-forget work that must report its own failures, attach a
`CoroutineExceptionHandler`; for awaited work, prefer try/catch around `await()`.

## Surfacing to the UI {#surfacing}

Map every caught error to a `UiState.Error` (see architecture#uistate) carrying a
**user-facing message** — short, actionable, localized. Keep the technical
detail (stack trace, status code) in the log, not on screen. Distinguish
recoverable errors (offer a retry action) from terminal ones. The View renders
the error case like any other state; it never runs try/catch itself.

## Network Errors {#network}

Translate transport and HTTP failures into domain errors inside the data layer,
so higher layers never branch on Retrofit types.

```kotlin
suspend fun fetch(): Result<List<Item>> = runCatching {
    api.getItems()                         // suspend Retrofit call
}.recoverCatching { e ->
    throw when (e) {
        is IOException        -> NetworkError.Offline
        is HttpException      -> NetworkError.Http(e.code())
        else                  -> e
    }
}
```

Set sensible call timeouts on the OkHttp client; retry only idempotent reads, and
bound the retries. Surface a distinct message for offline vs. server errors.
```

- [ ] **Step 2: Verify it renders**

Run: `cargo build && ./target/debug/palugada q errorhandling --profile android-mvvm | head -5 && ./target/debug/palugada q errorhandling.2 --profile android-mvvm | head -3`
Expected: the title/description outline prints, and section 2 ("Errors in Coroutines") prints — proving the `{#...}` anchors parse.

- [ ] **Step 3: Commit**

```bash
git add knowledge/profiles/android-mvvm/conventions/errorhandling.md
git commit -m "feat(profile): add errorhandling convention to android-mvvm

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `testing.md` convention

**Files:**
- Create: `knowledge/profiles/android-mvvm/conventions/testing.md`

- [ ] **Step 1: Write the file**

```markdown
---
id: testing
title: Testing
description: Unit-test ViewModels, repositories, and mappers with coroutines-test and Turbine; prefer fakes over mocks; keep framework code out of pure-logic tests.
layer: all
sections:
  - { id: scope,      title: "What to Test",         tokens: 170, code: false }
  - { id: viewmodel,  title: "Testing ViewModels",   tokens: 220, code: true  }
  - { id: repository, title: "Testing Repositories", tokens: 170, code: true  }
  - { id: practices,  title: "Practices",            tokens: 150, code: false }
related:
  - { topic: architecture,  why: "Unidirectional data flow is what you assert on" }
  - { topic: errorhandling, why: "Test the error paths, not only success" }
tags: [testing, junit, coroutines-test, turbine, flow, fake]
---

# Testing

> Test behaviour, not implementation. The bulk of value is in fast JVM unit tests
> over ViewModels, repositories, and mappers — the pieces that hold logic.

## What to Test {#scope}

Follow the test pyramid: many fast unit tests, fewer integration tests, a thin
layer of UI/instrumented tests. Unit-test the layers that carry logic —
ViewModels (state transitions), repositories (source mediation, error mapping),
and DTO→domain mappers. Don't unit-test framework glue (Hilt modules, generated
code) or trivial getters; cover those, if at all, with a single smoke test. Every
feature gets at least: a success-path test and an error-path test.

## Testing ViewModels {#viewmodel}

Use `kotlinx-coroutines-test` to control time and dispatchers, and **Turbine** to
assert on `StateFlow`/`Flow` emissions. Swap the Main dispatcher with a rule.

```kotlin
@get:Rule val mainRule = MainDispatcherRule()   // sets Dispatchers.Main to a TestDispatcher

@Test
fun `emits Success when repository returns items`() = runTest {
    val vm = ItemListViewModel(FakeItemRepository(items = listOf(item)))
    vm.uiState.test {                                  // Turbine
        assertEquals(UiState.Loading, awaitItem())
        vm.load()
        assertEquals(UiState.Success(listOf(item)), awaitItem())
        cancelAndIgnoreRemainingEvents()
    }
}
```

`runTest` auto-advances virtual time, so no real delays. Inject a
`TestDispatcher` rather than hardcoding `Dispatchers.IO` in the code under test.

## Testing Repositories {#repository}

Prefer hand-written **fakes** over mock frameworks for data sources — they read
clearly and survive refactors. Mock only at true external boundaries.

```kotlin
class FakeItemApi(private val result: Result<List<ItemDto>>) : ItemApi {
    override suspend fun getItems(): List<ItemDto> = result.getOrThrow()
}

@Test
fun `maps offline IOException to NetworkError_Offline`() = runTest {
    val repo = ItemRepositoryImpl(FakeItemApi(Result.failure(IOException())))
    assertEquals(NetworkError.Offline, repo.fetch().exceptionOrNull())
}
```

## Practices {#practices}

Name tests as behaviour: `` `emits Error when api fails` `` (backtick names).
Structure each as given–when–then. Keep pure-logic tests on the JVM — avoid
Robolectric unless you genuinely need Android framework classes; it is far
slower. One assertion concept per test; share setup through small fixtures, not
deep inheritance.
```

- [ ] **Step 2: Verify it renders**

Run: `./target/debug/palugada q testing --profile android-mvvm | head -5 && ./target/debug/palugada q testing.2 --profile android-mvvm | head -3`
Expected: outline prints; section 2 ("Testing ViewModels") prints.

- [ ] **Step 3: Commit**

```bash
git add knowledge/profiles/android-mvvm/conventions/testing.md
git commit -m "feat(profile): add testing convention to android-mvvm

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `style.md` convention

**Files:**
- Create: `knowledge/profiles/android-mvvm/conventions/style.md`

- [ ] **Step 1: Write the file**

```markdown
---
id: style
title: Style
description: Official Kotlin style enforced with ktlint/detekt, consistent naming with role suffixes, idiomatic null-safety, and package-by-feature structure.
layer: all
sections:
  - { id: formatting, title: "Formatting",     tokens: 150, code: false }
  - { id: naming,     title: "Naming",         tokens: 170, code: false }
  - { id: idioms,     title: "Idioms",         tokens: 200, code: true  }
  - { id: structure,  title: "File Structure", tokens: 150, code: false }
related:
  - { topic: architecture, why: "Naming reflects the layer a type belongs to" }
tags: [style, kotlin, ktlint, naming, idioms, formatting]
---

# Style

> Consistency over preference. Adopt the official Kotlin style, enforce it
> automatically, and let reviewers spend their attention on logic, not layout.

## Formatting {#formatting}

Use the official Kotlin code style (4-space indent, 100–120 col soft wrap) and
enforce it with **ktlint** (or detekt's formatting rules) in CI, so formatting is
never a review topic. Use trailing commas in multi-line argument and parameter
lists for clean diffs. Order imports per the default rules and forbid wildcard
imports. Run the formatter on commit (a pre-commit hook or Gradle task).

## Naming {#naming}

Types are `UpperCamelCase`, functions and properties `lowerCamelCase`, constants
`UPPER_SNAKE_CASE`. Name by **role suffix** so a type's layer is obvious:
`XxxViewModel`, `XxxRepository`/`XxxRepositoryImpl`, `XxxService` (Retrofit),
`XxxUiState`, `XxxDao`. No Hungarian notation and no `m`-prefixes. Booleans read
as predicates (`isLoading`, `hasError`). Test functions may use backtick
behaviour names.

## Idioms {#idioms}

Prefer immutability and expressions over statements.

```kotlin
// Prefer val; expression bodies; safe calls over !!
val name = user?.displayName ?: "Anonymous"          // not user!!.displayName
fun area(r: Rect) = r.width * r.height                // expression body
data class Money(val cents: Long, val currency: String)

// when as an expression, exhaustive over a sealed type
val label = when (state) {
    is UiState.Loading -> "…"
    is UiState.Success -> state.data.title
    is UiState.Error   -> state.message
    UiState.Empty      -> "Nothing here"
}
```

Avoid `!!` (it asserts a crash); handle null explicitly. Use scope functions
(`let`/`apply`/`also`) sparingly and only when they improve readability.

## File Structure {#structure}

Organize **package-by-feature**, not package-by-layer — keep a feature's
ViewModel, UiState, and repository together. Keep functions small and single
purpose; if a function needs a section comment, it probably wants to be two
functions. One focused top-level type per file as a default; small, tightly
related types (a sealed hierarchy) may share a file.
```

- [ ] **Step 2: Verify it renders**

Run: `./target/debug/palugada q style --profile android-mvvm | head -5 && ./target/debug/palugada q style.3 --profile android-mvvm | head -3`
Expected: outline prints; section 3 ("Idioms") prints.

- [ ] **Step 3: Commit**

```bash
git add knowledge/profiles/android-mvvm/conventions/style.md
git commit -m "feat(profile): add style convention to android-mvvm

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: `refactor.md` recipe

**Files:**
- Create: `knowledge/profiles/android-mvvm/recipes/refactor.md`

- [ ] **Step 1: Write the file**

```markdown
---
id: refactor
title: Refactor safely
description: Change structure or readability without changing behaviour — characterize with tests first, move in small reversible steps, keep tests green throughout.
references:
  conventions:
    - { topic: testing,      section: viewmodel, why: "Pin behaviour with tests before changing it" }
    - { topic: style,        section: idioms,    why: "What 'cleaner' looks like here" }
    - { topic: architecture, section: layers,    why: "Respect layer boundaries while moving code" }
related_recipes: [feature]
tags: [refactor, cleanup, tests]
---

# Recipe: Refactor safely

## When to use this

You are improving the structure, naming, or readability of code **without
changing its observable behaviour** — extracting a class, renaming, splitting a
god-ViewModel, replacing `!!` with safe handling. If you are changing behaviour,
that is a feature/bugfix, not a refactor; do that separately.

## What you'll produce

The same behaviour, expressed more clearly, with the test suite still green and
the diff reviewable as a series of small, intention-revealing steps.

## Steps

1. **Characterize first.** Before touching anything, make sure the behaviour is
   covered by tests (see testing#viewmodel). If it isn't, add characterization
   tests that pin the current behaviour — even if that behaviour is imperfect.
2. **Small, reversible steps.** Refactor in commits small enough to revert
   cleanly. Don't mix a rename with a logic change in one step.
3. **Use the tooling.** Prefer the IDE's Rename/Extract/Inline/Change-Signature
   refactorings over manual edits — they update all references safely.
4. **Stay green.** Run the relevant tests after each step; a red bar means stop
   and undo the last step, not push forward.
5. **Tidy to the conventions.** Align names and idioms with style#naming and
   style#idioms, and keep each change inside its architectural layer
   (architecture#layers). Commit when green.

## Done when

Behaviour is unchanged (tests prove it), the code reads more clearly, and no
step left the suite red.
```

- [ ] **Step 2: Verify it renders**

Run: `./target/debug/palugada for refactor --profile android-mvvm | head -6`
Expected: the recipe body prints starting at "# Recipe: Refactor safely".

- [ ] **Step 3: Commit**

```bash
git add knowledge/profiles/android-mvvm/recipes/refactor.md
git commit -m "feat(profile): add refactor recipe to android-mvvm

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Register in indexes + enrich review_map

**Files:**
- Modify: `knowledge/profiles/android-mvvm/conventions/_index.json`
- Modify: `knowledge/profiles/android-mvvm/recipes/_index.json`
- Modify: `knowledge/profiles/android-mvvm/profile.yaml`

- [ ] **Step 1: Add the three topics to `conventions/_index.json`**

Inside the `"topics"` array, after the existing `architecture` object, add:

```json
    ,
    {
      "id": "errorhandling",
      "title": "Error Handling",
      "file": "errorhandling.md",
      "description": "Model failures explicitly, handle coroutine cancellation correctly, and surface errors as UiState rather than crashing or swallowing them.",
      "tags": ["error", "exception", "result", "coroutines", "flow", "retrofit"],
      "sections": [
        { "id": "result",     "title": "Modeling Failures",    "tokens": 180 },
        { "id": "coroutines", "title": "Errors in Coroutines", "tokens": 210 },
        { "id": "surfacing",  "title": "Surfacing to the UI",  "tokens": 170 },
        { "id": "network",    "title": "Network Errors",       "tokens": 190 }
      ],
      "related": ["architecture", "testing", "networking"]
    },
    {
      "id": "testing",
      "title": "Testing",
      "file": "testing.md",
      "description": "Unit-test ViewModels, repositories, and mappers with coroutines-test and Turbine; prefer fakes over mocks; keep framework code out of pure-logic tests.",
      "tags": ["testing", "junit", "coroutines-test", "turbine", "flow", "fake"],
      "sections": [
        { "id": "scope",      "title": "What to Test",         "tokens": 170 },
        { "id": "viewmodel",  "title": "Testing ViewModels",   "tokens": 220 },
        { "id": "repository", "title": "Testing Repositories", "tokens": 170 },
        { "id": "practices",  "title": "Practices",            "tokens": 150 }
      ],
      "related": ["architecture", "errorhandling", "concurrency"]
    },
    {
      "id": "style",
      "title": "Style",
      "file": "style.md",
      "description": "Official Kotlin style enforced with ktlint/detekt, consistent naming with role suffixes, idiomatic null-safety, and package-by-feature structure.",
      "tags": ["style", "kotlin", "ktlint", "naming", "idioms", "formatting"],
      "sections": [
        { "id": "formatting", "title": "Formatting",     "tokens": 150 },
        { "id": "naming",     "title": "Naming",         "tokens": 170 },
        { "id": "idioms",     "title": "Idioms",         "tokens": 200 },
        { "id": "structure",  "title": "File Structure", "tokens": 150 }
      ],
      "related": ["architecture"]
    }
```

(The leading `,` closes the existing `architecture` object — make sure the final array still has exactly one comma between each object and no trailing comma.)

- [ ] **Step 2: Add the recipe to `recipes/_index.json`**

Inside the `"recipes"` array, after the existing `feature` object, add:

```json
    ,
    {
      "id": "refactor",
      "title": "Refactor safely",
      "description": "Change structure or readability without changing behaviour — characterize with tests first, move in small reversible steps, keep tests green throughout.",
      "file": "refactor.md",
      "convention_refs": [
        { "topic": "testing",      "section": "viewmodel" },
        { "topic": "style",        "section": "idioms" },
        { "topic": "architecture", "section": "layers" }
      ],
      "related_recipes": ["feature"],
      "tags": ["refactor", "cleanup", "tests"]
    }
```

- [ ] **Step 3: Enrich `review_map` in `profile.yaml`**

Replace the existing `review_map:` block with:

```yaml
review_map:
  viewmodel:  [architecture, testing]
  repository: [architecture, errorhandling, testing]
  service:    [architecture, errorhandling]
  route:      [architecture]
  i18n:       [style]
```

- [ ] **Step 4: Verify the indexes are valid JSON and lists/search work**

Run:
```bash
python3 -m json.tool knowledge/profiles/android-mvvm/conventions/_index.json > /dev/null && echo "conv index OK"
python3 -m json.tool knowledge/profiles/android-mvvm/recipes/_index.json > /dev/null && echo "recipe index OK"
./target/debug/palugada q --list --profile android-mvvm
./target/debug/palugada for --list --profile android-mvvm
./target/debug/palugada s turbine --profile android-mvvm
```
Expected: both "index OK"; `q --list` shows architecture/errorhandling/style/testing; `for --list` shows feature/refactor; `s turbine` returns a hit.

- [ ] **Step 5: Commit**

```bash
git add knowledge/profiles/android-mvvm/conventions/_index.json knowledge/profiles/android-mvvm/recipes/_index.json knowledge/profiles/android-mvvm/profile.yaml
git commit -m "feat(profile): register new conventions/recipe + enrich review_map

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: Final verification — flows render real content

**Files:** none (verification only)

- [ ] **Step 1: Verify bugfix + refactor flows are no longer degraded**

Run:
```bash
./target/debug/palugada brief bugfix src/main.rs --profile android-mvvm | grep -E "convention:|no convention"
./target/debug/palugada brief refactor src/main.rs --profile android-mvvm | grep -E "convention:|recipe:|no convention|no recipe"
```
Expected: `convention: errorhandling`, `convention: testing` appear for bugfix; `convention: style` and `recipe: refactor` appear for refactor; NO `(no convention …)` / `(no recipe …)` lines.

- [ ] **Step 2: Verify review surfaces mapped conventions**

Run: `./target/debug/palugada brief review HEAD~1 --profile android-mvvm | sed -n '/by file kind/,$p' | head`
Expected: prints `## conventions by file kind` — for a Rust repo the changed files are unclassified so it may say "(no fact-family files changed)"; that is correct (the `review_map` is exercised by the indexer test path, and by Kotlin repos in practice).

- [ ] **Step 3: Confirm no code regressed**

Run: `cargo test`
Expected: `43 passed` (unchanged — this task touched only data).

- [ ] **Step 4: Done — no commit (verification only)**

---

## Self-review notes

- **Spec coverage:** §5.1→Task 1, §5.2→Task 2, §5.3→Task 3, §5.4→Task 4, §6→Task 5, §7→Tasks 1–6 verification. §3 non-goals (no di/networking/concurrency, no Rust, no bugfix recipe) respected.
- **Anchor consistency:** every `## … {#id}` in Tasks 1–3 matches that doc's front-matter `sections[].id` and the `_index.json` `sections[].id` in Task 5 (result/coroutines/surfacing/network; scope/viewmodel/repository/practices; formatting/naming/idioms/structure).
- **JSON safety:** Task 5 Steps 1–2 insert with a leading comma after the existing object; Step 4 validates with `python3 -m json.tool` before committing.
- **review_map names** reference only existing conventions after Tasks 1–3 (architecture/errorhandling/testing/style).
