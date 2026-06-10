// Assemble publishable npm packages from extracted release archives.
//
//   node npm/build-npm.mjs <version> [distDir]
//
// <distDir> (default ./dist) holds one extracted archive per target:
//   <distDir>/palugada-<triple>/{palugada|palugada.exe, knowledge/}
//
// Output: npm/.staging/<pkg>/ for each available platform, plus the main
// `palugada-cli` package with versions + optionalDependencies pinned. The CI
// release job then runs `npm publish` in each staging dir.

import { existsSync, mkdirSync, cpSync, writeFileSync, rmSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url)); // .../npm
const version = process.argv[2];
const distDir = process.argv[3] || join(here, '..', 'dist');

if (!version) {
  console.error('usage: node build-npm.mjs <version> [distDir]');
  process.exit(1);
}

const TARGETS = [
  { triple: 'x86_64-unknown-linux-gnu', pkg: 'palugada-cli-linux-x64', os: 'linux', cpu: 'x64', exe: 'palugada' },
  { triple: 'aarch64-apple-darwin', pkg: 'palugada-cli-darwin-arm64', os: 'darwin', cpu: 'arm64', exe: 'palugada' },
  { triple: 'x86_64-apple-darwin', pkg: 'palugada-cli-darwin-x64', os: 'darwin', cpu: 'x64', exe: 'palugada' },
  { triple: 'x86_64-pc-windows-msvc', pkg: 'palugada-cli-win32-x64', os: 'win32', cpu: 'x64', exe: 'palugada.exe' },
];

const staging = join(here, '.staging');
rmSync(staging, { recursive: true, force: true });
mkdirSync(staging, { recursive: true });

const built = [];
for (const t of TARGETS) {
  const src = join(distDir, `palugada-${t.triple}`);
  const exeSrc = join(src, t.exe);
  if (!existsSync(exeSrc)) {
    console.warn(`skip ${t.pkg}: ${exeSrc} not found`);
    continue;
  }
  const out = join(staging, t.pkg);
  mkdirSync(out, { recursive: true });
  cpSync(exeSrc, join(out, t.exe));
  cpSync(join(src, 'knowledge'), join(out, 'knowledge'), { recursive: true });
  writeFileSync(
    join(out, 'package.json'),
    JSON.stringify(
      {
        name: t.pkg,
        version,
        description: `palugada prebuilt binary for ${t.os}-${t.cpu}`,
        homepage: 'https://github.com/yudistirosaputro/palugada-cli',
        license: 'MIT',
        os: [t.os],
        cpu: [t.cpu],
        files: [t.exe, 'knowledge'],
      },
      null,
      2
    ) + '\n'
  );
  built.push(t.pkg);
}

if (built.length === 0) {
  console.error(`no target archives found under ${distDir} — nothing to publish`);
  process.exit(1);
}

// Main package: copy template, pin version + optionalDependencies.
const mainOut = join(staging, 'palugada-cli');
cpSync(join(here, 'palugada-cli'), mainOut, { recursive: true });
const pkg = JSON.parse(readFileSync(join(mainOut, 'package.json'), 'utf8'));
pkg.version = version;
pkg.optionalDependencies = Object.fromEntries(built.map((name) => [name, version]));
writeFileSync(join(mainOut, 'package.json'), JSON.stringify(pkg, null, 2) + '\n');

console.log(`Assembled ${built.length} platform package(s) + palugada-cli @ ${version}`);
console.log(`  ${built.join('\n  ')}`);
