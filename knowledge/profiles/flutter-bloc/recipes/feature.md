---
id: feature
title: Scaffold a feature package
description: End-to-end recipe for a new feature — workspace package, data/domain/presentation layers, Cubit + State, Page, GetIt registration, and a route.
references:
  conventions:
    - { topic: architecture, section: layers, why: "where each file goes" }
    - { topic: architecture, section: di, why: "register the feature in GetIt" }
    - { topic: statemanagement, section: state-classes, why: "model the State" }
related_recipes: [refactor]
tags: [feature, scaffold, clean-architecture]
---

# Recipe: Scaffold a feature package

Add a self-contained feature to the workspace.

1. **Create the package.** `features/<name>/` with its own `pubspec.yaml` added
   as a workspace member; depend on `dependencies` and `shared` libraries.

2. **Domain layer.** `domain/repositories/<name>_repository.dart` — an abstract
   `*Repository` (+ entities under `domain/`). Pure Dart, no Flutter.

3. **Data layer.** `data/datasources/<name>_data_source.dart` (remote/local) and
   `data/repositories/<name>_repository_impl.dart` implementing the domain
   interface, translating transport errors to domain failures.

4. **Presentation layer.** `presentation/cubit/<name>_cubit.dart` +
   `<name>_state.dart` (sealed-style states), and `presentation/ui/<name>_page.dart`
   using `BlocProvider`/`BlocBuilder`.

5. **Register DI.** Add `register<Name>()` wiring the Cubit (factory) and
   repository/datasource (lazy singletons) into GetIt; call it from the root.

6. **Route it.** Add a route constant in `libraries/shared` `named_routes.dart`
   and a `GoRoute` in the router pointing at the page.

7. **Export + verify.** Export the public API from the feature barrel; run
   `flutter analyze` and the feature's `bloc_test`s.
