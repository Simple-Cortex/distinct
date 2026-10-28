#!/usr/bin/env node
// Install the Distinct runtime package into a destination directory.
//
// --dest <path>    Required. Target directory to install into.
// --force          Allow overwriting an existing non-empty destination.
// --root <path>    Source root (defaults to the directory containing this script's parent).
//
// Copies only the files listed in runtime-manifest.json include[].
// No network access. Never builds or downloads a native binary.

// Node version guard: require Node 20+.
const MIN_NODE_MAJOR = 20;
const currentMajor = parseInt(process.versions.node.split('.')[0], 10);
if (currentMajor < MIN_NODE_MAJOR) {
  process.stderr.write(
    `distinct install: Node ${currentMajor} is below minimum required v${MIN_NODE_MAJOR}\n`
  );
  process.exit(1);
}

import { readFileSync, existsSync, mkdirSync, readdirSync, statSync, copyFileSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

function parseArgs(argv) {
  const args = { dest: null, force: false, root: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--dest' && i + 1 < argv.length) args.dest = argv[++i];
    else if (argv[i] === '--force') args.force = true;
    else if (argv[i] === '--root' && i + 1 < argv.length) args.root = argv[++i];
  }
  return args;
}

function isNonEmpty(dir) {
  if (!existsSync(dir)) return false;
  return readdirSync(dir).length > 0;
}

function ensureDir(dir) {
  if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
}

function main() {
  const { dest, force, root: rootArg } = parseArgs(process.argv.slice(2));
  if (!dest) {
    console.error('Usage: install.mjs --dest <path> [--force] [--root <path>]');
    process.exit(3);
  }

  const thisFile = fileURLToPath(import.meta.url);
  const defaultRoot = resolve(dirname(thisFile), '..');
  const root = rootArg ? resolve(rootArg) : defaultRoot;
  const destDir = resolve(dest);

  // Refuse existing non-empty destination without --force
  if (!force && isNonEmpty(destDir)) {
    console.error(`install: destination ${destDir} is non-empty. Use --force to overwrite.`);
    process.exit(1);
  }

  // Read manifest
  const manifestPath = join(root, 'runtime-manifest.json');
  if (!existsSync(manifestPath)) {
    console.error(`install: runtime-manifest.json not found at ${manifestPath}`);
    process.exit(1);
  }
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf-8'));
  const include = manifest.include || [];

  if (include.length === 0) {
    console.error('install: manifest include list is empty');
    process.exit(1);
  }

  // Verify all source files exist before copying anything
  for (const f of include) {
    const src = join(root, f);
    if (!existsSync(src)) {
      console.error(`install: source file missing: ${f}`);
      process.exit(1);
    }
  }

  // Create destination
  ensureDir(destDir);

  // Copy each included file
  let copied = 0;
  for (const f of include) {
    const src = join(root, f);
    const dst = join(destDir, f);
    ensureDir(dirname(dst));
    copyFileSync(src, dst);
    copied++;
  }

  console.log(`install: copied ${copied} file(s) to ${destDir}`);
  process.exit(0);
}

main();
