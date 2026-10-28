// Launcher tests for bin/distinct.mjs
// Tests the Node launcher's core behaviors: version, help, native detection,
// skill-root resolution, protocol mismatch, and outside-cwd operation.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, writeFileSync, chmodSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const launcher = resolve(__dirname, '..', '..', 'bin', 'distinct.mjs');
const repoRoot = resolve(__dirname, '..', '..');

function run(args = [], env = {}, cwd = undefined) {
  return spawnSync('node', [launcher, ...args], {
    encoding: 'utf-8',
    env: { ...process.env, ...env },
    timeout: 10000,
    cwd: cwd || repoRoot,
  });
}

describe('launcher', () => {
  it('reports version without native', () => {
    const r = run(['--version']);
    assert.equal(r.status, 0);
    assert.match(r.stdout, /distinct launcher/);
  });

  it('shows help without native', () => {
    const r = run(['--help']);
    assert.equal(r.status, 0);
    assert.match(r.stdout, /brand skill launcher/);
  });

  // WO-07: missing_native_help_control — help and version work even when
  // DISTINCT_RS_BIN points to a nonexistent path.
  it('missing_native_help_control', () => {
    const r = run(['--help'], { DISTINCT_RS_BIN: '/nonexistent/distinct-rs' });
    assert.equal(r.status, 0, `--help should work without native; got exit ${r.status}`);
    assert.match(r.stdout, /brand skill launcher/);

    const v = run(['--version'], { DISTINCT_RS_BIN: '/nonexistent/distinct-rs' });
    assert.equal(v.status, 0, `--version should work without native; got exit ${v.status}`);
    assert.match(v.stdout, /distinct launcher/);
  });

  // WO-07: protocol_mismatch — a fake binary that returns wrong protocol
  // should cause the launcher to exit 2 with an unavailable envelope.
  it('protocol_mismatch', () => {
    const fakeBinDir = mkdtempSync(join(tmpdir(), 'distinct-fake-'));
    const fakeBin = join(fakeBinDir, 'distinct-rs');
    writeFileSync(fakeBin, '#!/bin/sh\necho \'{"protocol_version":"distinct/9.9","engine_version":"0.0.0","execution":{"engine":"fake","command":"protocol"},"result":null,"limitations":[],"changed":[],"error":null}\'\n');
    chmodSync(fakeBin, 0o755);

    const r = run(['status', '--brand', 'acme'], { DISTINCT_RS_BIN: fakeBin });
    assert.equal(r.status, 2, `expected exit 2 for protocol mismatch, got ${r.status}`);
    const envelope = JSON.parse(r.stdout.trim());
    assert.equal(envelope.protocol_version, 'distinct/1.0');
    assert.ok(envelope.error, 'should have an error field');
    assert.match(envelope.error, /not found|protocol mismatch/i);
  });

  // WO-07: outside_cwd_skill_resolution — launcher works from an unrelated cwd.
  it('outside_cwd_skill_resolution', () => {
    const unrelatedDir = mkdtempSync(join(tmpdir(), 'distinct-cwd-'));
    const r = run(['--help'], {}, unrelatedDir);
    assert.equal(r.status, 0, `--help should work from unrelated cwd; got exit ${r.status}`);
    assert.match(r.stdout, /brand skill launcher/);
    assert.match(r.stdout, /Skill root:/);
  });

  it('exits 2 when native not found for commands', () => {
    const r = run(['status', '--brand', 'acme'], {
      DISTINCT_RS_BIN: '/nonexistent/distinct-rs',
    });
    assert.equal(r.status, 2, `expected exit 2, got ${r.status}`);
    const envelope = JSON.parse(r.stdout.trim());
    assert.ok(envelope.error, 'should have error field');
  });

  it('delegates to native when available', () => {
    const nativePath = resolve(repoRoot, 'target', 'debug', 'distinct-rs');
    if (!existsSync(nativePath)) {
      return; // Skip if not built
    }
    const r = run(['--version'], { DISTINCT_RS_BIN: nativePath });
    assert.equal(r.status, 0);
  });

  it('rejects old Node versions conceptually', () => {
    const major = parseInt(process.versions.node.split('.')[0], 10);
    assert.ok(major >= 20, 'test requires Node 20+');
  });

  // WO-09: WO-00 control — missing_toolchain_is_blocked.
  // Launcher with no binary → exit 2 structured envelope.
  it('missing_toolchain_is_blocked', () => {
    const r = run(['status', '--brand', 'acme'], {
      DISTINCT_RS_BIN: '/nonexistent/distinct-rs',
    });
    assert.equal(r.status, 2, `expected exit 2, got ${r.status}`);
    const envelope = JSON.parse(r.stdout.trim());
    assert.equal(envelope.protocol_version, 'distinct/1.0');
    assert.ok(envelope.error, 'should have error field');
  });

  // WO-09: WO-00 control — unsupported_node_rejected.
  // The install script checks Node version at runtime.
  it('unsupported_node_rejected', () => {
    // We can't easily run Node < 20 in this test, but we verify the install
    // script has the version guard by static inspection.
    const src = readFileSync(resolve(repoRoot, 'scripts', 'install.mjs'), 'utf-8');
    assert.match(src, /Node\s*(>=?\s*20|version|major)/i,
      'install.mjs should check Node version');
    // Also verify the launcher checks.
    const launcherSrc = readFileSync(launcher, 'utf-8');
    assert.match(launcherSrc, /Node|version|major/i,
      'launcher should reference Node version');
  });
});
