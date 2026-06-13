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
