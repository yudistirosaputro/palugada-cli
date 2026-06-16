# npm single-package with download-on-install

**Date:** 2026-06-16
**Status:** approved (design)
**Branch:** `feat/npm-single-package-download`

## Problem

`npm install -g palugada-cli` returns 404 — the package was never published.
The release workflow for `v0.1.0` ran fine (token + builds OK), but the npm job
failed at **publish**:

```
npm error code E403
403 Forbidden - PUT .../palugada-cli-win32-x64 - Package name triggered spam detection
```

The old design publishes **five** similarly-named packages (`palugada-cli` plus
four `palugada-cli-<platform>` binary packages wired via `optionalDependencies`).
npm's anti-spam heuristic flagged the 4th package. Three platform packages did
publish at `0.1.0` (`linux-x64`, `darwin-arm64`, `darwin-x64`); `win32-x64` was
blocked and the main `palugada-cli` package was never reached.

So a plain re-run cannot work: the unscoped name family is on npm's spam radar
and three sub-packages already occupy `0.1.0`.

## Goal

Publish a working `npm install -g palugada-cli` that keeps the short, unscoped
name while minimizing spam-detection risk, by collapsing distribution to a
**single** npm package that downloads the matching prebuilt binary from the
GitHub Release at install time and verifies it against an embedded checksum.

Non-goals: changing the Rust binary, Homebrew/Scoop channels, or the
`release` build matrix (beyond adding one Windows artifact).

## Approach

One published package `palugada-cli` containing only JavaScript. On install, a
`postinstall` script resolves the platform, downloads
`palugada-<triple>.tar.gz` from the matching GitHub Release, verifies its
SHA-256 against a bundled `checksums.json`, and extracts it into `vendor/`.
The `bin` launcher execs the extracted binary, with a lazy-download fallback if
`postinstall` was skipped (`npm ci --ignore-scripts`).

Single package → far lower spam risk. Zero runtime/install npm dependencies
(extraction uses the OS `tar`, present on macOS/Linux and Windows 10+ bsdtar).

## Components — `npm/palugada-cli/`

| File | Responsibility |
|---|---|
| `package.json` | name `palugada-cli`; `bin.palugada → bin/palugada.js`; `scripts.postinstall: node install.js`; `dependencies: {}`. **Remove** `optionalDependencies` and any `os`/`cpu` gating. `files` lists `bin/`, `lib/`, `install.js`, `checksums.json`, `README.md`. |
| `lib/resolve.js` | Pure functions: `triple(platform, arch)`, `assetName(triple)`, `assetUrl(version, triple)`, `binName(platform)`. No I/O — unit-testable. Throws a clear error for unsupported `platform-arch`. |
| `lib/download.js` | `ensureBinary()` → returns path to the runnable binary, downloading + verifying + extracting if absent. Download to a temp file, compute SHA-256, compare to `checksums.json[triple]`; on mismatch delete and throw. Extract via `spawnSync('tar', ['-xzf', tmp, '-C', tmpDir])` then atomically rename `tmpDir` → `vendor/`. Idempotent (no-op if `vendor/<bin>` already present). |
| `install.js` | postinstall entry. Skip if `PALUGADA_SKIP_DOWNLOAD` set, or binary already present. Otherwise `ensureBinary()`. On failure: print an actionable **warning** and exit 0 (non-fatal — the launcher retries). Unsupported platform: warn + exit 0. |
| `bin/palugada.js` | Launcher. `const bin = ensureBinary()` (lazy download if missing). Set `PALUGADA_KNOWLEDGE` to `vendor/knowledge`. Set `HOME` from `os.homedir()` on Windows if unset (existing behavior). `spawnSync(bin, argv.slice(2), {stdio:'inherit'})`; propagate exit code. |
| `checksums.json` | `{ "<triple>": "<sha256>" }` — **generated at publish time** from the release `.sha256` assets and written into the package before `npm publish`, so the trusted hash ships inside the npm tarball (not fetched from the same origin as the binary). |

Platform triples:

