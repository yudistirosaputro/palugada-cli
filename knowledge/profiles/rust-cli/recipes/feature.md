---
id: feature
title: Add a subcommand
description: End-to-end recipe for a new CLI subcommand — clap variant, cmd_ handler, owning module, dispatch wiring, and inline tests.
references:
  conventions:
    - { topic: architecture, section: overview, why: "main dispatches to a cmd_ handler" }
    - { topic: errorhandling, section: result-type, why: "handler returns Result<(), String>" }
    - { topic: testing, section: pure-first, why: "test the pure parts directly" }
related_recipes: [refactor]
tags: [feature, subcommand, clap]
---

# Recipe: Add a subcommand

Add a new verb to the CLI, wired end-to-end.

1. **Declare the command.** Add a variant to the `clap`-derive `Commands` enum
   with its args (positional + flags), and a short doc-comment — clap turns it
   into `--help` text.

2. **Write the handler.** In the owning module (or a new `mod` if it's a new
   concern), add `fn cmd_<verb>(...) -> Result<(), String>`. Keep it thin:
   resolve inputs, call the pure core, print the result. Push real logic into a
   pure helper so it can be tested without I/O (see [[architecture]] core-shell).

3. **Wire dispatch.** Add a `match` arm in `main` mapping the new variant to
   `cmd_<verb>(...)`. `main` stays logic-free.

4. **Test.** Add inline `#[cfg(test)]` tests for the pure helper with literal
   inputs; if the handler touches the filesystem, add a `tempfile` smoke test.

5. **Verify.** `cargo test`, `cargo clippy`, and `cargo run -- <verb> --help` to
   confirm the command surface reads well. Commit.
