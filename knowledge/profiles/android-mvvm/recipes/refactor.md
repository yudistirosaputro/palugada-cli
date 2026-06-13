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
