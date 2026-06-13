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
