// Unit test for critic/score.mjs with a synthetic key, manifest and critic output.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { score } from './critic/score.mjs';

const manifest = {
  version: 1,
  measured_items: ['r1', 'engine_safe'],
  fixtures: [
    { id: 'FA', role: 'flawed', flaws: [{ type: 'tA', rubric_item: 'r1', detectable: 'measured' }] },
    { id: 'FB', role: 'flawed', flaws: [{ type: 'tB', rubric_item: 'r2', detectable: 'judged' }] },
    { id: 'CA', role: 'control', flaws: [] },
    { id: 'CB', role: 'control', flaws: [] },
    { id: 'REF', role: 'reference', flaws: [] },
  ],
};
const key = { 'K-01': 'FA', 'K-02': 'FB', 'K-03': 'CA', 'K-04': 'CB', 'E-01': 'REF' };
const cand = (label, verdict, findings = []) => ({ label, verdict, findings });
const fnd = (item, severity, cls = 'JUDGED') => ({ item, severity, class: cls, text: 't' });

const critic = {
  candidates: [
    cand('K-01', 'rework', [fnd('r1', 'major', 'MEASURED')]), // caught, valid MEASURED claim
    cand('K-02', 'present', [fnd('x', 'major', 'MEASURED')]), // missed, self-flagged, invalid MEASURED claim
    cand('K-03', 'present'),
    cand('K-04', 'rework', [fnd('y', 'minor')]), // false flag via verdict
    cand('E-01', 'drop', [fnd('z', 'blocker')]), // reference label: ignored
  ],
};
const reverse = {
  candidates: [cand('K-01', 'rework'), cand('K-02', 'rework'), cand('K-03', 'present'), cand('K-04', 'rework')],
};

test('score: every metric on a synthetic run', () => {
  const r = score({ key, critic, manifest, reverseCritic: reverse });
  assert.equal(r.flaw_recall.overall, 0.5);
  assert.deepEqual(r.flaw_recall.per_type, {
    tA: { caught: 1, total: 1, recall: 1 },
    tB: { caught: 0, total: 1, recall: 0 },
  });
  assert.equal(r.control_false_flag_rate, 0.5);
  assert.deepEqual(r.self_flagged_but_presented, { count: 1, labels: ['K-02'] });
  assert.equal(r.verdict_accuracy, 0.5);
  assert.equal(r.order_consistency, 0.75);
  assert.deepEqual(r.measured_claim_violations, [{ label: 'K-02', item: 'x' }]);
  assert.deepEqual(r.missing_labels, []);
});

test('score: no reverse run gives order_consistency null', () => {
  assert.equal(score({ key, critic, manifest }).order_consistency, null);
});

test('score: verdict rework with a major finding catches a flaw without item match', () => {
  const c = { candidates: [cand('K-02', 'drop', [fnd('other', 'major')])] };
  const r = score({ key, critic: c, manifest });
  assert.equal(r.flaw_recall.per_type.tB.caught, 1);
  // K-01, K-03, K-04 missing: flaw tA uncaught, reported as missing
  assert.deepEqual(r.missing_labels, ['K-01', 'K-03', 'K-04']);
  assert.equal(r.flaw_recall.per_type.tA.caught, 0);
});

test('score: minor-only findings on a present flawed fixture do not count as a catch', () => {
  const c = { candidates: [cand('K-02', 'present', [fnd('other', 'minor')])] };
  assert.equal(score({ key, critic: c, manifest }).flaw_recall.per_type.tB.caught, 0);
});

test('score: no controls in the key gives a null false-flag rate', () => {
  const r = score({ key: { 'K-01': 'FA' }, critic: { candidates: [cand('K-01', 'present')] }, manifest });
  assert.equal(r.control_false_flag_rate, null);
});

test('score: unknown label is an error', () => {
  assert.throws(() => score({ key, critic: { candidates: [cand('K-99', 'present')] }, manifest }), /unknown label/);
  assert.throws(
    () => score({ key, critic, manifest, reverseCritic: { candidates: [cand('K-77', 'present')] } }),
    /unknown label/,
  );
});

test('score: invalid verdict, duplicate label, bad shape and unknown fixture are errors', () => {
  assert.throws(() => score({ key, critic: { candidates: [cand('K-01', 'maybe')] }, manifest }), /invalid verdict/);
  assert.throws(
    () => score({ key, critic: { candidates: [cand('K-01', 'present'), cand('K-01', 'drop')] }, manifest }),
    /duplicate/,
  );
  assert.throws(() => score({ key, critic: {}, manifest }), /candidates/);
  assert.throws(() => score({ key: { 'K-01': 'NOPE' }, critic: { candidates: [] }, manifest }), /not in the manifest/);
});

test('score: a control missing from the critic output is a false flag and an accuracy miss', () => {
  const c = { candidates: [cand('K-03', 'present')] }; // K-04 (control) never reviewed
  const r = score({ key: { 'K-03': 'CA', 'K-04': 'CB' }, critic: c, manifest });
  assert.equal(r.control_false_flag_rate, 0.5);
  assert.equal(r.verdict_accuracy, 0.5);
  assert.deepEqual(r.missing_labels, ['K-04']);
});

test('score: MEASURED is allowed on engine-measured items regardless of fixture, else a violation', () => {
  const c = {
    candidates: [
      cand('K-03', 'present', [fnd('engine_safe', 'minor', 'MEASURED')]), // control, measurable item: fine
      cand('K-02', 'rework', [fnd('r2', 'major', 'MEASURED')]), // r2 is not measurable: violation
    ],
  };
  const r = score({ key: { 'K-03': 'CA', 'K-02': 'FB' }, critic: c, manifest });
  assert.deepEqual(r.measured_claim_violations, [{ label: 'K-02', item: 'r2' }]);
});
