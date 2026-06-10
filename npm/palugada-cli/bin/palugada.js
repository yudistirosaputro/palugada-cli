#!/usr/bin/env node
'use strict';

// Thin launcher: resolve the platform-specific package npm installed (via
// optionalDependencies + os/cpu gating), then exec the bundled native binary.
// We set PALUGADA_KNOWLEDGE to the knowledge/ dir shipped inside that package so
// `q`/`for`/`s`/`brief` work regardless of where npm placed the files, and set
// HOME on Windows (the binary needs a home dir; Windows uses %USERPROFILE%).

const { spawnSync } = require('node:child_process');
const path = require('node:path');
const os = require('node:os');

const PLATFORM_PACKAGES = {
  'linux-x64': 'palugada-cli-linux-x64',
  'darwin-arm64': 'palugada-cli-darwin-arm64',
  'darwin-x64': 'palugada-cli-darwin-x64',
  'win32-x64': 'palugada-cli-win32-x64',
};

const key = `${process.platform}-${process.arch}`;
const pkgName = PLATFORM_PACKAGES[key];

if (!pkgName) {
  console.error(
    `palugada: no prebuilt binary for ${key}.\n` +
      `Build from source or see https://github.com/yudistirosaputro/palugada-cli/releases`
  );
  process.exit(1);
}

let pkgDir;
try {
  pkgDir = path.dirname(require.resolve(`${pkgName}/package.json`));
} catch {
  console.error(
    `palugada: the platform package "${pkgName}" is not installed.\n` +
      `This usually means npm ran with --no-optional or --ignore-scripts.\n` +
      `Reinstall normally, or install it directly:\n  npm install ${pkgName}`
  );
  process.exit(1);
}

const exeName = process.platform === 'win32' ? 'palugada.exe' : 'palugada';
const binPath = path.join(pkgDir, exeName);

const env = { ...process.env };
if (!env.PALUGADA_KNOWLEDGE) {
  env.PALUGADA_KNOWLEDGE = path.join(pkgDir, 'knowledge');
}
if (!env.HOME && !env.USERPROFILE) {
  env.HOME = os.homedir();
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: 'inherit', env });
if (result.error) {
  console.error(`palugada: failed to launch ${binPath}: ${result.error.message}`);
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
