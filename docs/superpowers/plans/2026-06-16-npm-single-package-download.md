# npm single-package download-on-install — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `npm install -g palugada-cli` work by collapsing npm distribution to a single package that downloads + SHA-256-verifies the prebuilt binary from the GitHub Release at install time.

**Architecture:** One JS-only package `palugada-cli`. `postinstall` (and a lazy fallback in the `bin` launcher) resolves the platform, downloads `palugada-<triple>.tar.gz` from the release matching the package version, verifies it against a bundled `checksums.json`, and extracts it into `vendor/`. The launcher execs the extracted binary with `PALUGADA_KNOWLEDGE` pointed at `vendor/knowledge`. Zero npm dependencies; extraction uses the OS `tar`.

**Tech Stack:** Node.js (`node:https`/`crypto`/`child_process`/`fs`), `node:test`, GitHub Actions, system `tar`.

Spec: `docs/superpowers/specs/2026-06-16-npm-single-package-download-design.md`

---

## File structure (npm/palugada-cli/)

| File | Responsibility | Action |
|---|---|---|
| `lib/resolve.js` | Pure platform→triple/asset/URL resolution | Create |
| `lib/download.js` | `ensureBinary()` download+verify+extract; `sha256File` | Create |
| `install.js` | postinstall entry (non-fatal) | Create |
| `bin/palugada.js` | launcher: ensureBinary + exec | Rewrite |
| `package.json` | single package; postinstall; no deps/optionalDeps | Modify |
| `.gitignore` | ignore `vendor/`, `checksums.json` | Create |
| `test/resolve.test.js` | unit tests for resolve | Create |
| `test/download.test.js` | unit test for sha256File | Create |
| `README.md` | install/usage docs | Modify |
| `../gen-checksums.mjs` | build `checksums.json` from release `.sha256` | Create |
| `../build-npm.mjs` | old multi-package assembler | Delete |
| `../../.github/workflows/release.yml` | Windows `.tar.gz` + npm job rewrite | Modify |
| `../../Cargo.toml` | bump version to 0.1.1 | Modify |
| `../../docs/PUBLISHING.md` | npm section rewrite | Modify |

---

## Task 1: `lib/resolve.js` + unit tests (TDD)

**Files:**
- Create: `npm/palugada-cli/lib/resolve.js`
- Test: `npm/palugada-cli/test/resolve.test.js`

- [ ] **Step 1: Write the failing tests**

Create `npm/palugada-cli/test/resolve.test.js`:

```js
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const { triple, assetName, assetUrl, binName } = require('../lib/resolve');

test('triple resolves supported platforms', () => {
  assert.equal(triple('linux', 'x64'), 'x86_64-unknown-linux-gnu');
  assert.equal(triple('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.equal(triple('darwin', 'x64'), 'x86_64-apple-darwin');
  assert.equal(triple('win32', 'x64'), 'x86_64-pc-windows-msvc');
});

test('triple throws on unsupported platform', () => {
  assert.throws(() => triple('linux', 'arm64'), /no prebuilt binary for linux-arm64/);
});

test('assetName + assetUrl', () => {
  const t = 'aarch64-apple-darwin';
  assert.equal(assetName(t), 'palugada-aarch64-apple-darwin.tar.gz');
  assert.equal(
    assetUrl('0.1.1', t),
    'https://github.com/yudistirosaputro/palugada-cli/releases/download/v0.1.1/palugada-aarch64-apple-darwin.tar.gz'
  );
});

test('assetUrl honors PALUGADA_RELEASE_TAG override', () => {
  process.env.PALUGADA_RELEASE_TAG = 'v0.1.0';
  try {
    assert.match(assetUrl('9.9.9', 'aarch64-apple-darwin'), /download\/v0\.1\.0\//);
  } finally {
    delete process.env.PALUGADA_RELEASE_TAG;
  }
});

test('binName', () => {
  assert.equal(binName('darwin'), 'palugada');
  assert.equal(binName('win32'), 'palugada.exe');
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd npm/palugada-cli && node --test`
Expected: FAIL — `Cannot find module '../lib/resolve'`.

- [ ] **Step 3: Implement `lib/resolve.js`**

