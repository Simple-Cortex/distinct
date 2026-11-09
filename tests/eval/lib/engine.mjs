// Shared helpers for the eval suite: find/run the engine, load fixture sets.
// Node built-ins only.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, isAbsolute, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..', '..');
export const LAUNCHER = join(ROOT, 'bin', 'distinct.mjs');
export const CAL_DIR = join(ROOT, 'tests', 'fixtures', 'calibration');
const EXE = process.platform === 'win32' ? 'distinct-rs.exe' : 'distinct-rs';

/** Same lookup order as tests/runtime/toolchain.test.mjs. */
export function nativeBinary() {
  if (process.env.DISTINCT_RS_BIN) return process.env.DISTINCT_RS_BIN;
  for (const profile of ['debug', 'release']) {
    const p = join(ROOT, 'target', profile, EXE);
    if (existsSync(p)) return p;
  }
  return null;
}

export const SKIP_REASON =
  'SKIPPED: distinct-rs not built (run `cargo build -p distinct-cli` or set DISTINCT_RS_BIN)';

function runOnce(bin, args, opts) {
  const r = spawnSync('node', [LAUNCHER, ...args], {
    encoding: 'utf8',
    env: { ...process.env, DISTINCT_RS_BIN: bin },
    timeout: 60000,
    maxBuffer: 64 * 1024 * 1024,
    ...opts,
  });
  let envelope = null;
  try {
    envelope = JSON.parse(r.stdout);
  } catch {
    /* leave null */
  }
  return { status: r.status, envelope, stderr: r.stderr, stdout: r.stdout };
}

/**
 * Run the launcher. Returns { status, envelope, stderr } (envelope null if stdout is not JSON).
 * Retries ONCE, with a loud log line, only when the launcher reports the distinct
 * `native_probe_timeout` limitation (its protocol probe timed out under load).
 * Everything else, including a signal-killed native (exit 3, no output), is returned
 * as is; callers surface stderr in their assertion messages.
 */
export function distinct(bin, args, opts = {}) {
  let r = runOnce(bin, args, opts);
  if (r.envelope?.limitations?.includes?.('native_probe_timeout')) {
    process.stderr.write(
      `\n!! eval: TRANSIENT native_probe_timeout on "${args.slice(0, 2).join(' ')}"; retrying once. ` +
        `envelope=${JSON.stringify(r.envelope)} stderr=${JSON.stringify(r.stderr)}\n`,
    );
    r = runOnce(bin, args, opts);
  }
  return { status: r.status, envelope: r.envelope, stderr: r.stderr };
}

/** Fresh temp workspace with an initialised brand. Returns { ws, brand, G, run }. */
export function freshWorkspace(bin, brand = 'calibration') {
  const ws = mkdtempSync(join(tmpdir(), 'distinct-eval-ws-'));
  const G = ['--workspace', ws, '--brand', brand];
  const init = distinct(bin, ['init', ...G]);
  if (init.status !== 0) throw new Error(`init failed: ${init.stderr || JSON.stringify(init.envelope)}`);
  return { ws, brand, G, run: (args) => distinct(bin, [...args, ...G]) };
}

/** Read a stored record by the content_hash an envelope reported. */
export function storedRecord(ws, brand, contentHash) {
  return JSON.parse(readFileSync(join(ws, '.distinct', brand, 'store', 'objects', `${contentHash}.json`), 'utf8'));
}

/**
 * Inspect then proof one SVG in the given workspace.
 * Returns { verdict, proof: 'ok'|'blocked'|'error', proofDir }.
 */
export function inspectAndProof(w, svgPath, outDir, profile = null) {
  const ins = w.run(['logo', 'inspect', '--file', svgPath]);
  if (ins.status !== 0 || !ins.envelope?.result) {
    return { verdict: 'error', proof: 'error', detail: JSON.stringify({ stage: 'inspect', exit: ins.status, envelope: ins.envelope, stderr: ins.stderr }) };
  }
  const verdict = ins.envelope.result.verdict;
  if (verdict !== 'inspected') return { verdict, proof: 'blocked' };
  const args = ['logo', 'proof', '--file', svgPath, '--out', outDir];
  if (profile) {
    const pf = join(dirname(outDir), `profile-${Math.abs(hash(outDir))}.json`);
    writeFileSync(pf, JSON.stringify(profile));
    args.push('--profile', pf);
  }
  const pr = w.run(args);
  const proof = pr.status === 0 ? 'ok' : pr.status === 2 ? 'blocked' : 'error';
  const detail = proof === 'ok' ? undefined : JSON.stringify({ exit: pr.status, envelope: pr.envelope, stderr: pr.stderr }, null, 2);
  return { verdict, proof, proofDir: outDir, detail };
}

function hash(s) {
  let h = 0;
  for (const c of s) h = (h * 31 + c.charCodeAt(0)) | 0;
  return h;
}

/**
 * Fixture sets: the committed synthetic set, plus the private set when
 * DISTINCT_EVAL_PRIVATE is set. Private fixture ids are prefixed "private:".
 * Each fixture gets an absolute `path`.
 */
export function loadFixtures() {
  const sets = [{ dir: CAL_DIR, prefix: '', priv: false }];
  const pdir = process.env.DISTINCT_EVAL_PRIVATE;
  if (pdir && existsSync(join(pdir, 'manifest.json'))) sets.push({ dir: resolve(pdir), prefix: 'private:', priv: true });
  const out = [];
  for (const s of sets) {
    const m = JSON.parse(readFileSync(join(s.dir, 'manifest.json'), 'utf8'));
    for (const f of m.fixtures) {
      out.push({
        ...f,
        id: s.prefix + f.id,
        private: s.priv,
        path: isAbsolute(f.file) ? f.file : join(s.dir, f.file),
      });
    }
  }
  return out;
}

/** Merge fixtures into a manifest-shaped object (for score.mjs). */
export function asManifest(fixtures) {
  return { version: 1, fixtures };
}

export function tmp(prefix) {
  return mkdtempSync(join(tmpdir(), prefix));
}
