// WO-15: Hardening tests
// Tests each defect fix by running the binary.
import { test } from 'node:test';
import { strict as assert } from 'node:assert';
import { spawnSync } from 'node:child_process';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync, mkdirSync, writeFileSync, rmSync, readFileSync } from 'node:fs';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..', '..');
const NATIVE = resolve(ROOT, 'target', 'debug', 'distinct-rs');

const SKIP = !existsSync(NATIVE) ? 'distinct-rs not built' : false;

function run(args, opts = {}) {
  const result = spawnSync(NATIVE, args, {
    encoding: 'utf-8',
    cwd: ROOT,
    timeout: 30000,
    ...opts,
  });
  return {
    status: result.status,
    stdout: result.stdout || '',
    stderr: result.stderr || '',
    json: () => {
      try {
        return JSON.parse(result.stdout.trim());
      } catch {
        return null;
      }
    },
  };
}

function setupBrand(brandId) {
  const wsDir = resolve(ROOT, `.distinct-${brandId}-test`);
  rmSync(wsDir, { recursive: true, force: true });
  mkdirSync(wsDir, { recursive: true });
  return wsDir;
}

function initBrand(wsDir, brandId) {
  const r = run(['init', '--workspace', wsDir, '--brand', brandId]);
  assert.equal(r.status, 0, `init failed: ${r.stderr}`);
  return r;
}

function writeRecord(wsDir, brandId, filename, record) {
  const path = resolve(wsDir, filename);
  writeFileSync(path, JSON.stringify(record, null, 2));
  return path;
}

