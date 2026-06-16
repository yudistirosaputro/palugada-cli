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
    throw new Error(
      `palugada: failed to run 'tar' (${r.error.message}). Install tar or download manually from https://github.com/${OWNER_REPO}/releases`
    );
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
