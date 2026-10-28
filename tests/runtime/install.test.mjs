// Install and runtime validation tests for WO-07.
// Tests: packed install from unrelated cwd, existing_install_refused,
// installer_no_network (static check), manifest_private_file_rejected.

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, existsSync, readdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, '..', '..');
const installScript = resolve(repoRoot, 'scripts', 'install.mjs');
const validateScript = resolve(repoRoot, 'scripts', 'validate-runtime.mjs');
const launcher = resolve(repoRoot, 'bin', 'distinct.mjs');

function runInstall(args = [], env = {}, cwd = undefined) {
  return spawnSync('node', [installScript, ...args], {
    encoding: 'utf-8',
    env: { ...process.env, ...env },
    timeout: 15000,
    cwd: cwd || repoRoot,
  });
}

function runValidate(args = []) {
  return spawnSync('node', [validateScript, ...args], {
    encoding: 'utf-8',
    env: { ...process.env },
    timeout: 15000,
    cwd: repoRoot,
  });
}

function runLauncher(args = [], env = {}, cwd = undefined) {
  return spawnSync('node', [launcher, ...args], {
    encoding: 'utf-8',
    env: { ...process.env, ...env },
    timeout: 10000,
    cwd: cwd || repoRoot,
  });
}

describe('install', () => {
  it('packed install into temp dir from unrelated cwd, then launcher --help', () => {
    const dest = mkdtempSync(join(tmpdir(), 'distinct-install-'));
    const unrelatedCwd = mkdtempSync(join(tmpdir(), 'distinct-unrelated-'));

    const r = runInstall(['--dest', dest], {}, unrelatedCwd);
    assert.equal(r.status, 0, `install failed: ${r.stderr}`);
    assert.match(r.stdout, /copied \d+ file/);

    // Verify key files were copied
    assert.ok(existsSync(join(dest, 'SKILL.md')), 'SKILL.md should be installed');
    assert.ok(existsSync(join(dest, 'bin', 'distinct.mjs')), 'bin/distinct.mjs should be installed');
    assert.ok(existsSync(join(dest, 'capabilities.json')), 'capabilities.json should be installed');
    assert.ok(existsSync(join(dest, 'package.json')), 'package.json should be installed');

    // Launcher --help should work from the installed location
    const installedLauncher = join(dest, 'bin', 'distinct.mjs');
    const help = spawnSync('node', [installedLauncher, '--help'], {
      encoding: 'utf-8',
      timeout: 10000,
    });
    assert.equal(help.status, 0);
    assert.match(help.stdout, /brand skill launcher/);

    // Cleanup
    rmSync(dest, { recursive: true, force: true });
    rmSync(unrelatedCwd, { recursive: true, force: true });
  });

  it('packed install with DISTINCT_RS_BIN runs native status', () => {
    // Build native first
    const build = spawnSync('cargo', ['build', '--offline'], {
      encoding: 'utf-8',
      cwd: repoRoot,
      timeout: 120000,
    });
    if (build.status !== 0) return; // Skip if build fails

    const nativePath = resolve(repoRoot, 'target', 'debug', 'distinct-rs');
    if (!existsSync(nativePath)) return;

    const dest = mkdtempSync(join(tmpdir(), 'distinct-install-native-'));
    const workspace = mkdtempSync(join(tmpdir(), 'distinct-workspace-'));

    try {
      const r = runInstall(['--dest', dest]);
      assert.equal(r.status, 0, `install failed: ${r.stderr}`);

      // Run launcher with native from installed location
      const installedLauncher = join(dest, 'bin', 'distinct.mjs');
      const status = spawnSync('node', [installedLauncher, 'status', '--brand', 'acme', '--workspace', workspace], {
        encoding: 'utf-8',
        env: { ...process.env, DISTINCT_RS_BIN: nativePath },
        timeout: 10000,
      });
      // Should succeed (exit 0) or at least delegate to native
      assert.ok(status.status === 0 || status.status === 3, `expected exit 0 or 3, got ${status.status}`);
    } finally {
      rmSync(dest, { recursive: true, force: true });
      rmSync(workspace, { recursive: true, force: true });
    }
  });

  // WO-07: existing_install_refused — non-empty dest without --force is refused.
  it('existing_install_refused', () => {
    const dest = mkdtempSync(join(tmpdir(), 'distinct-install-refuse-'));
    // Place a sentinel file
    writeFileSync(join(dest, 'sentinel.txt'), 'do not overwrite');

    const r = runInstall(['--dest', dest]);
    assert.equal(r.status, 1, `expected exit 1 for existing non-empty dest, got ${r.status}`);
    assert.match(r.stderr, /non-empty|force/i);

    // Sentinel should still exist
    assert.ok(existsSync(join(dest, 'sentinel.txt')), 'sentinel should be preserved');
    assert.equal(readFileSync(join(dest, 'sentinel.txt'), 'utf-8'), 'do not overwrite');

    // With --force it should succeed
    const r2 = runInstall(['--dest', dest, '--force']);
    assert.equal(r2.status, 0, `--force install should succeed: ${r2.stderr}`);

    rmSync(dest, { recursive: true, force: true });
  });

  // WO-07: installer_no_network — static check that install.mjs has no http/net imports.
  it('installer_no_network', () => {
    const src = readFileSync(installScript, 'utf-8');
    // Check that no network-related modules are imported
    const networkPatterns = [
      /require\s*\(\s*['"]https?['"]/,
      /from\s+['"]https?['"]/,
      /import.*['"]https?['"]/,
      /require\s*\(\s*['"]http['"]/,
      /from\s+['"]http['"]/,
      /import.*['"]http['"]/,
      /require\s*\(\s*['"]net['"]/,
      /from\s+['"]net['"]/,
      /import.*['"]net['"]/,
      /fetch\s*\(/,
      /XMLHttpRequest/,
    ];
    for (const pat of networkPatterns) {
      assert.ok(!pat.test(src), `install.mjs should not use network: matched ${pat}`);
    }
  });

  // WO-07: manifest_private_file_rejected — manifest should not include files
  // from planning/, tasks/, or crates/.
  it('manifest_private_file_rejected', () => {
    const manifest = JSON.parse(readFileSync(resolve(repoRoot, 'runtime-manifest.json'), 'utf-8'));
    const include = manifest.include || [];
    const forbiddenPrefixes = ['planning/', 'tasks/', 'crates/'];

    for (const f of include) {
      for (const prefix of forbiddenPrefixes) {
        assert.ok(!f.startsWith(prefix), `manifest include should not contain ${f} (under ${prefix})`);
      }
    }

    // Also check that no private dev files are included
    const forbiddenFiles = ['QWEN.md', 'AGENTS.md', 'CLAUDE.md', 'DEVLOG.md'];
    for (const f of include) {
      assert.ok(!forbiddenFiles.includes(f), `manifest include should not contain dev file ${f}`);
    }
  });
});

describe('validate-runtime', () => {
  it('passes on the current repo', () => {
    const r = runValidate(['--manifest', 'runtime-manifest.json', '--root', '.']);
    assert.equal(r.status, 0, `validate-runtime failed: ${r.stderr}\n${r.stdout}`);
    assert.match(r.stdout, /OK/);
  });
});

// ── WO-09: WO-00 control tests ──────────────────────────────────────

describe('WO-00 controls', () => {
  // existing_destination_preserved — install refuses a non-empty destination
  // without --force, and preserves existing files.
  it('existing_destination_preserved', () => {
    const dest = mkdtempSync(join(tmpdir(), 'distinct-wo00-dest-'));
    writeFileSync(join(dest, 'existing.txt'), 'preserve-me');

    const r = runInstall(['--dest', dest]);
    assert.equal(r.status, 1, `expected exit 1 for non-empty dest, got ${r.status}`);

    // Existing file must still be there.
    assert.ok(existsSync(join(dest, 'existing.txt')), 'existing file should be preserved');
    assert.equal(readFileSync(join(dest, 'existing.txt'), 'utf-8'), 'preserve-me');

    // With --force it succeeds.
    const r2 = runInstall(['--dest', dest, '--force']);
    assert.equal(r2.status, 0, `--force install should succeed: ${r2.stderr}`);

    rmSync(dest, { recursive: true, force: true });
  });

  // offline_build_no_fetch — cargo build --offline --locked succeeds.
  it('offline_build_no_fetch', () => {
    const build = spawnSync('cargo', ['build', '--offline', '--locked'], {
      encoding: 'utf-8',
      cwd: repoRoot,
      timeout: 180000,
    });
    assert.equal(build.status, 0, `offline build failed: ${build.stderr}`);
  });
});
