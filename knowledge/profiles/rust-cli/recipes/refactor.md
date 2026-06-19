---
id: refactor
title: Extract a pure helper
description: Lift pure logic out of an I/O function into a tested helper (or split an overgrown module), keeping tests green throughout.
references:
  conventions:
    - { topic: architecture, section: core-shell, why: "separate decisions from effects" }
    - { topic: testing, section: pure-first, why: "TDD the extracted helper" }
    - { topic: style, section: signatures, why: "borrow-friendly extracted signature" }
related_recipes: [feature]
tags: [refactor, extract, tdd, module-split]
---

# Recipe: Extract a pure helper

Untangle logic from I/O so it can be tested and reused.

1. **Spot the tangle.** Find a function that mixes a decision (parsing, merging,
   formatting) with effects (fs/network/process). The decision is what you'll
   lift.

2. **Name the helper.** Define `fn <name>(input: &T) -> U` taking plain data and
   returning a value — no I/O. Use borrowed inputs (`&str`, `&[T]`).

3. **Test it first.** Write `#[cfg(test)]` cases against the new signature with
   literal inputs, including edge cases. Run them red.

4. **Move the body.** Cut the pure logic into the helper; the original function
   now loads inputs, calls the helper, writes outputs.

5. **Green + clean.** `cargo test` stays green; `cargo clippy` clean. If a module
   has grown to cover two concerns, split it into focused modules in the same
   pass. Commit.
