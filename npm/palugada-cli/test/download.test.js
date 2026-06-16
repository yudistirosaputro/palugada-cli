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
