---
id: errorhandling
title: Error Handling
description: Result<T,String> with context via map_err, propagation with ?, no unwrap/panic on non-test paths, and best-effort steps that degrade to an inline note.
sections:
  - { id: result-type, title: "Result Type",       tokens: 120, code: true  }
  - { id: context,     title: "Adding Context",     tokens: 140, code: true  }
  - { id: no-panic,    title: "No Panics",          tokens: 130, code: true  }
  - { id: degrade,     title: "Degrade, Don't Abort", tokens: 130, code: true }
tags: [rs, rust, error, result, map-err, panic]
---

# Error Handling

> Fallible work returns `Result<T, String>`. The error string is shown to the
> user, so make it specific and actionable.

## Result Type {#result-type}

Handlers and helpers return `Result<T, String>`. Reserve richer error enums for
libraries with many callers; for a single CLI, a contextful `String` is enough
and keeps `?` ergonomic:

```rust
pub fn load(path: &Path) -> Result<Config, String> { /* ... */ }
```

## Adding Context {#context}

Convert foreign errors at the boundary and name what failed:

```rust
serde_yaml::from_str(&raw)
    .map_err(|e| format!("parse {}: {e}", path.display()))?;
```

Each `map_err` answers "what were we doing?" so the final message reads as a
trail, not a bare OS error.

## No Panics {#no-panic}

No `unwrap()`, `expect()`, `panic!`, or `unreachable!` on non-test paths. Replace
them with `?`, `ok_or_else(|| format!(...))`, or a match that returns an error.
Panics abort the process with a backtrace — never a user-facing failure mode.
(`unwrap` is fine inside `#[cfg(test)]`.)

## Degrade, Don't Abort {#degrade}

A best-effort step (an optional connector, a network lookup) should degrade to an
inline note rather than fail the whole command:

```rust
let extra = fetch(ctx).unwrap_or_else(|e| format!("(skipped: {e})"));
```

The command still produces useful output; the note tells the user what was
missing and why.
