<!-- palugada:start -->
# Working with palugada

This project uses **palugada** for token-cheap, always-current engineering
context — ask palugada instead of re-reading lots of files.

**Before** grepping for code (`grep`/`find`/`rg`/Glob), use the index:
`palugada symbol <name>` / `palugada fact <family>`.

On-demand skills (loaded by trigger):
- `palugada-search` — locate code/symbols (use before grep)
- `palugada-bugfix` / `-feature` / `-refactor` / `-review` — scoped task packs via `palugada brief`
- `palugada-git` — git, PR/MR, commit conventions
- `palugada-docs` — issues, wiki pages, PRDs

Discover: `palugada q --list` (conventions) · `palugada for --list` (recipes) · `palugada <cmd> --help`.
Bound profile: `rust-cli` — switch with `palugada profile use <id>` (skills follow the active profile automatically).
<!-- palugada:end -->
