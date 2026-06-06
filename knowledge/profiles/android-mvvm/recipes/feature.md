---
id: feature
title: Scaffold a new feature
description: End-to-end recipe for a new screen/feature — data source, repository, sealed UiState, ViewModel, and UI — with Hilt wiring.
references:
  conventions:
    - { topic: architecture, section: layers,    why: "Responsibilities of each layer" }
    - { topic: architecture, section: uistate,   why: "Model screen state as a sealed UiState" }
    - { topic: architecture, section: data-flow, why: "Wire StateFlow from repository to UI" }
related_recipes: [viewmodel, repository]
tags: [feature, scaffold, mvvm]
---

# Recipe: Scaffold a new feature

## When to use this

You are adding a new screen or feature that fetches data and renders it. If you
are only adding a field to an existing screen, edit that screen's ViewModel
instead of scaffolding a new vertical slice.

## What you'll produce

A vertical slice, top to bottom: data source (Retrofit service and/or Room DAO)
→ Repository → sealed `UiState` → `@HiltViewModel` ViewModel → UI (Fragment or
Composable) → Hilt module → registered route.

## Steps

1. **Data source** — define a Retrofit `interface` with `suspend` functions
   returning DTOs, and/or a Room `@Dao`.
2. **Domain + UiState** — map DTOs to domain models inside the repository;
   define a `sealed interface XxxUiState` (see architecture#uistate).
3. **Repository** — make it the single source of truth; expose `Flow`/`suspend`;
   bind interface → impl with a Hilt `@Binds`.
4. **ViewModel** — annotate `@HiltViewModel`, inject the repository, and expose
   `StateFlow<UiState>` updated inside `viewModelScope` (architecture#data-flow).
5. **UI** — observe the `StateFlow` lifecycle-aware (`repeatOnLifecycle` for
   Views, `collectAsStateWithLifecycle()` for Compose); render every UiState
   case; send user actions back as ViewModel function calls.
6. **DI** — a Hilt `@Module` that provides the service and binds the repository.
7. **Navigation** — register the screen's route/destination.

## Checklist

- [ ] No business logic in the View; no Android UI types in the ViewModel.
- [ ] State exposed as an immutable `StateFlow`; the mutable backing stays private.
- [ ] Every state is a `UiState` case; errors surface as `UiState.Error`, never swallowed.
- [ ] ViewModel and repository are covered by unit tests.
