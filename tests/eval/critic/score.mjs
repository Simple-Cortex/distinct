#!/usr/bin/env node
// Layer 2: score a critic run against the key and the fixture manifest. Pure, no I/O in score().
// Usage: node score.mjs --key key.json --critic critic.json [--reverse-critic critic-rev.json] [--manifest path]
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SEV = { minor: 1, major: 2, blocker: 3 };
const VERDICTS = new Set(['present', 'rework', 'drop']);
const major = (f) => (SEV[f.severity] ?? 0) >= SEV.major;
const ratio = (a, b) => (b === 0 ? null : Math.round((a / b) * 1000) / 1000);

function indexCritic(critic, key, label) {
  if (!critic || !Array.isArray(critic.candidates)) throw new Error(`${label}: "candidates" array missing`);
  const map = new Map();
  for (const c of critic.candidates) {
    if (!(c.label in key)) throw new Error(`${label}: unknown label ${JSON.stringify(c.label)} (not in key)`);
    if (!VERDICTS.has(c.verdict)) throw new Error(`${label}: ${c.label} has invalid verdict ${JSON.stringify(c.verdict)}`);
    if (map.has(c.label)) throw new Error(`${label}: duplicate label ${c.label}`);
    map.set(c.label, { ...c, findings: Array.isArray(c.findings) ? c.findings : [] });
  }
  return map;
}

export function score({ key, critic, manifest, reverseCritic = null }) {
  const byId = new Map(manifest.fixtures.map((f) => [f.id, f]));
  for (const [label, id] of Object.entries(key)) {
    if (!byId.has(id)) throw new Error(`key label ${label} maps to ${id}, which is not in the manifest`);
  }
  const run = indexCritic(critic, key, 'critic');
  const rev = reverseCritic ? indexCritic(reverseCritic, key, 'reverse critic') : null;

  // Only controls and flawed fixtures are scored; reference ("existing marks") labels are ignored.
  const scored = Object.entries(key)
    .map(([label, id]) => ({ label, fx: byId.get(id) }))
    .filter((x) => x.fx.role === 'control' || x.fx.role === 'flawed');

  const measurable = new Set(manifest.measured_items ?? []);
  let flawTotal = 0;
  let flawCaught = 0;
  const perType = {};
  let controls = 0;
  let controlsFlagged = 0;
  let accTotal = 0;
  let accRight = 0;
  const selfFlagged = [];
  const violations = [];
  const missing = [];

  for (const { label, fx } of scored) {
    const c = run.get(label);
    if (!c) {
      missing.push(label);
      if (fx.role === 'flawed') {
        for (const fl of fx.flaws) {
          const t = (perType[fl.type] ??= { caught: 0, total: 0, recall: null });
          t.total++;
          flawTotal++;
        }
        accTotal++;
      } else {
        // A control the critic never reviewed counts as a false flag and an accuracy miss.
        controls++;
        controlsFlagged++;
        accTotal++;
      }
      continue;
    }
    const hasMajor = c.findings.some(major);
    const rejects = c.verdict === 'rework' || c.verdict === 'drop';

    if (fx.role === 'flawed') {
      for (const fl of fx.flaws) {
        const caught = c.findings.some((f) => f.item === fl.rubric_item) || (rejects && hasMajor);
        const t = (perType[fl.type] ??= { caught: 0, total: 0, recall: null });
        t.total++;
        flawTotal++;
        if (caught) {
          t.caught++;
          flawCaught++;
        }
      }
      accTotal++;
      if (rejects) accRight++;
    } else {
      controls++;
      if (hasMajor || c.verdict !== 'present') controlsFlagged++;
      accTotal++;
      if (c.verdict === 'present') accRight++;
    }

    if (hasMajor && c.verdict === 'present') selfFlagged.push(label);

    for (const f of c.findings) {
      if (f.class !== 'MEASURED') continue;
      // MEASURED is legitimate only for items the engine measures (manifest.measured_items).
      if (!measurable.has(f.item)) violations.push({ label, item: f.item });
    }
  }
  for (const t of Object.values(perType)) t.recall = ratio(t.caught, t.total);

  let order = null;
  if (rev) {
    let same = 0;
    let both = 0;
    for (const { label } of scored) {
      const a = run.get(label);
      const b = rev.get(label);
      if (!a || !b) continue;
      both++;
      if (a.verdict === b.verdict) same++;
    }
    order = ratio(same, both);
  }

  return {
    flaw_recall: { overall: ratio(flawCaught, flawTotal), caught: flawCaught, total: flawTotal, per_type: perType },
    control_false_flag_rate: ratio(controlsFlagged, controls),
    self_flagged_but_presented: { count: selfFlagged.length, labels: selfFlagged },
    verdict_accuracy: ratio(accRight, accTotal),
    order_consistency: order,
    measured_claim_violations: violations,
    missing_labels: missing,
  };
}

function arg(argv, name) {
  const i = argv.indexOf(name);
  return i >= 0 ? argv[i + 1] : null;
}
const readJson = (p) => JSON.parse(readFileSync(p, 'utf8'));

const self = fileURLToPath(import.meta.url);
if (process.argv[1] && resolve(process.argv[1]) === self) {
  try {
    const argv = process.argv.slice(2);
    const keyPath = arg(argv, '--key');
    const criticPath = arg(argv, '--critic');
    if (!keyPath || !criticPath) throw new Error('--key and --critic are required');
    const root = resolve(dirname(self), '..', '..', '..');
    const manifestPath = arg(argv, '--manifest') ?? join(root, 'tests', 'fixtures', 'calibration', 'manifest.json');
    const manifest = readJson(manifestPath);
    // Private set (local only): merged in when DISTINCT_EVAL_PRIVATE is set.
    if (process.env.DISTINCT_EVAL_PRIVATE && !arg(argv, '--manifest')) {
      const pm = readJson(join(process.env.DISTINCT_EVAL_PRIVATE, 'manifest.json'));
      for (const f of pm.fixtures) manifest.fixtures.push({ ...f, id: `private:${f.id}` });
    }
    const revPath = arg(argv, '--reverse-critic');
    const report = score({
      key: readJson(keyPath),
      critic: readJson(criticPath),
      manifest,
      reverseCritic: revPath ? readJson(revPath) : null,
    });
    process.stdout.write(JSON.stringify(report, null, 2) + '\n');
  } catch (e) {
    process.stderr.write(`score: ${e.message}\n`);
    process.exit(2);
  }
}