| node platform-arch | triple | asset |
|---|---|---|
| `linux-x64` | `x86_64-unknown-linux-gnu` | `palugada-x86_64-unknown-linux-gnu.tar.gz` |
| `darwin-arm64` | `aarch64-apple-darwin` | `palugada-aarch64-apple-darwin.tar.gz` |
| `darwin-x64` | `x86_64-apple-darwin` | `palugada-x86_64-apple-darwin.tar.gz` |
| `win32-x64` | `x86_64-pc-windows-msvc` | `palugada-x86_64-pc-windows-msvc.tar.gz` |

`vendor/` (binary + extracted `knowledge/`) is created at install time on the
user's machine and is never committed.

## Install / run flow

```
npm install -g palugada-cli
  └─ postinstall (install.js): resolve triple → download v<ver>/palugada-<triple>.tar.gz
       → verify sha256 (checksums.json) → tar -xzf → rename → vendor/

palugada q --list
  └─ bin/palugada.js: vendor/palugada present? → exec (PALUGADA_KNOWLEDGE=vendor/knowledge)
       absent (postinstall skipped) → ensureBinary() downloads first, then exec
```

Download URL: `https://github.com/yudistirosaputro/palugada-cli/releases/download/v${version}/palugada-${triple}.tar.gz`,
where `version` is read from the package's own `package.json` — so the npm
version and the release tag are always in lockstep.

## `release.yml` changes

- **`release` job (Windows packaging):** in addition to the existing `.zip`
  (still used by Scoop), also emit `palugada-x86_64-pc-windows-msvc.tar.gz` and
  its `.sha256`, and attach both to the release.
- **`npm` job (rewrite):** keep the `NPM_TOKEN` gate. Download the release
  `*.tar.gz.sha256` assets, assemble `checksums.json` keyed by triple, set the
  package `version` from the tag (strip leading `v`), then
  `npm publish --access public` the single `npm/palugada-cli` package.
- `npm/build-npm.mjs` (the old multi-package assembler) is removed or reduced to
  a small `checksums.json` + version generator invoked by the npm job.

## Version & release

Bump `Cargo.toml` and `npm/palugada-cli/package.json` to **0.1.1**; commit;
`git tag v0.1.1 && git push origin v0.1.1`. The release job builds the archives
(including the new Windows `.tar.gz`) and the npm job publishes
`palugada-cli@0.1.1`.

## Error handling

| Condition | Behavior |
|---|---|
| Unsupported platform/arch | Clear message naming the platform + GitHub Releases URL. |
| Download failure (network/proxy) | postinstall: non-fatal warning (install still succeeds); launcher retries on first run; if still failing, actionable error (set `HTTPS_PROXY` / `PALUGADA_SKIP_DOWNLOAD` and download manually). Node does not auto-honor `HTTPS_PROXY`; proxy users are pointed at the manual path — we stay zero-dep. |
| Checksum mismatch | Delete the partial download; throw a security-relevant error. |
| `tar` missing (very old Windows) | Clear error + manual-install / upgrade hint. |

## Testing

- **Unit:** `lib/resolve.js` — triple/asset/URL resolution and the unsupported
  case. (Plain Node assertions; no test framework dependency.)
- **Local end-to-end (darwin-arm64, this machine):** `npm pack`, install the
  tarball into a temp global prefix, run `palugada q --list` — exercises
  postinstall download + launcher.
- **Skip path:** `PALUGADA_SKIP_DOWNLOAD=1` install succeeds with no binary;
  first `palugada` run lazily downloads.
- **Checksum mismatch:** corrupt `checksums.json` → install/run fails with the
  expected error.
- The actual publish can only be verified by cutting `v0.1.1`; everything else
  is verified locally first.

## Cleanup (optional follow-up, non-blocking)

`npm deprecate` the three orphaned `palugada-cli-{linux-x64,darwin-arm64,darwin-x64}@0.1.0`
sub-packages with a note that they moved into `palugada-cli`.

## Risk

If the bare name `palugada-cli` itself trips spam detection on publish, the
fallback is the scoped name `@yudistirosaputro/palugada-cli` — a small rename of
`package.json` `name` and the launcher's release-owner constant from here.
