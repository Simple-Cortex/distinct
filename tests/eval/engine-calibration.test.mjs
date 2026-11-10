// Layer 1: deterministic engine calibration against the synthetic fixtures.
// Skips (same pattern as toolchain.test.mjs) when distinct-rs is not built.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { generate, writeTo } from '../fixtures/calibration/generate.mjs';
import {
  CAL_DIR, SKIP_REASON, freshWorkspace, inspectAndProof, loadFixtures, nativeBinary, storedRecord, tmp,
} from './lib/engine.mjs';

const bin = nativeBinary();
const skip = bin ? false : SKIP_REASON;

test('fixtures: regenerating gives byte-identical output (no drift)', () => {
  const dir = tmp('distinct-cal-gen-');
  writeTo(dir);
  const { files } = generate();
  for (const [rel, content] of files) {
    assert.equal(readFileSync(join(dir, rel), 'utf8'), content, `${rel}: regenerate not stable`);
    assert.equal(readFileSync(join(CAL_DIR, rel), 'utf8'), content, `${rel}: committed file drifted from generate.mjs`);
  }
  const committed = readdirSync(join(CAL_DIR, 'svg')).map((f) => `svg/${f}`).sort();
  const generated = [...files.keys()].filter((k) => k.startsWith('svg/')).sort();
  assert.deepEqual(committed, generated, 'committed svg/ has files the generator does not produce');
});

test('fixtures: manifest is well formed and not shipped in the runtime manifest', () => {
  const m = JSON.parse(readFileSync(join(CAL_DIR, 'manifest.json'), 'utf8'));
  assert.equal(m.version, 1);
  // Rubric ids from references/critique.md (shipped in Simple-Cortex/distinct#5). Keep in sync.
  const RUBRIC = ['appropriate', 'simple', 'distinctiveness', 'small_size_legibility', 'negative_space_hazard',
    'balance', 'one_colour', 'reversed', 'colour_contrast', 'silhouette', 'construction_quality',
    'engine_safe', 'adaptable', 'durability', 'memorable'];
  for (const it of m.measured_items) assert.ok(RUBRIC.includes(it), `measured_items: ${it} not a rubric id`);
  const ids = new Set();
  for (const f of m.fixtures) {
    assert.ok(!ids.has(f.id), `duplicate id ${f.id}`);
    ids.add(f.id);
    assert.ok(['control', 'flawed', 'reference'].includes(f.role), `${f.id}: bad role`);
    assert.ok(existsSync(join(CAL_DIR, f.file)), `${f.id}: missing file`);
    assert.equal((f.flaws.length > 0), f.role === 'flawed', `${f.id}: flaws only on flawed fixtures`);
    for (const fl of f.flaws) assert.ok(RUBRIC.includes(fl.rubric_item), `${f.id}: ${fl.rubric_item} not a rubric id`);
    for (const fl of f.flaws) assert.ok(['measured', 'judged', 'observed'].includes(fl.detectable), `${f.id}: bad detectable`);
    const svg = readFileSync(join(CAL_DIR, f.file), 'utf8');
    assert.ok(svg.includes(`<desc>${f.id}</desc>`), `${f.id}: desc must carry the id`);
    assert.ok(!/flaw|bloat|collision|vanish|hazard/i.test(svg), `${f.id}: svg must not describe its flaw`);
  }
  const rm = readFileSync(join(CAL_DIR, '..', '..', '..', 'runtime-manifest.json'), 'utf8');
  assert.ok(!rm.includes('calibration'), 'fixtures must not ship with the skill');
});

test('engine expectations per fixture', { skip }, (t) => {
  const fixtures = loadFixtures();
  for (const f of fixtures) {
    const w = freshWorkspace(bin);
    const label = f.private ? '(private fixture)' : f.id;
    const out = join(w.ws, `proof-${fixtures.indexOf(f)}`);
    const r = inspectAndProof(w, f.path, out);
    if (f.expect_engine) {
      assert.equal(r.verdict, f.expect_engine.inspect_verdict, `${label}: inspect verdict; engine said: ${r.detail ?? '(n/a)'}`);
      assert.equal(r.proof, f.expect_engine.proof, `${label}: proof outcome; engine said: ${r.detail ?? '(n/a)'}`);
    }
    if (f.colours) {
      const pairs = join(w.ws, `pairs-${fixtures.indexOf(f)}.txt`);
      writeFileSync(pairs, f.colours.map((c) => `${c.fg} ${c.bg}`).join('\n') + '\n');
      const cc = w.run(['color', 'check', '--pairs', pairs]);
      assert.equal(cc.status, 0, `${label}: color check exit`);
      const rec = storedRecord(w.ws, w.brand, cc.envelope.result.content_hash);
      assert.equal(rec.data.results.length, f.colours.length, `${label}: pair count`);
      f.colours.forEach((c, i) => {
        const lc = rec.data.results[i].apca_lc;
        assert.ok(
          lc >= c.expect_lc_min && lc <= c.expect_lc_max,
          `${label}: pair ${c.fg} on ${c.bg}: Lc ${lc} outside [${c.expect_lc_min}, ${c.expect_lc_max}]`,
        );
      });
    }
  }
  t.diagnostic(`checked ${fixtures.length} fixtures`);
});

test('coverage table: which flaw types the engine measures today', () => {
  const fixtures = loadFixtures().filter((f) => !f.private);
  const types = new Map();
  for (const f of fixtures) {
    for (const fl of f.flaws) {
      const t = types.get(fl.type) ?? { measured: false, future: new Set(), ids: [] };
      if (fl.detectable === 'measured') t.measured = true;
      if (fl.future) t.future.add(fl.future);
      t.ids.push(f.id);
      types.set(fl.type, t);
    }
  }
  const rows = [...types].sort(([a], [b]) => a.localeCompare(b));
  const w = Math.max(...rows.map(([k]) => k.length));
  const lines = ['', 'Engine coverage of planted flaw types', `${'flaw type'.padEnd(w)}  now       future        fixtures`];
  for (const [k, v] of rows) {
    lines.push(`${k.padEnd(w)}  ${(v.measured ? 'measured' : 'judged').padEnd(8)}  ${([...v.future].join(',') || '-').padEnd(12)}  ${v.ids.join(',')}`);
  }
  const measured = rows.filter(([, v]) => v.measured).length;
  lines.push(`measured: ${measured}/${rows.length} flaw types`, '');
  console.log(lines.join('\n'));
  assert.ok(rows.length > 0);
});
