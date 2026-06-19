---
id: style
title: Style
description: cargo fmt + clippy clean, idiomatic naming, doc-comments on public items, borrow-friendly signatures, and a minimal pub surface.
sections:
  - { id: fmt-clippy, title: "fmt + clippy",     tokens: 110, code: false }
  - { id: naming,     title: "Naming",           tokens: 110, code: false }
  - { id: docs,       title: "Doc Comments",      tokens: 100, code: true  }
  - { id: signatures, title: "Signatures",        tokens: 120, code: true  }
tags: [rs, rust, style, rustfmt, clippy, naming]
---

# Style

> Idiomatic Rust the compiler and clippy are happy with. Consistency beats
> cleverness.

## fmt + clippy {#fmt-clippy}

`cargo fmt` is the formatter of record — never hand-format. `cargo clippy` is the
linter of record — keep it clean and treat its lints as signal, not noise. Run
both before every commit; a green clippy is part of "done".

## Naming {#naming}

`snake_case` for functions, variables, and modules; `CamelCase` for types, traits,
and enum variants; `SCREAMING_SNAKE_CASE` for consts and statics. Command
handlers share a verb prefix (`cmd_*`) so the command surface is greppable. Names
say what a thing is, not how it's implemented.

## Doc Comments {#docs}

Public items get a `///` doc-comment; modules get a `//!` header explaining their
role:

```rust
//! Per-project config: load/merge/save `.palugada/config.yaml`.

/// Load the project config, or a clear error pointing at `init`.
pub fn load_from(repo: &str) -> Result<Config, String> { /* ... */ }
```

## Signatures {#signatures}

Borrow in signatures — `&str` over `String`, `&[T]` over `Vec<T>`, `&Path` over
`PathBuf` — so callers aren't forced to allocate:

```rust
fn families_for_path(rel: &str, ext: &str, fams: &[Family]) -> Vec<String>
```

Keep the `pub` surface minimal: expose what other modules need, keep the rest
private.
