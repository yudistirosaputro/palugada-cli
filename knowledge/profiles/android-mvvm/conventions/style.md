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
