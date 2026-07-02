# Changelog

All notable changes to palugada are documented here. Versions follow semver;
dates are YYYY-MM-DD.

## [0.3.0] - 2026-07-02 — Production-readiness: security, correctness, trust, tests

The output of a full multi-dimension audit (security, correctness, product,
tests, architecture). No breaking changes to normal use; the `brief --json`
output is now an object `{degraded, packs}` (was a bare array).

### Security
- **web console CSRF → token exfiltration (critical).** `palugada web` now
  requires a per-session CSPRNG token on every `/api/*` request and rejects
  cross-site `Sec-Fetch-Site`/`Origin`. Previously any web page open while the
  console ran could POST to `127.0.0.1` and, via the verify endpoints'
  saved-token fallback, exfiltrate a real connector token. Verify also refuses
  to send a stored token to a `base_url` it was not saved against.
- **installer integrity.** `install.sh` verifies the archive's sha256 against
  the published sidecar and supports `PALUGADA_VERSION` pinning
  (`PALUGADA_SKIP_CHECKSUM=1` to bypass, unsafe).
- **repo `exec` verbs (supply chain).** Verbs defined in a cloned repo's
  `.palugada/config.yaml` are now gated by trust-on-first-use; bundled profile
  verbs stay trusted. `--yes` / `PALUGADA_TRUST_REPO_EXEC=1` approve in CI.
- **secret-in-URL leak.** Errors redact URLs to `scheme://host/…` (incl. the
  transport-error path), so a Slack webhook no longer leaks into logs.

### Fixed
- **`extends` inheritance now works everywhere.** `project rules` / the web
  Effective Rules card showed zero conventions + false warnings for child
  profiles; `brief`/`exec` didn't inherit flows/review_map/exec. All read paths
  now resolve the chain. A mistyped `extends` fails `profile validate` instead
  of silently disabling inheritance.
- **indexer correctness.** Family rules match the repo-relative path (no phantom
  facts); a repo cloned into a dir named like an ignore entry (`build`/`target`)
  is still scanned; `.palugada/` is never self-indexed.
- **first run.** `init` auto-detects Rust (`Cargo.toml`) and Flutter
  (`pubspec.yaml`), drops the nonexistent `web-react` mapping, and hard-errors on
  an unknown profile instead of failing later with a raw OS error.
- **`doctor`** treats an unconfigured connector as SKIP, not FAIL — a fresh
  `init` + `doctor` now exits 0.
- **`brief <flow> <file>`** lists symbols defined in the file (was a dead
  name-match); a fully-degraded pack sets `"degraded": true` and exits 3.
- exec preserves its output tail on non-UTF8 bytes.
- Rust/Dart symbols now carry impl/trait scope + real signatures (was
  Kotlin-only).

### Added
- **`init` builds the local code index** by default so `symbol`/`brief` work
  immediately (`--no-index` to skip; offline).
- Index **staleness warning** ("N commits behind HEAD — run `palugada index`").
- **Tests**: 181 → 227, incl. e2e tests of the built binary, a web-console CSRF
  regression, and mocked-HTTP connector tests.
- **CI**: `clippy -D warnings` gate, per-bundled-profile `profile validate`,
  release tag↔version guard + per-target binary smoke.
- `SECURITY.md` (threat model + private reporting) and this changelog.
- Internals: one `ProfileManifest` type replaces 6 scattered `profile.yaml`
  parsers.

### Removed
- The `kmp` profile (an empty test artifact); the bundled profiles are now
  `android-mvvm`, `flutter-bloc`, `rust-cli`.

[0.3.0]: https://github.com/yudistirosaputro/palugada-cli/releases/tag/v0.3.0
