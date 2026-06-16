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
