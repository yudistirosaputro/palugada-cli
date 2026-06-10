# Publishing & distribution

palugada ships as prebuilt, self-contained archives (binary **+** bundled
`knowledge/` profiles). Everything is driven by one tag push.

## Cut a release

```bash
# bump version in Cargo.toml first (e.g. 0.1.0), commit, then:
git tag v0.1.0
git push origin v0.1.0
```

`.github/workflows/release.yml` then:

1. **`release`** — builds `cargo build --release` on native runners for
   `x86_64-unknown-linux-gnu`, `aarch64-apple-darwin`, `x86_64-apple-darwin`,
   `x86_64-pc-windows-msvc`; packages each as `palugada-<target>.tar.gz`
   (`.zip` on Windows) containing the binary + `knowledge/` + `examples/` +
   `README.md`; attaches them (with `.sha256`) to the GitHub Release.
2. **`npm` / `homebrew` / `scoop`** — download those archives and republish to
   each package manager. **Each is gated on a secret and skips cleanly if the
   secret is absent**, so you can enable channels one at a time.

You can also run the workflow manually (Actions → Release → Run workflow) and
pass the tag as input.

## Channel setup (one-time, opt-in)

### npm
1. Create an npm account; ensure the package name `palugada-cli` is available
   (or change `name` in `npm/palugada-cli/package.json` and the package names in
   `npm/build-npm.mjs` + `bin/palugada.js` to a scope like `@you/palugada`).
2. Create an **automation** access token (npmjs → Access Tokens).
3. Add it as the repo secret **`NPM_TOKEN`**.

Publishes `palugada-cli` plus four platform packages (`palugada-cli-linux-x64`,
`-darwin-arm64`, `-darwin-x64`, `-win32-x64`). Users then:

```bash
npm install -g palugada-cli   # or: npx palugada-cli q --list
```

### Homebrew tap
1. Create a repo named **`homebrew-tap`** under the same owner
   (`yudistirosaputro/homebrew-tap`).
2. Create a PAT (fine-grained, Contents: read/write on that repo) and add it as
   the repo secret **`HOMEBREW_TAP_TOKEN`**.

The release job renders `packaging/homebrew/palugada.rb` (filling version +
sha256) and commits it to `Formula/palugada.rb` in the tap. Users:

```bash
brew install yudistirosaputro/tap/palugada
```

### Scoop bucket (Windows)
1. Create a repo named **`scoop-bucket`** under the same owner.
2. Create a PAT (Contents: read/write on that repo) and add it as the repo
   secret **`SCOOP_BUCKET_TOKEN`**.

The release job renders `packaging/scoop/palugada.json` into `bucket/palugada.json`.
Users:

```powershell
scoop bucket add palugada https://github.com/yudistirosaputro/scoop-bucket
scoop install palugada
```

## How the bundled knowledge is found

Every channel keeps `knowledge/` next to the binary:

- **Archive / `install.sh` / Homebrew / Scoop** — the binary canonicalizes its
  own path and walks up to find `knowledge/profiles` (`src/knowledge.rs`), so a
  symlinked launcher on `PATH` still resolves it.
- **npm** — the JS launcher sets `PALUGADA_KNOWLEDGE` to the bundled dir; Scoop
  sets it via `env_set`.

## License metadata

palugada is MIT-licensed (`LICENSE`). The `license` field is set to `MIT` in
`Cargo.toml`, the npm packages (`npm/build-npm.mjs` + `npm/palugada-cli`), the
Homebrew formula, and the Scoop manifest.
