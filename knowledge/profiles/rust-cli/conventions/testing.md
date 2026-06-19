---
id: testing
title: Testing
description: Inline #[cfg(test)] modules, tempfile for filesystem isolation, pure transforms tested directly, real fixtures over mocks, assertions on outcomes.
sections:
  - { id: inline-tests, title: "Inline Tests",     tokens: 120, code: true  }
  - { id: tempfile,     title: "Tempfile for I/O",  tokens: 130, code: true  }
  - { id: pure-first,   title: "Test Pure First",   tokens: 120, code: false }
  - { id: no-mocks,     title: "Fakes Over Mocks",  tokens: 110, code: false }
tags: [rs, rust, testing, cargo-test, tempfile, cfg-test]
---

# Testing

> Tests live next to the code they cover. Test pure logic directly; reserve
> tempdirs and end-to-end runs for the I/O shell.

## Inline Tests {#inline-tests}

Each module carries its own tests in an inline module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_overlay_over_profile() {
        assert_eq!(merge(&base(), &over()), expected());
    }
}
```

Co-locating tests keeps them in sync with the code and visible during edits.

## Tempfile for I/O {#tempfile}

Isolate filesystem work with `tempfile::tempdir()` — never touch the real home or
repo:

```rust
let tmp = tempfile::tempdir().unwrap();
write(&tmp.path().join("config.yaml"), CONTENT);
assert!(load_from(tmp.path().to_str().unwrap()).is_ok());
```

The dir is removed when it drops, so tests stay hermetic and parallel-safe.

## Test Pure First {#pure-first}

Push logic into pure functions (see [[architecture]] core-shell) and test those
with literal inputs — fast, no setup, exhaustive on edge cases. The thin I/O
wrapper then needs only a tempdir smoke test, not full-coverage.

## Fakes Over Mocks {#no-mocks}

Prefer real fixtures and small hand-written fakes over a mocking framework. Build
the actual data structure, run the function, assert on the outcome — not on which
methods were called. Tests then survive refactors that preserve behavior.
