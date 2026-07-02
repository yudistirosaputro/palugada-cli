# Changelog

All notable changes to palugada are documented here. Versions follow semver;
dates are YYYY-MM-DD.

## [0.2.4] - 2026-07-02 — Security hardening

Fixes surfaced by a full security audit. No breaking changes to normal use.

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
  `.palugada/config.yaml` are now gated by trust-on-first-use before running;
  bundled profile verbs stay trusted. `--yes` / `PALUGADA_TRUST_REPO_EXEC=1`
  approve non-interactively.
- **secret-in-URL leak.** Error messages redact URLs to `scheme://host/…`, so a
  Slack webhook (secret in the URL) no longer leaks into logs. The `--insecure`
  warning now states it disables TLS verification for every host that run.

### Added
- `SECURITY.md` (threat model + private reporting) and this changelog.

[0.2.4]: https://github.com/yudistirosaputro/palugada-cli/releases/tag/v0.2.4
