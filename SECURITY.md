# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately — do **not** open a public
issue for anything exploitable. Use GitHub's **Security → Report a
vulnerability** (private advisory) on `yudistirosaputro/palugada-cli`, or email
the maintainer. Include repro steps and the version (`palugada --version`).
We aim to acknowledge within a few days.

## Threat model

palugada is a **local, single-user developer CLI**. It holds API tokens for
your connectors and executes shell verbs. The trust boundaries that matter:

### Credentials
- Tokens live only in `~/.palugada/secrets.yaml` (written `0600`, atomic
  rename) and are referenced from project config by auth-profile **name** —
  never committed.
- `config show` / the web console show tokens **masked** (presence + length);
  plaintext is never printed or sent to the browser.
- Auth is header-borne (Bearer / Basic / provider header), never in URLs. The
  one URL-carried secret is a **Slack webhook**; error messages redact URLs to
  `scheme://host/…` so the webhook cannot leak into logs.

### `palugada web` (the authoring console)
- Binds **loopback only** (`127.0.0.1`) and accepts only `localhost` Host
  headers (defeats DNS-rebinding).
- Every `/api/*` request requires a **per-session token** (256-bit CSPRNG)
  injected into the served page, plus a same-origin `Sec-Fetch-Site`/`Origin`
  check. A cross-origin web page cannot read the served HTML, so it cannot
  learn the token — this is what stops CSRF from a malicious page you have open
  while the console runs.
- On a **shared multi-user host**, any local user who can reach the loopback
  port during a session is in the trust boundary. Run the console only on
  machines where you trust local users.

### Repo-defined `exec` verbs (supply chain)
- Verbs that ship **with palugada** (profile-bundled) are trusted.
- Verbs defined in a **cloned repo's** `.palugada/config.yaml` run a shell
  command from that checkout. They are gated by **trust-on-first-use**: shown
  once, approved, and cached by `(repo, verb, exact command)` in
  `~/.palugada/exec-trust.json`. Editing a verb re-prompts.
- Approve without a prompt (trusted CI) via `--yes` or
  `PALUGADA_TRUST_REPO_EXEC=1`. **AI agents should not auto-approve** verbs from
  untrusted repositories.
- `exec` interpolates `k=v` **values** into the shell string, so treat verb
  arguments as you would any shell input.

### Installer & releases
- `install.sh` verifies the downloaded archive's **sha256** against the
  published `.sha256` sidecar and supports pinning a version with
  `PALUGADA_VERSION`. `PALUGADA_SKIP_CHECKSUM=1` bypasses the check (unsafe).
- Release archives are downloaded over HTTPS from GitHub Releases.

### TLS
- `--insecure` disables certificate verification for **every host** contacted
  in that run — use it only against a known self-signed host, prefer pinning
  the corporate CA. Per-host scoping (`--insecure-host`) is planned.

## Supported versions

Security fixes target the latest released `0.2.x`. Update with
`npm install -g palugada-cli@latest`, `brew upgrade`, `scoop update`, or the
installer.
