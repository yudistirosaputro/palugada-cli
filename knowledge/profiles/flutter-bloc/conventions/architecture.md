---
id: architecture
title: Architecture
description: Feature-first Clean Architecture in a workspace monorepo — data/domain/presentation layers per feature, GetIt DI registered per feature, GoRouter, and barrel exports.
sections:
  - { id: overview, title: "Feature-First Monorepo", tokens: 150, code: false }
  - { id: layers,   title: "Clean Layers",            tokens: 180, code: false }
  - { id: di,       title: "GetIt DI",                tokens: 150, code: true  }
  - { id: routing,  title: "GoRouter",               tokens: 130, code: false }
  - { id: barrels,  title: "Barrel Exports",          tokens: 110, code: true  }
tags: [dart, flutter, architecture, clean-architecture, getit, gorouter, melos]
---

# Architecture

> Each feature is a standalone workspace package with its own Clean-Architecture
> layers. Shared concerns live in `libraries/`; the root app wires features
> together.

## Feature-First Monorepo {#overview}

The repo is a Dart workspace: `features/<name>` packages, plus
`libraries/{dependencies,shared}` for cross-cutting deps and utilities. The root
`lib/` only bootstraps — `main.dart` configures DI and runs `MyApp`,
`app.dart` builds `MaterialApp.router`. A feature owns everything it needs and
exposes a small public surface; features don't reach into each other's internals.

## Clean Layers {#layers}

Within a feature, three layers with a strict dependency direction
(presentation → domain → data, never the reverse):

- **domain/** — abstractions (`*Repository` interfaces) and entities. No Flutter,
  no packages — pure Dart.
- **data/** — `datasources/` (remote/local) and `repositories/` (`*RepositoryImpl`
  implementing the domain interface).
- **presentation/** — `cubit/` (state holders) and `ui/` (pages + widgets).

## GetIt DI {#di}

Dependencies are resolved through a GetIt service locator. Each feature exposes a
`register<Feature>()` that registers its bindings; the root aggregates them:

```dart
void registerDetail() {
  sl.registerFactory(() => SampleCubit(repository: sl()));
  sl.registerLazySingleton<SampleRepository>(() => SampleRepositoryImpl(sl()));
}
```

`registerFactory` for transient (Cubits), `registerLazySingleton` for shared
services/repositories.

## GoRouter {#routing}

Navigation uses GoRouter configured once (`lib/router/app_router.dart`). Route
names/paths are centralized as constants in `libraries/shared`
(`named_routes.dart`) — never hard-code path strings at call sites. Pages are
referenced by name so routing stays refactor-safe.

## Barrel Exports {#barrels}

Each feature has a barrel (`<feature>.dart`) that re-exports its public API and
its `register<Feature>()`:

```dart
export 'presentation/ui/detail_page.dart';
export 'detail_injection.dart' show registerDetail;
```

Consumers import the barrel, not deep paths.
