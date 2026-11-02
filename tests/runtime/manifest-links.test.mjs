// Every `references/*.md` that a shipped Markdown file routes to must itself
// ship: listed in runtime-manifest.json `include` and present on disk.
// Guards against a route pointing at a file the install leaves out.

import { it } from 'node:test';
import assert from 'node:assert/strict';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync, readFileSync } from 'node:fs';

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..');
const manifest = JSON.parse(readFileSync(resolve(repoRoot, 'runtime-manifest.json'), 'utf-8'));
const included = new Set(manifest.include);

it('shipped Markdown only routes to references that ship', () => {
  const missing = [];
  for (const file of manifest.include.filter((p) => p.endsWith('.md'))) {
    const text = readFileSync(resolve(repoRoot, file), 'utf-8');
    for (const [ref] of text.matchAll(/references\/[a-z0-9-]+\.md/g)) {
      if (!included.has(ref) || !existsSync(resolve(repoRoot, ref))) {
        missing.push(`${file} -> ${ref}`);
      }
    }
  }
  assert.deepEqual([...new Set(missing)], []);
});
