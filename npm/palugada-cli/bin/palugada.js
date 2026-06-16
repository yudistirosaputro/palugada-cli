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