Create `npm/palugada-cli/lib/resolve.js`:

```js
'use strict';

// Pure platform/asset resolution — no I/O, unit-testable.

const OWNER_REPO = 'yudistirosaputro/palugada-cli';

const TRIPLES = {
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'win32-x64': 'x86_64-pc-windows-msvc',
};

function triple(platform, arch) {
  const key = `${platform}-${arch}`;
  const t = TRIPLES[key];
  if (!t) {
    throw new Error(
      `palugada: no prebuilt binary for ${key}. ` +
        `Supported: ${Object.keys(TRIPLES).join(', ')}. ` +
        `See https://github.com/${OWNER_REPO}/releases`
    );
  }
  return t;
}

function assetName(t) {
  return `palugada-${t}.tar.gz`;
}

function releaseTag(version) {
  return process.env.PALUGADA_RELEASE_TAG || `v${version}`;
}

function assetUrl(version, t) {
  return `https://github.com/${OWNER_REPO}/releases/download/${releaseTag(version)}/${assetName(t)}`;
}

function binName(platform) {
  return platform === 'win32' ? 'palugada.exe' : 'palugada';
}

module.exports = { triple, assetName, assetUrl, releaseTag, binName, TRIPLES, OWNER_REPO };
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd npm/palugada-cli && node --test`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add npm/palugada-cli/lib/resolve.js npm/palugada-cli/test/resolve.test.js
git commit -m "feat(npm): pure platform/asset resolver for single-package install

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 2: `lib/download.js` + sha256 test

**Files:**
- Create: `npm/palugada-cli/lib/download.js`
- Test: `npm/palugada-cli/test/download.test.js`

- [ ] **Step 1: Write the failing test**

Create `npm/palugada-cli/test/download.test.js`:

```js
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const crypto = require('node:crypto');
const { sha256File } = require('../lib/download');

test('sha256File matches node:crypto', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pt-'));
  const f = path.join(dir, 'x');
  fs.writeFileSync(f, 'hello palugada');
  const want = crypto.createHash('sha256').update('hello palugada').digest('hex');
  assert.equal(sha256File(f), want);
  fs.rmSync(dir, { recursive: true, force: true });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd npm/palugada-cli && node --test test/download.test.js`
Expected: FAIL — `Cannot find module '../lib/download'`.

- [ ] **Step 3: Implement `lib/download.js`**

Create `npm/palugada-cli/lib/download.js`:

```js
'use strict';

// Download + verify + extract the prebuilt binary into vendor/. Used by both
// install.js (postinstall) and bin/palugada.js (lazy fallback). No side effects
// on import — version/package.json are read lazily inside ensureBinary().

const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const https = require('node:https');
const crypto = require('node:crypto');

const { triple, assetName, assetUrl, binName, OWNER_REPO } = require('./resolve');

const PKG_DIR = path.join(__dirname, '..'); // npm/palugada-cli/
const VENDOR = path.join(PKG_DIR, 'vendor');

function version() {
  return require(path.join(PKG_DIR, 'package.json')).version;
}

function binaryPath() {
  return path.join(VENDOR, binName(process.platform));
}

function knowledgeDir() {
  return path.join(VENDOR, 'knowledge');
}

// GitHub release downloads 302-redirect to a CDN; follow up to 10 hops.
function httpGet(url, dest, redirects = 0) {
  return new Promise((resolve, reject) => {
    if (redirects > 10) return reject(new Error('too many redirects'));
    const req = https.get(url, { headers: { 'User-Agent': 'palugada-cli-installer' } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        res.resume();
        return resolve(httpGet(res.headers.location, dest, redirects + 1));
      }
      if (res.statusCode !== 200) {
        res.resume();
        return reject(new Error(`download failed: HTTP ${res.statusCode} for ${url}`));
      }
      const out = fs.createWriteStream(dest);
      res.pipe(out);
      out.on('finish', () => out.close(() => resolve()));
      out.on('error', reject);
    });
    req.on('error', reject);
  });
}

