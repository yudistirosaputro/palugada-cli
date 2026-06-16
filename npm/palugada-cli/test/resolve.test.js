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
