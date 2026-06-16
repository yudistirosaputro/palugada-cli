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