function sha256File(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function expectedSha(t) {
  const file = path.join(PKG_DIR, 'checksums.json');
  if (!fs.existsSync(file)) {
    throw new Error('palugada: checksums.json missing from the package — cannot verify the download');
  }
  const sha = JSON.parse(fs.readFileSync(file, 'utf8'))[t];
  if (!sha) throw new Error(`palugada: no checksum for ${t} in checksums.json`);
  return sha;
}

function extract(tarFile, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  const r = spawnSync('tar', ['-xzf', tarFile, '-C', destDir], { stdio: 'inherit' });
  if (r.error) {
    throw new Error(`palugada: failed to run 'tar' (${r.error.message}). Install tar or download manually from https://github.com/${OWNER_REPO}/releases`);
  }
  if (r.status !== 0) {
    throw new Error(`palugada: tar exited with status ${r.status} extracting ${tarFile}`);
  }
}

async function ensureBinary() {
  const bin = binaryPath();
  if (fs.existsSync(bin)) return bin;

  const t = triple(process.platform, process.arch); // throws on unsupported
  const url = assetUrl(version(), t);
  const dl = fs.mkdtempSync(path.join(os.tmpdir(), 'palugada-'));
  const tarFile = path.join(dl, assetName(t));
  const stage = path.join(PKG_DIR, 'vendor.tmp'); // same filesystem as VENDOR → rename is atomic

  try {
    await httpGet(url, tarFile);
    const got = sha256File(tarFile);
    const want = expectedSha(t);
    if (got !== want) {
      throw new Error(`palugada: checksum mismatch for ${assetName(t)} (got ${got}, want ${want})`);
    }
    fs.rmSync(stage, { recursive: true, force: true });
    extract(tarFile, stage);
    fs.rmSync(VENDOR, { recursive: true, force: true });
    fs.renameSync(stage, VENDOR);
  } finally {
    fs.rmSync(dl, { recursive: true, force: true });
    fs.rmSync(stage, { recursive: true, force: true });
  }

  if (!fs.existsSync(bin)) {
    throw new Error(`palugada: binary not found after extraction at ${bin}`);
  }
  return bin;
}

module.exports = { ensureBinary, binaryPath, knowledgeDir, sha256File, version };
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd npm/palugada-cli && node --test`
Expected: PASS (6 tests total).

- [ ] **Step 5: Commit**

```bash
git add npm/palugada-cli/lib/download.js npm/palugada-cli/test/download.test.js
git commit -m "feat(npm): download+verify+extract binary into vendor/

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 3: `install.js` (postinstall, non-fatal)

**Files:**
- Create: `npm/palugada-cli/install.js`

- [ ] **Step 1: Implement `install.js`**

Create `npm/palugada-cli/install.js`:

```js
#!/usr/bin/env node
'use strict';

// postinstall: fetch + verify + extract the prebuilt binary. Non-fatal on
// failure — bin/palugada.js retries lazily on first run.

const { ensureBinary } = require('./lib/download');

async function main() {
  if (process.env.PALUGADA_SKIP_DOWNLOAD) {
    console.error('palugada: PALUGADA_SKIP_DOWNLOAD set — skipping binary download.');
    return;
  }
  try {
    const bin = await ensureBinary();
    console.error(`palugada: installed binary at ${bin}`);
  } catch (err) {
    console.error(
      'palugada: could not download the prebuilt binary during install:\n' +
        `  ${err.message}\n` +
        "It will be retried automatically the first time you run 'palugada'.\n" +
        'If you are offline or behind a proxy, set PALUGADA_SKIP_DOWNLOAD=1 and\n' +
        'download manually from https://github.com/yudistirosaputro/palugada-cli/releases'
    );
    // Non-fatal: allow the install to succeed.
  }
}

main();
```

- [ ] **Step 2: Smoke-check it parses and the skip path works**

Run: `cd npm/palugada-cli && PALUGADA_SKIP_DOWNLOAD=1 node install.js`
Expected: prints `PALUGADA_SKIP_DOWNLOAD set — skipping binary download.` and exits 0.

- [ ] **Step 3: Commit**

```bash
git add npm/palugada-cli/install.js
git commit -m "feat(npm): non-fatal postinstall downloader

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 4: Rewrite `bin/palugada.js` launcher

**Files:**
- Modify (replace): `npm/palugada-cli/bin/palugada.js`

- [ ] **Step 1: Replace the launcher**

Replace the entire contents of `npm/palugada-cli/bin/palugada.js` with:

```js
#!/usr/bin/env node
'use strict';

// Resolve (downloading on first use if postinstall was skipped) the native
// binary, then exec it with the bundled knowledge/ dir wired in.

const { spawnSync } = require('node:child_process');
const os = require('node:os');
const { ensureBinary, knowledgeDir } = require('../lib/download');

async function main() {
  let bin;
  try {
    bin = await ensureBinary();
  } catch (err) {
    console.error(err.message);
    process.exit(1);
  }

  const env = { ...process.env };
  if (!env.PALUGADA_KNOWLEDGE) env.PALUGADA_KNOWLEDGE = knowledgeDir();
  if (!env.HOME && !env.USERPROFILE) env.HOME = os.homedir();

  const result = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit', env });
  if (result.error) {
    console.error(`palugada: failed to launch ${bin}: ${result.error.message}`);
    process.exit(1);
  }
  process.exit(result.status === null ? 1 : result.status);
}

main();
```

- [ ] **Step 2: Commit**

```bash
git add npm/palugada-cli/bin/palugada.js
git commit -m "feat(npm): launcher resolves binary via ensureBinary + lazy fallback

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 5: Update `package.json` + add `.gitignore`

**Files:**
- Modify: `npm/palugada-cli/package.json`
- Create: `npm/palugada-cli/.gitignore`

- [ ] **Step 1: Replace `package.json`**

Replace the entire contents of `npm/palugada-cli/package.json` with:

```json
{
  "name": "palugada-cli",
  "version": "0.1.1",
  "description": "Project-agnostic developer knowledge & connector CLI — one binary for stack conventions, task recipes, code indexing, and Jira/Confluence/GitLab/GitHub/Figma/Jenkins connectors.",
  "license": "MIT",
  "bin": {
    "palugada": "bin/palugada.js"
  },
  "scripts": {
    "postinstall": "node install.js",
    "test": "node --test"
  },
  "files": [
    "bin/palugada.js",
    "install.js",
    "lib/",
    "checksums.json",
    "README.md"
  ],
  "engines": {
    "node": ">=16"
  },
  "homepage": "https://github.com/yudistirosaputro/palugada-cli",
  "repository": {
    "type": "git",
    "url": "git+https://github.com/yudistirosaputro/palugada-cli.git"
  },
  "bugs": {
    "url": "https://github.com/yudistirosaputro/palugada-cli/issues"
  },
  "keywords": [
    "palugada",
    "cli",
    "android",
    "mvvm",
    "jira",
    "confluence",
    "connectors",
    "developer-tools"
  ]
}
```

Note: `optionalDependencies` and the four `palugada-cli-<platform>` packages are removed. (Test runner `node --test` needs Node ≥18; the runtime/postinstall use only Node ≥16 APIs.)

- [ ] **Step 2: Create `.gitignore`**

Create `npm/palugada-cli/.gitignore`:

```
vendor/
vendor.tmp/
checksums.json
*.tgz
```

- [ ] **Step 3: Commit**

```bash
git add npm/palugada-cli/package.json npm/palugada-cli/.gitignore
git commit -m "feat(npm): single-package manifest (no platform sub-packages)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 6: `gen-checksums.mjs` + delete `build-npm.mjs`

**Files:**
- Create: `npm/gen-checksums.mjs`
- Delete: `npm/build-npm.mjs`

- [ ] **Step 1: Create `npm/gen-checksums.mjs`**

```js
// Build checksums.json from release .sha256 files.
//   node npm/gen-checksums.mjs <dlDir> <outFile>
// dlDir holds palugada-<triple>.tar.gz.sha256 files (content: "<hash>  <name>").

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const dlDir = process.argv[2];
const outFile = process.argv[3];
if (!dlDir || !outFile) {
  console.error('usage: node gen-checksums.mjs <dlDir> <outFile>');
  process.exit(1);
}

const map = {};
for (const f of readdirSync(dlDir)) {
  const m = f.match(/^palugada-(.+)\.tar\.gz\.sha256$/);
  if (!m) continue;
  map[m[1]] = readFileSync(join(dlDir, f), 'utf8').trim().split(/\s+/)[0];
}
if (Object.keys(map).length === 0) {
  console.error(`no palugada-*.tar.gz.sha256 files found in ${dlDir}`);
  process.exit(1);
}
writeFileSync(outFile, JSON.stringify(map, null, 2) + '\n');
console.log(`wrote ${outFile} with ${Object.keys(map).length} checksum(s): ${Object.keys(map).join(', ')}`);
```

- [ ] **Step 2: Delete the old assembler**

Run: `git rm npm/build-npm.mjs`

- [ ] **Step 3: Commit**

```bash
git add npm/gen-checksums.mjs
git commit -m "feat(npm): gen-checksums.mjs; drop multi-package build-npm.mjs

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 7: Local end-to-end verification (darwin-arm64)

This task verifies the install/run plumbing against the **existing v0.1.0**
release assets (which contain real binaries + checksums), using
`PALUGADA_RELEASE_TAG=v0.1.0` to override the version→tag mapping. No code
changes — pure verification.

**Files:** none (generates ignored `checksums.json` + `*.tgz`).

- [ ] **Step 1: Generate `checksums.json` from the live v0.1.0 release**

```bash
cd npm/palugada-cli
rm -rf /tmp/pl-sha && mkdir -p /tmp/pl-sha
gh release download v0.1.0 --repo yudistirosaputro/palugada-cli -p 'palugada-*.tar.gz.sha256' -D /tmp/pl-sha --clobber
node ../gen-checksums.mjs /tmp/pl-sha checksums.json
cat checksums.json
```
Expected: `checksums.json` with 3 triples (linux-x64, darwin-arm64, darwin-x64 — v0.1.0 has no win32 tar.gz, fine for local mac test).

- [ ] **Step 2: Pack and install into a temp global prefix (postinstall path)**

```bash
cd npm/palugada-cli
npm pack
PREFIX=$(mktemp -d)
PALUGADA_RELEASE_TAG=v0.1.0 npm install -g --prefix "$PREFIX" ./palugada-cli-0.1.1.tgz
ls "$PREFIX/lib/node_modules/palugada-cli/vendor"   # expect: palugada, knowledge, examples, README.md
```
Expected: postinstall prints `installed binary at .../vendor/palugada`; `vendor/` contains the binary + `knowledge/`.

- [ ] **Step 3: Run the launcher — proves exec + knowledge wiring**

```bash
"$PREFIX/bin/palugada" --help | head -20
"$PREFIX/bin/palugada" q --list | head -20
```
Expected: `--help` exits 0 and shows the command list (includes `brief`, `q`, `fact`). `q --list` lists bundled android-mvvm conventions (proves `PALUGADA_KNOWLEDGE` → `vendor/knowledge` works).

- [ ] **Step 4: Verify the skip + lazy-download path**

```bash
PREFIX2=$(mktemp -d)
PALUGADA_SKIP_DOWNLOAD=1 npm install -g --prefix "$PREFIX2" ./palugada-cli-0.1.1.tgz
ls "$PREFIX2/lib/node_modules/palugada-cli" | grep -q vendor && echo "UNEXPECTED vendor" || echo "OK: no vendor after skip"
PALUGADA_RELEASE_TAG=v0.1.0 "$PREFIX2/bin/palugada" --help | head -5   # launcher lazily downloads now
ls "$PREFIX2/lib/node_modules/palugada-cli/vendor/palugada" && echo "OK: lazy download worked"
```
Expected: no `vendor/` right after the skipped install; first run downloads it; binary then present.

- [ ] **Step 5: Verify checksum-mismatch is rejected**

```bash
cd npm/palugada-cli
cp checksums.json /tmp/cs.bak
node -e "const f='checksums.json';const m=JSON.parse(require('fs').readFileSync(f));for(const k in m)m[k]='0'.repeat(64);require('fs').writeFileSync(f,JSON.stringify(m,null,2))"
npm pack
PREFIX3=$(mktemp -d)
PALUGADA_RELEASE_TAG=v0.1.0 npm install -g --prefix "$PREFIX3" ./palugada-cli-0.1.1.tgz   # postinstall warns (non-fatal)
PALUGADA_RELEASE_TAG=v0.1.0 "$PREFIX3/bin/palugada" --help; echo "exit=$?"
cp /tmp/cs.bak checksums.json   # restore good checksums
npm pack
```
Expected: install logs a checksum-mismatch warning (non-fatal); running the launcher exits non-zero with `checksum mismatch`.

- [ ] **Step 6: Clean up local artifacts (they are gitignored, but tidy up)**

```bash
cd npm/palugada-cli && rm -f palugada-cli-0.1.1.tgz
```
No commit (all artifacts gitignored). If any step failed, STOP and fix the relevant earlier task before continuing.

---

## Task 8: `release.yml` — Windows `.tar.gz` + npm job rewrite

**Files:**
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: Add `.tar.gz` to the Windows packaging step**

In the `Package (Windows)` step, after the `.zip.sha256` line, append:

```pwsh
          tar czf "palugada-${{ matrix.target }}.tar.gz" -C $stage .
          (Get-FileHash "palugada-${{ matrix.target }}.tar.gz" -Algorithm SHA256).Hash.ToLower() + "  palugada-${{ matrix.target }}.tar.gz" | Out-File -Encoding ascii "palugada-${{ matrix.target }}.tar.gz.sha256"
```

(The existing `Attach to release` step already lists `palugada-${{ matrix.target }}.tar.gz` and `.tar.gz.sha256`, so they are attached automatically.)

- [ ] **Step 2: Replace the entire `npm:` job**

Replace the `npm:` job (from `  npm:` down to just before `  homebrew:`) with:

```yaml
  npm:
    name: publish to npm
    needs: release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.inputs.tag || github.ref }}
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'
      - name: Gate on NPM_TOKEN
        id: gate
        env:
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: |
          if [ -z "$NPM_TOKEN" ]; then
            echo "NPM_TOKEN not set — skipping npm publish."
            echo "enabled=false" >> "$GITHUB_OUTPUT"
          else
            echo "enabled=true" >> "$GITHUB_OUTPUT"
          fi
      - name: Download release checksums
        if: steps.gate.outputs.enabled == 'true'
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          tag="${{ github.event.inputs.tag || github.ref_name }}"
          mkdir -p dl
          gh release download "$tag" --repo "$GITHUB_REPOSITORY" -p 'palugada-*.tar.gz.sha256' -D dl
      - name: Assemble & publish
        if: steps.gate.outputs.enabled == 'true'
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: |
          tag="${{ github.event.inputs.tag || github.ref_name }}"; ver="${tag#v}"
          node npm/gen-checksums.mjs dl npm/palugada-cli/checksums.json
          cat npm/palugada-cli/checksums.json
          cd npm/palugada-cli
          npm version "$ver" --no-git-tag-version --allow-same-version
          npm publish --access public
```

- [ ] **Step 3: Validate the YAML parses**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))" && echo OK`
Expected: `OK`.

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci(release): emit Windows .tar.gz; publish single npm package

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 9: Bump version to 0.1.1 (Cargo)

