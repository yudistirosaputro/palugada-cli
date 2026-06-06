---
id: architecture
title: Architecture
description: MVVM with a repository layer, Hilt DI, Coroutines + Flow, and a sealed UiState. UI-toolkit agnostic.
layer: all
sections:
  - { id: overview,   title: "MVVM Overview",             tokens: 170, code: false }
  - { id: layers,     title: "Layers & Responsibilities", tokens: 210, code: false }
  - { id: uistate,    title: "Sealed UiState",            tokens: 190, code: true  }
  - { id: data-flow,  title: "Unidirectional Data Flow",  tokens: 230, code: true  }
related:
  - { topic: di,          why: "Hilt module wiring for ViewModels and repositories" }
  - { topic: networking,  why: "Retrofit services and repository data sources" }
  - { topic: concurrency, why: "viewModelScope, Flow, and lifecycle-aware collection" }
  - { topic: testing,     why: "Unit-testing ViewModels and repositories" }
tags: [mvvm, architecture, hilt, flow, stateflow, repository]
---

# Architecture

> MVVM with a repository layer, Hilt DI, Coroutines + Flow, and a sealed UiState.
> Works the same whether the UI layer is Jetpack Compose or XML/ViewBinding.

## MVVM Overview {#overview}

MVVM separates three concerns: the **View** renders state and forwards user
events; the **ViewModel** holds and transforms UI state; the **Repository** owns
data access. The View contains no business logic, and the ViewModel never
references Android UI types (`Context`, `View`, `Fragment`). State flows in one
direction — down from ViewModel to View — while events flow up. This keeps
screens predictable, testable, and safe across configuration changes.

## Layers & Responsibilities {#layers}

- **View** — an Activity, Fragment, or Composable. Observes an immutable UiState
  and renders it; forwards user actions to the ViewModel as function calls. No
  data fetching, no business rules.
- **ViewModel** — exposes `StateFlow<UiState>` (and one-off effects via
  `SharedFlow`). Runs work in `viewModelScope`, survives configuration changes,
  and depends on repositories — never on UI types.
- **Repository** — the single source of truth for a data domain. Mediates remote
  (Retrofit) and local (Room/DataStore) sources, returns `Flow` or `suspend`
  results, and exposes domain models rather than raw DTOs.
- **Data sources** — Retrofit services, Room DAOs, DataStore — wired via Hilt.

Dependencies point inward: View → ViewModel → Repository → data source. Nothing
points back out.

## Sealed UiState {#uistate}

Model every state of a screen explicitly with one sealed type, so the View
handles each case in a `when` with no scattered boolean flags
(`isLoading`, `hasError`, …). Prefer `data object` for stateless cases.

```kotlin
sealed interface UiState<out T> {
    data object Loading : UiState<Nothing>
    data class  Success<T>(val data: T) : UiState<T>
    data class  Error(val message: String) : UiState<Nothing>
    data object Empty : UiState<Nothing>
}
```

## Unidirectional Data Flow {#data-flow}

The ViewModel exposes only an immutable `StateFlow`; the backing
`MutableStateFlow` stays private. State is produced from repository `Flow`s.

```kotlin
@HiltViewModel
class ItemListViewModel @Inject constructor(
    private val repository: ItemRepository
) : ViewModel() {

    private val _uiState = MutableStateFlow<UiState<List<Item>>>(UiState.Loading)
    val uiState: StateFlow<UiState<List<Item>>> = _uiState.asStateFlow()

    fun load() = viewModelScope.launch {
        repository.getItems()
            .map { items -> if (items.isEmpty()) UiState.Empty else UiState.Success(items) }
            .catch { e -> _uiState.value = UiState.Error(e.message ?: "Unknown error") }
            .collect { _uiState.value = it }
    }
}
```

Collect state in a lifecycle-aware way: `repeatOnLifecycle(STARTED)` for Views, or
`collectAsStateWithLifecycle()` for Compose. Events go up via ViewModel function
calls; state comes down via the `StateFlow`.
