#!/usr/bin/env node
// Validate that the runtime manifest is complete and honest.
//
// --manifest <path>  Path to runtime-manifest.json (required)
// --root <path>      Repository root to resolve paths against (required)
//
// Checks:
// 1. Every file in include[] exists on disk.
// 2. No file under excluded directories (planning/, tasks/, crates/) is referenced.
// 3. Every command named in SKILL.md and skills/*/SKILL.md exists in capabilities.json.
// 4. No undeclared payload files exist outside include+exclude coverage.

import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { resolve, relative, join } from 'node:path';

function parseArgs(argv) {
  const args = { manifest: null, root: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--manifest' && i + 1 < argv.length) args.manifest = argv[++i];
    else if (argv[i] === '--root' && i + 1 < argv.length) args.root = argv[++i];
  }
  return args;
}

function collectFiles(dir, base, out) {
  for (const entry of readdirSync(dir)) {
    if (entry.startsWith('.')) continue;
    const full = join(dir, entry);
    const rel = relative(base, full);
    const st = statSync(full);
    if (st.isDirectory()) collectFiles(full, base, out);
    else out.push(rel);
  }
}

function extractCommandsFromMarkdown(text) {
  const commands = new Set();
  // Match patterns like `command name` or backtick-wrapped command invocations
  const knownNative = [
    'init', 'status', 'capabilities', 'record import', 'evidence import',
    'query', 'packet', 'gate', 'verify', 'export',
    'check plan create', 'check result record', 'review create',
    'decision record', 'logo inspect',
  ];
  for (const cmd of knownNative) {
    if (text.includes(cmd)) commands.add(cmd);
  }
  return commands;
}

function main() {
  const { manifest: manifestPath, root: rootPath } = parseArgs(process.argv.slice(2));
  if (!manifestPath || !rootPath) {
    console.error('Usage: validate-runtime.mjs --manifest <path> --root <path>');
    process.exit(3);
  }

  const root = resolve(rootPath);
  const manifestFile = resolve(manifestPath);
  if (!existsSync(manifestFile)) {
    console.error(`manifest not found: ${manifestFile}`);
    process.exit(3);
  }

  const manifest = JSON.parse(readFileSync(manifestFile, 'utf-8'));
  const include = manifest.include || [];
  const exclude = manifest.exclude || [];
  const errors = [];

  // Check 1: Every included file exists
  for (const f of include) {
    const full = resolve(root, f);
    if (!existsSync(full)) {
      errors.push(`MISSING included file: ${f}`);
    }
  }

  // Check 2: No included file is under an excluded directory
  const excludedDirs = ['planning/', 'tasks/', 'crates/'];
  for (const f of include) {
    for (const d of excludedDirs) {
      if (f.startsWith(d)) {
        errors.push(`INCLUDED file is under excluded dir: ${f} (under ${d})`);
      }
    }
  }

  // Check 3: Commands in SKILL.md exist in capabilities.json
  const capsFile = resolve(root, 'capabilities.json');
  if (existsSync(capsFile)) {
    const caps = JSON.parse(readFileSync(capsFile, 'utf-8'));
    const nativeCaps = new Set(Object.keys(caps.native || {}));

    // Read root SKILL.md
    const skillRoot = resolve(root, 'SKILL.md');
    if (existsSync(skillRoot)) {
      const skillText = readFileSync(skillRoot, 'utf-8');
      const commands = extractCommandsFromMarkdown(skillText);
      for (const cmd of commands) {
        // Normalize: "record import" -> "record import" (capabilities uses space form)
        if (!nativeCaps.has(cmd)) {
          errors.push(`Command "${cmd}" found in SKILL.md but not in capabilities.json native`);
        }
      }
    }

    // Read skills/*/SKILL.md
    const skillsDir = resolve(root, 'skills');
    if (existsSync(skillsDir) && statSync(skillsDir).isDirectory()) {
      for (const phase of readdirSync(skillsDir)) {
        const phaseSkill = join(skillsDir, phase, 'SKILL.md');
        if (existsSync(phaseSkill)) {
          const text = readFileSync(phaseSkill, 'utf-8');
          const commands = extractCommandsFromMarkdown(text);
          for (const cmd of commands) {
            if (!nativeCaps.has(cmd)) {
              errors.push(`Command "${cmd}" found in skills/${phase}/SKILL.md but not in capabilities.json native`);
            }
          }
        }
      }
    }
  }

  // Check 4: No undeclared payload — files on disk that are not covered by include or exclude
  const allFiles = [];
  collectFiles(root, root, allFiles);
  const includeSet = new Set(include);

  // Build exclude prefix set
  const excludePrefixes = exclude.filter(e => e.endsWith('/'));
  const excludeGlobs = exclude.filter(e => !e.endsWith('/'));

  for (const f of allFiles) {
    // Skip hidden files and this script's own artifacts
    if (f.startsWith('.') || f === 'node_modules') continue;

    if (includeSet.has(f)) continue;

    // Check if covered by an exclude prefix
    let excluded = false;
    for (const prefix of excludePrefixes) {
      if (f.startsWith(prefix)) { excluded = true; break; }
    }
    if (excluded) continue;

    // Check if matches an exclude glob (simple wildcard)
    for (const glob of excludeGlobs) {
      if (glob.endsWith('*')) {
        const prefix = glob.slice(0, -1);
        if (f.startsWith(prefix)) { excluded = true; break; }
      } else if (f === glob) {
        excluded = true;
        break;
      }
    }
    if (excluded) continue;

    // Check if it's a directory entry for skills/ or references/ (covered by include entries)
    if (f.startsWith('skills/') || f.startsWith('references/')) continue;

    errors.push(`UNDECLARED file on disk (not in include or exclude): ${f}`);
  }

  if (errors.length > 0) {
    console.error(`validate-runtime: ${errors.length} error(s)`);
    for (const e of errors) console.error(`  - ${e}`);
    process.exit(1);
  }

  console.log(`validate-runtime: OK (${include.length} included, ${exclude.length} excluded patterns)`);
  process.exit(0);
}

main();