test('WO-15.1: record import rejects governance kinds (exit 3)', { skip: SKIP }, () => {
  const brandId = 'wo15-test1';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Try to import a decision record via record import
  const decisionRecord = {
    schema_version: '1.0.0',
    kind: 'decision',
    id: 'dec-test',
    brand_id: brandId,
    created_at: '2026-10-27T00:00:00Z',
    created_by: { kind: 'owner', id: 'test-owner' },
    privacy: 'internal',
    dependencies: [],
    limitations: [],
    data: { scope: 'test', decision: 'approve' },
  };
  const filePath = writeRecord(wsDir, brandId, 'decision.json', decisionRecord);

  const r = run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath]);
  assert.equal(r.status, 3, `expected exit 3, got ${r.status}`);
  const env = r.json();
  assert.ok(env.error, 'expected error field');
  assert.ok(env.error.includes('governance'), `error should mention governance: ${env.error}`);

  // Try check_result
  const checkResultRecord = { ...decisionRecord, kind: 'check_result', id: 'cr-test' };
  const filePath2 = writeRecord(wsDir, brandId, 'check_result.json', checkResultRecord);
  const r2 = run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath2]);
  assert.equal(r2.status, 3, `expected exit 3 for check_result, got ${r2.status}`);

  // Try review
  const reviewRecord = { ...decisionRecord, kind: 'review', id: 'rv-test' };
  const filePath3 = writeRecord(wsDir, brandId, 'review.json', reviewRecord);
  const r3 = run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath3]);
  assert.equal(r3.status, 3, `expected exit 3 for review, got ${r3.status}`);

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.2: record import updates STATE (verify passes)', { skip: SKIP }, () => {
  const brandId = 'wo15-test2';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Import a content record
  const claimRecord = {
    schema_version: '1.0.0',
    kind: 'claim',
    id: 'claim-test',
    brand_id: brandId,
    created_at: '2026-10-27T00:00:00Z',
    created_by: { kind: 'agent', id: 'distinct-rs' },
    privacy: 'internal',
    dependencies: [],
    limitations: [],
    data: { text: 'Test claim' },
  };
  const filePath = writeRecord(wsDir, brandId, 'claim.json', claimRecord);
  const r = run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath]);
  assert.equal(r.status, 0, `record import failed: ${r.stderr}`);

  // Verify should pass
  const v = run(['verify', '--workspace', wsDir, '--brand', brandId]);
  assert.equal(v.status, 0, `verify failed: ${v.stderr}`);
  const env = v.json();
  assert.equal(env.result.state_valid, true, 'state_valid should be true');

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.3: verify exits 1 when state is invalid', { skip: SKIP }, () => {
  const brandId = 'wo15-test3';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Import a record to create a snapshot
  const claimRecord = {
    schema_version: '1.0.0',
    kind: 'claim',
    id: 'claim-test',
    brand_id: brandId,
    created_at: '2026-10-27T00:00:00Z',
    created_by: { kind: 'agent', id: 'distinct-rs' },
    privacy: 'internal',
    dependencies: [],
    limitations: [],
    data: { text: 'Test claim' },
  };
  const filePath = writeRecord(wsDir, brandId, 'claim.json', claimRecord);
  run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath]);

  // Tamper with STATE.json to make it invalid
  const statePath = resolve(wsDir, '.distinct', brandId, 'STATE.json');
  const state = JSON.parse(readFileSync(statePath, 'utf-8'));
  state.content_snapshot_hash = '0000000000000000000000000000000000000000000000000000000000000000';
  writeFileSync(statePath, JSON.stringify(state, null, 2));

  // Verify should exit 1
  const v = run(['verify', '--workspace', wsDir, '--brand', brandId]);
  assert.equal(v.status, 1, `expected exit 1, got ${v.status}`);
  const env = v.json();
  assert.equal(env.result.state_valid, false, 'state_valid should be false');

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.4: patch --apply with wrong parent_hash exits 3', { skip: SKIP }, () => {
  const brandId = 'wo15-test4';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Import a record first
  const claimRecord = {
    schema_version: '1.0.0',
    kind: 'claim',
    id: 'claim-test',
    brand_id: brandId,
    created_at: '2026-10-27T00:00:00Z',
    created_by: { kind: 'agent', id: 'distinct-rs' },
    privacy: 'internal',
    dependencies: [],
    limitations: [],
    data: { text: 'Original' },
  };
  const filePath = writeRecord(wsDir, brandId, 'claim.json', claimRecord);
  run(['record', 'import', '--workspace', wsDir, '--brand', brandId, '--file', filePath]);

  // Try to replace with wrong parent_hash
  const patchPlan = {
    brand_id: brandId,
    operations: [
      {
        op: 'replace',
        kind: 'claim',
        id: 'claim-test',
        parent_hash: '0000000000000000000000000000000000000000000000000000000000000000',
        data: { text: 'Updated' },
      },
    ],
  };
  const patchPath = writeRecord(wsDir, brandId, 'patch.json', patchPlan);
  const r = run(['patch', '--workspace', wsDir, '--brand', brandId, '--plan', patchPath, '--apply']);
  assert.equal(r.status, 3, `expected exit 3, got ${r.status}`);
  const env = r.json();
  assert.ok(env.error, 'expected error field');
  assert.ok(env.error.includes('conflict') || env.error.includes('parent_hash'), `error should mention conflict: ${env.error}`);

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.5: color check accepts #rrggbb and // comments, exits 3 on zero pairs', { skip: SKIP }, () => {
  const brandId = 'wo15-test5';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Test with #rrggbb format and // comments
  const pairsContent = `// This is a comment
#5b3f8c #f4efe6
// Another comment
c9a227 ffffff
`;
  const pairsPath = resolve(wsDir, 'pairs.txt');
  writeFileSync(pairsPath, pairsContent);

  const r = run(['color', 'check', '--workspace', wsDir, '--brand', brandId, '--pairs', pairsPath]);
  assert.equal(r.status, 0, `color check failed: ${r.stderr}`);
  const env = r.json();
  assert.equal(env.result.pair_count, 2, `expected 2 pairs, got ${env.result.pair_count}`);

  // Test zero pairs (only comments)
  const emptyPairsPath = resolve(wsDir, 'empty-pairs.txt');
  writeFileSync(emptyPairsPath, '// Only comments\n// No pairs\n');
  const r2 = run(['color', 'check', '--workspace', wsDir, '--brand', brandId, '--pairs', emptyPairsPath]);
  assert.equal(r2.status, 3, `expected exit 3 for zero pairs, got ${r2.status}`);

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.6: packet --scope with unknown field exits 3', { skip: SKIP }, () => {
  const brandId = 'wo15-test6';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  const r = run([
    'packet',
    '--workspace', wsDir,
    '--brand', brandId,
    '--task', 'test',
    '--scope', 'platform_fields,unknown_field',
  ]);
  assert.equal(r.status, 3, `expected exit 3, got ${r.status}`);
  const env = r.json();
  assert.ok(env.error, 'expected error field');
  assert.ok(env.error.includes('unknown scope fields'), `error should mention unknown scope: ${env.error}`);

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.7: test import with nonexistent frozen_plan_ref exits 3', { skip: SKIP }, () => {
  const brandId = 'wo15-test7';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  const testResult = {
    id: 'tr-test',
    data: {
      frozen_plan_ref: 'nonexistent-plan',
      execution: 'completed',
      observations: [{ stimulus: 'test', participant: 'P1', note: 'test' }],
      interpretation: 'mixed',
      limitations: [],
    },
  };
  const filePath = writeRecord(wsDir, brandId, 'result.json', testResult);

  const r = run(['test', 'import', '--workspace', wsDir, '--brand', brandId, '--results', filePath]);
  assert.equal(r.status, 3, `expected exit 3, got ${r.status}`);
  const env = r.json();
  assert.ok(env.error, 'expected error field');
  assert.ok(env.error.includes('frozen_plan_ref'), `error should mention frozen_plan_ref: ${env.error}`);

  rmSync(wsDir, { recursive: true, force: true });
});

test('WO-15.8: decision record without data.target exits 3', { skip: SKIP }, () => {
  const brandId = 'wo15-test8';
  const wsDir = setupBrand(brandId);
  initBrand(wsDir, brandId);

  // Decision without data.target
  const decisionRecord = {
    schema_version: '1.0.0',
    kind: 'decision',
    id: 'dec-test',
    brand_id: brandId,
    created_at: '2026-10-27T00:00:00Z',
    created_by: { kind: 'owner', id: 'test-owner' },
    privacy: 'internal',
    dependencies: [],
    limitations: [],
    data: { scope: 'test', decision: 'approve' },
  };
  const filePath = writeRecord(wsDir, brandId, 'decision.json', decisionRecord);

  const r = run([
    'decision', 'record',
    '--workspace', wsDir,
    '--brand', brandId,
    '--file', filePath,
    '--owner', 'test-owner',
  ]);
  assert.equal(r.status, 3, `expected exit 3, got ${r.status}`);
  const env = r.json();
  assert.ok(env.error, 'expected error field');
  assert.ok(env.error.includes('data.target'), `error should mention data.target: ${env.error}`);

  rmSync(wsDir, { recursive: true, force: true });
});
