---
id: architecture
title: Architecture
description: Single-binary CLI — thin main dispatch over a clap-derive command enum, module-per-concern, and a pure-core / thin-I/O-shell split with Result<T,String>.
sections:
  - { id: overview,   title: "CLI Overview",            tokens: 150, code: false }
  - { id: modules,    title: "Module-per-Concern",      tokens: 150, code: false }
  - { id: core-shell, title: "Pure Core, I/O Shell",     tokens: 180, code: true  }
  - { id: results,    title: "Result Plumbing",          tokens: 150, code: true  }
tags: [rs, rust, cli, clap, architecture, module]
---

# Architecture

> A single statically-linked binary. `main` is a thin dispatcher; the real work
> lives in focused modules with a pure core and a thin I/O shell.

## CLI Overview {#overview}

The command surface is a `clap`-derive `Commands` enum. `main` parses args, then
`match`es each variant to a small `cmd_*` handler that returns
`Result<(), String>`. Keep `main` free of logic — it only routes. Each subcommand
maps to exactly one handler; the handler orchestrates modules and prints the
result. Global flags (e.g. `--project`, `--profile`) are resolved once and passed
down, never re-read ad hoc.

## Module-per-Concern {#modules}

One `mod` per concern (config, indexing, a connector family, a command group),
not per type. Files that change together live together. Keep the `pub` surface
minimal — expose the functions other modules call, keep helpers private. When a
file grows past a few hundred lines or mixes two responsibilities, split it. A
reader should understand a module without holding the whole crate in their head.

## Pure Core, I/O Shell {#core-shell}

Separate decisions from effects. Pure functions take and return data — no fs,
network, or process calls — and are unit-tested directly with literals. Thin
wrappers do the I/O and call the pure core:

```rust
// pure: tested with literals, no I/O
fn merge(profile: &Map, overlay: &Map) -> Map { /* ... */ }

// shell: thin I/O, delegates the decision to `merge`
pub fn apply(repo: &str) -> Result<(), String> {
    let (p, o) = load_both(repo)?;     // I/O in
    save(repo, &merge(&p, &o))         // I/O out
}
```

This keeps the logic testable without tempdirs and the I/O layer trivial.

## Result Plumbing {#results}

Everything fallible returns `Result<T, String>` and propagates with `?`. The
`String` is a user-facing message; build it with context at each boundary:

```rust
let data = fs::read_to_string(&path)
    .map_err(|e| format!("read {}: {e}", path.display()))?;
```

Data flows in as typed inputs and out as values or rendered packs — handlers
don't reach back into globals mid-computation.
