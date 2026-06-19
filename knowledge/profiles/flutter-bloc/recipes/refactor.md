---
id: refactor
title: Extract a widget / split a Cubit
description: Pull a sub-tree out of a Page into its own View widget, or split an overgrown Cubit, keeping bloc_tests green and DI registration in sync.
references:
  conventions:
    - { topic: statemanagement, section: consume, why: "widgets stay logic-free" }
    - { topic: architecture, section: di, why: "update GetIt registration" }
    - { topic: testing, section: bloc-test, why: "keep state-sequence tests green" }
related_recipes: [feature]
tags: [refactor, widget, cubit]
---

# Recipe: Extract a widget / split a Cubit

Two common refactors that keep features readable.

**Extract a View widget**

1. Find a chunk of a `*Page`'s `build` that is self-contained.
2. Move it into a `*View` widget (const constructor; takes the data it needs as
   params or reads the Cubit via context).
3. Replace the chunk in the page with the new widget.
4. `flutter analyze`; widget tests still pass.

**Split an overgrown Cubit**

1. If a Cubit's `State` has grown too many unrelated variants, identify a cohesive
   subset.
2. Extract a focused Cubit + State for that subset; move the relevant methods.
3. Update `register<Feature>()` so both Cubits are provided, and wrap the page
   subtree in the new `BlocProvider`.
4. Split/extend the `bloc_test`s to cover each Cubit; keep them green. Commit.
