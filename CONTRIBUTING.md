# Contributing to palugada

Thanks for helping improve palugada — a single Rust binary that gives any
project a developer-knowledge layer and provider-agnostic connectors. This
guide covers the local workflow and how to add the most common changes.

## Prerequisites

A stable Rust toolchain (`rustup`, tested with 1.94). If you don't have one:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Build, test, lint

```bash
cargo build              # debug build
cargo build --release    # optimized binary at target/release/palugada
cargo test               # unit tests
cargo fmt --all          # format
cargo clippy --all-targets   # lint
```

Run `cargo fmt` and `cargo clippy` before opening a PR. CI
(`.github/workflows/ci.yml`) builds, tests, and smoke-tests on Linux, macOS,
and Windows and must be green to merge.

Try the CLI without installing — run from inside the repo so it finds the
bundled profiles:

```bash
cargo run -- q --list
cargo run -- doctor
```

## Project layout

See the **Layout** section of the [README](README.md). In short:

| Path | Area |
|---|---|
| `src/main.rs` | clap dispatch + command handlers |
| `src/config.rs` | global/project config, secrets, resolution |
| `src/knowledge.rs` | `q` / `for` / `s` |
| `src/indexer.rs` | `index` / `symbol` |
| `src/brief.rs` | `brief` flow packs |
| `src/exec.rs` | `exec` / `doctor` verbs |
| `src/http.rs` | synchronous HTTP (ureq) |
| `src/clients/` | connector traits + per-provider impls |
| `knowledge/profiles/` | bundled stack profiles |

## Coding conventions

- Match the style of the surrounding code; keep modules focused.
- No async runtime — HTTP is synchronous via `ureq`.
- Command handlers return `Result<(), String>`; user-facing errors print as
  `error: …` and exit non-zero.
- Cover new behavior with a unit test where practical.

## Commit & PR conventions

- Use a conventional prefix in commit subjects: `feat:`, `fix:`, `chore:`,
  `docs:`. Keep commits atomic and the subject in the imperative mood.
- Branch from `main`; open a PR into `main`, describe the change and how you
  tested it, and wait for CI.

## How to add common things

**A connector provider** (e.g. a new issue tracker):
1. Add `src/clients/<provider>.rs` implementing the relevant trait from
   `src/clients/mod.rs` (`IssueTracker` / `DocSource` / `GitHost` /
   `DesignSource` / `CiProvider`), including `verify()`.
2. Wire it into the matching factory in `src/clients/mod.rs` and add the
   provider name to that factory's `unsupported …` "supported:" list.
3. Update the provider list in the README.

**A knowledge profile**: add `knowledge/profiles/<id>/` with its convention and
recipe docs, `_index.json`, `extractors.yaml`, and optional `exec:` / `flows:`
maps. Stack auto-detection lives in `src/scaffold.rs`.

**An `exec` verb or `brief` flow**: add it to the profile's `exec:` / `flows:`
map (or a project's `.palugada/config.yaml`) — verbs are data, no code change
needed.

## Releases

Maintainers: see [docs/PUBLISHING.md](docs/PUBLISHING.md) for cutting a tagged
release and the npm / Homebrew / Scoop publishing setup.

## License

By contributing, you agree that your contributions are licensed under the
project's [MIT License](LICENSE).