**Files:**
- Modify: `Cargo.toml`
- Modify (auto): `Cargo.lock`

- [ ] **Step 1: Bump the crate version**

In `Cargo.toml`, change `version = "0.1.0"` to `version = "0.1.1"`.

- [ ] **Step 2: Sync `Cargo.lock`**

Run: `cargo check`
Expected: builds; `Cargo.lock` now records `palugada` at `0.1.1`.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump to 0.1.1

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 10: Docs — `PUBLISHING.md` + `README.md`

**Files:**
- Modify: `docs/PUBLISHING.md`
- Modify: `npm/palugada-cli/README.md`

- [ ] **Step 1: Rewrite the npm section of `docs/PUBLISHING.md`**

Replace the `### npm` subsection (lines describing the four platform packages)
with text describing the single-package + download-on-install model:

```markdown
### npm
1. Create an npm account; ensure the package name `palugada-cli` is available
   (or change `name` in `npm/palugada-cli/package.json` + `OWNER_REPO` in
   `npm/palugada-cli/lib/resolve.js` to a scope like `@you/palugada-cli`).
2. Create an **automation** access token (npmjs → Access Tokens). A plain
   "publish" token will fail in CI if your account enforces 2FA.
3. Add it as the repo secret **`NPM_TOKEN`**.

The release job publishes a single `palugada-cli` package (JS only, zero
dependencies). On install its `postinstall` downloads the matching
`palugada-<triple>.tar.gz` from this release and verifies it against the
`checksums.json` bundled in the npm tarball (generated from the release
`.sha256` files at publish time). The `bin` launcher re-downloads lazily if
`postinstall` was skipped (`--ignore-scripts`); set `PALUGADA_SKIP_DOWNLOAD=1`
to opt out. Users:

\`\`\`bash
npm install -g palugada-cli   # or: npx palugada-cli q --list
\`\`\`
```

