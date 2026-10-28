#!/usr/bin/env node
// Distinct Node launcher (contract section 2/5).
//
// Locates the native distinct-rs binary via DISTINCT_RS_BIN env var or PATH,
// probes protocol distinct/1.0, and delegates all commands.
// No policy duplication — the native binary is the authority.
//
// Requirements: Node 20+, standard modules only.

import { spawnSync } from 'node:child_process';
import { existsSync, readFileSync, appendFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const PROTOCOL_VERSION = 'distinct/1.0';
const MIN_NODE_MAJOR = 20;

// Resolve skill root: the folder containing the root SKILL.md.
// This is the directory of this script (bin/distinct.mjs -> parent).
const __dirname = dirname(fileURLToPath(import.meta.url));
const SKILL_ROOT = resolve(__dirname, '..');

function findNative() {
  // 1. Explicit env override
  if (process.env.DISTINCT_RS_BIN) {
    const p = resolve(process.env.DISTINCT_RS_BIN);
    if (existsSync(p)) return p;
    return null;
  }

  // 2. On PATH
  const probe = spawnSync('which', ['distinct-rs'], { encoding: 'utf-8', timeout: 2000 });
  if (probe.status === 0 && probe.stdout.trim()) {
    return probe.stdout.trim();
  }

  return null;
}

function probeProtocol(binary) {
  const result = spawnSync(binary, ['protocol'], {
    encoding: 'utf-8',
    timeout: 5000,
  });
  if (result.status !== 0) return null;
  try {
    const envelope = JSON.parse(result.stdout.trim());
    if (envelope.protocol_version === PROTOCOL_VERSION) return envelope;
    return null;
  } catch {
    return null;
  }
}

function checkNodeVersion() {
  const major = parseInt(process.versions.node.split('.')[0], 10);
  if (major < MIN_NODE_MAJOR) {
    console.error(
      `distinct: Node ${major} is below minimum required v${MIN_NODE_MAJOR}`
    );
    process.exit(2);
  }
}

// ---- Opt-in tracing (DISTINCT_TRACE=<file>) ------------------------------
// Records one JSONL line per launcher call: metadata only, never stdout
// content, flag values or file contents. Failures never alter exit/output.

let traceWarned = false;
function traceWarn(msg) {
  if (traceWarned) return;
  traceWarned = true;
  try { process.stderr.write(`distinct: trace disabled: ${msg}\n`); } catch { /* ignore */ }
}

function nativeCommandNames() {
  try {
    const caps = JSON.parse(readFileSync(resolve(SKILL_ROOT, 'capabilities.json'), 'utf-8'));
    return Object.keys(caps.native || {});
  } catch {
    return [];
  }
}

// Subcommand words in space form, e.g. "logo proof", "ingest import".
// Hyphenated aliases (record-import) are normalised; positional values are dropped.
function commandWords(args) {
  const positional = [];
  for (const a of args) {
    if (a.startsWith('-')) break;
    positional.push(a);
  }
  if (positional.length === 0) return 'unknown';
  const names = nativeCommandNames();
  const norm = (t) => t.replace(/-/g, ' ');
  for (let n = Math.min(3, positional.length); n >= 1; n--) {
    const cand = positional.slice(0, n).join(' ');
    if (names.includes(cand)) return cand;
    const alias = norm(cand);
    if (names.includes(alias)) return alias;
  }
  // Hyphenated alias of a multi-word command given as one token
  const first = norm(positional[0]);
  for (let n = Math.min(3, first.split(' ').length + positional.length - 1); n >= 1; n--) {
    const cand = [...first.split(' '), ...positional.slice(1)].slice(0, n).join(' ');
    if (names.includes(cand)) return cand;
  }
  return positional[0];
}

function redactedFlags(args) {
  const out = [];
  for (const a of args) {
    if (a === '--') break;
    if (a.startsWith('-') && a.length > 1) out.push(a.split('=')[0]);
  }
  return out;
}

function writeTrace(args, exit, bytesOut, protocol) {
  const file = process.env.DISTINCT_TRACE;
  if (!file) return;
  try {
    const event = {
      ts: new Date().toISOString(),
      host: process.env.DISTINCT_HOST || 'unknown',
      route: process.env.DISTINCT_ROUTE || null,
      command: commandWords(args),
      args_redacted: redactedFlags(args),
      exit,
      bytes_out: bytesOut,
      protocol,
    };
    appendFileSync(file, JSON.stringify(event) + '\n');
  } catch (e) {
    traceWarn(e && e.message ? e.message : String(e));
  }
}

function exitWith(code) {
  process.stdout.write('', () => process.exit(code));
}

function emitUnavailable(command) {
  const envelope = {
    protocol_version: PROTOCOL_VERSION,
    engine_version: '0.2.0',
    execution: { engine: 'node-launcher', command },
    result: null,
    limitations: ['native binary not available'],
    changed: [],
    error: `native distinct-rs not found or protocol mismatch. Set DISTINCT_RS_BIN or ensure distinct-rs is on PATH.`,
  };
  const line = JSON.stringify(envelope);
  console.log(line);
  return Buffer.byteLength(line + '\n');
}

function main() {
  checkNodeVersion();

  const args = process.argv.slice(2);

  // --help and --version work without native
  if (args.includes('--help') || args.includes('-h')) {
    console.log(`distinct — brand skill launcher

Usage: distinct <command> [options]

Commands are delegated to the native distinct-rs binary.
Run 'distinct --help' for the full command list.

Skill root: ${SKILL_ROOT}

Environment:
  DISTINCT_RS_BIN  Override native binary path
  DISTINCT_TRACE   Append one JSONL metadata line per call (opt-in)

Requires Node >= ${MIN_NODE_MAJOR}.`);
    process.exit(0);
  }

  if (args.includes('--version') || args.includes('-V')) {
    console.log(`distinct launcher 0.2.0`);
    process.exit(0);
  }

  // Find and probe native
  const native = findNative();
  if (!native) {
    // Required command with no native: emit unavailable envelope
    const command = args[0] || 'unknown';
    const n = emitUnavailable(command);
    writeTrace(args, 2, n, null);
    exitWith(2);
    return;
  }

  const probe = probeProtocol(native);
  if (!probe) {
    const command = args[0] || 'unknown';
    const n = emitUnavailable(command);
    writeTrace(args, 2, n, null);
    exitWith(2);
    return;
  }

  // Delegate to native, injecting --skill-root so the binary knows where methods/schemas live
  const delegatedArgs = [...args];
  if (!delegatedArgs.includes('--skill-root')) {
    delegatedArgs.push('--skill-root', SKILL_ROOT);
  }

  if (!process.env.DISTINCT_TRACE) {
    const result = spawnSync(native, delegatedArgs, {
      stdio: ['inherit', 'inherit', 'inherit'],
    });
    process.exit(result.status ?? 3);
  }

  // Tracing: capture stdout only to count bytes, then pass it through unchanged.
  const result = spawnSync(native, delegatedArgs, {
    stdio: ['inherit', 'pipe', 'inherit'],
    maxBuffer: 512 * 1024 * 1024,
  });
  const out = result.stdout || Buffer.alloc(0);
  const code = result.status ?? 3;
  writeTrace(args, code, out.length, probe.protocol_version);
  if (out.length > 0) {
    process.stdout.write(out, () => process.exit(code));
  } else {
    process.exit(code);
  }
}

main();
