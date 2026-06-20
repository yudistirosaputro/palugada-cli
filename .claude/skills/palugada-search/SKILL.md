---
name: palugada-search
description: >
  TRIGGER when locating code — find a function/class/symbol, "where is X defined",
  what calls/lives in a module — and BEFORE any grep/find/rg/Glob over the repo.
allowed-tools: Bash(palugada *), Grep, Glob, Read
---

# Locate code via palugada FIRST

The project is indexed. Use the index before grepping.

    palugada symbol <name>                   # any definition: class/function/method/property
    palugada symbol <name> --kind function   # narrow by kind
    palugada fact <family> [name]            # curated facts (e.g. viewmodel, route)

**Hard rule:** run `palugada symbol` / `palugada fact` BEFORE any `grep`,
`find`, `rg`, or `Glob` for code. grep is the fallback ONLY when the index
returns nothing — and when that happens, say so (the indexer missed something
worth fixing) and refresh with `palugada index`.