Also update the "How the bundled knowledge is found" npm bullet to:

```markdown
- **npm** — `postinstall` extracts the release tarball (binary + `knowledge/`)
  into the package's `vendor/`; the launcher sets `PALUGADA_KNOWLEDGE` to
  `vendor/knowledge`.
```

- [ ] **Step 2: Update `npm/palugada-cli/README.md`**

Ensure it documents: `npm install -g palugada-cli`, that the platform binary is
downloaded + checksum-verified on install, the `PALUGADA_SKIP_DOWNLOAD` escape
hatch, and a manual-download pointer to the GitHub Releases page for
offline/proxy users. Keep it short (it ships in the package).

- [ ] **Step 3: Commit**

```bash
git add docs/PUBLISHING.md npm/palugada-cli/README.md
git commit -m "docs: single-package npm distribution model

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Task 11: Final verification + release handoff

**Files:** none.

- [ ] **Step 1: Re-run the unit tests**

Run: `cd npm/palugada-cli && node --test`
Expected: PASS (6 tests).

- [ ] **Step 2: Confirm repo is clean of generated artifacts**

Run: `git status --porcelain` (expect empty) and
`git ls-files npm/palugada-cli | grep -E 'vendor|checksums.json|\.tgz' || echo "OK: no generated artifacts tracked"`
Expected: `OK`.

- [ ] **Step 3: Hand off the release cut**

The actual publish requires pushing the `v0.1.1` tag (triggers the release +
npm jobs). This is an outward-facing action — confirm with the user before
running:

```bash
git tag v0.1.1
git push origin v0.1.1
```

Then watch: `gh run watch --repo yudistirosaputro/palugada-cli` and verify
`npm view palugada-cli version` returns `0.1.1`.

---

## Self-Review

**Spec coverage:**
- Single JS package, no optionalDeps → Tasks 4, 5. ✓
- `lib/resolve.js` pure resolver → Task 1. ✓
- `lib/download.js` ensureBinary + checksum + atomic extract → Task 2. ✓
- `install.js` non-fatal postinstall + SKIP env → Task 3. ✓
- `bin/palugada.js` lazy fallback + PALUGADA_KNOWLEDGE → Task 4. ✓
- `checksums.json` generated at publish, gitignored → Tasks 5, 6, 8. ✓
- Windows `.tar.gz` added; npm job rewrite → Task 8. ✓
- Version 0.1.1 + tag → Tasks 5, 9, 11. ✓
- Error handling (unsupported/ download/ checksum/ tar) → Tasks 1, 2 (code) + Task 7 (verified). ✓
- Testing (unit + local e2e + skip + mismatch) → Tasks 1, 2, 7. ✓
- Cleanup of orphan sub-packages → out of plan scope (optional follow-up, noted in spec). ✓

**Placeholder scan:** No TBD/TODO; every code step has complete code. ✓

**Type consistency:** `ensureBinary`, `knowledgeDir`, `binaryPath`, `sha256File`, `version` exported from `lib/download.js` and consumed by `install.js`/`bin/palugada.js` with matching names. `triple`/`assetName`/`assetUrl`/`binName`/`OWNER_REPO` from `lib/resolve.js` consumed by `lib/download.js`. ✓
