// WO-00 toolchain checks. Real checks only: the native-binary test is skipped
// with an explicit reason when the binary is not built, never passed vacuously.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const EXE = process.platform === 'win32' ? 'distinct-rs.exe' : 'distinct-rs';

function nativeBinary() {
  if (process.env.DISTINCT_RS_BIN) return process.env.DISTINCT_RS_BIN;
  for (const profile of ['debug', 'release']) {
    const p = join(root, 'target', profile, EXE);
    if (existsSync(p)) return p;
  }
  return null;
}

test('node major version is at least 20', () => {
  const major = Number(process.versions.node.split('.')[0]);
  assert.ok(major >= 20, `Node ${process.versions.node} is below the declared floor 20`);
});

test('cargo is present', () => {
  const r = spawnSync('cargo', ['--version'], { cwd: root, encoding: 'utf8' });
  assert.equal(r.status, 0, `cargo --version failed: ${r.error ?? r.stderr}`);
  assert.match(r.stdout, /^cargo \d+\.\d+\.\d+/);
});

const bin = nativeBinary();
test(
  'distinct-rs reports protocol distinct/1.0 and version',
  { skip: bin ? false : 'SKIPPED: distinct-rs not built (run `cargo build -p distinct-cli` or set DISTINCT_RS_BIN)' },
  () => {
    const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));
    const v = spawnSync(bin, ['--version'], { encoding: 'utf8' });
    assert.equal(v.status, 0);
    assert.equal(v.stdout, `distinct-rs ${pkg.version}\n`);
    const p = spawnSync(bin, ['protocol'], { encoding: 'utf8' });
    assert.equal(p.status, 0);
    const env = JSON.parse(p.stdout);
    assert.equal(env.protocol_version, 'distinct/1.0');
    assert.equal(env.engine_version, pkg.version);
    assert.equal(env.result.protocol, 'distinct/1.0');
    assert.equal(env.error, null);
    const u = spawnSync(bin, ['no-such-command'], { encoding: 'utf8' });
    assert.notEqual(u.status, 0);
    assert.match(u.stderr, /unknown command/);
  },
);
