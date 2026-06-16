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
