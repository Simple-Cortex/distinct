// WO-14: Size-budget test for skill payload files.
// Root SKILL.md ≤ 6144 B; each references/*.md ≤ 16384 B; each skills/*/SKILL.md ≤ 1024 B.
// Oversize files must be trimmed or split.
import { test } from 'node:test';
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..', '..');

const LIMITS = {
  'SKILL.md': 6144,
  'references': 16384,
  'skills': 1024,
};

function fileSize(path) {
  return statSync(path).size;
}

test('root SKILL.md is within the 6144-byte budget', () => {
  const path = join(ROOT, 'SKILL.md');
  const size = fileSize(path);
  if (size > LIMITS['SKILL.md']) {
    throw new Error(`SKILL.md is ${size} bytes, exceeds ${LIMITS['SKILL.md']} budget. Trim or split the file.`);
  }
});

test('each references/*.md is within the 16384-byte budget', () => {
  const dir = join(ROOT, 'references');
  const files = readdirSync(dir).filter((f) => f.endsWith('.md'));
  const oversize = [];
  for (const f of files) {
    const size = fileSize(join(dir, f));
    if (size > LIMITS['references']) {
      oversize.push(`  references/${f}: ${size} bytes`);
    }
  }
  if (oversize.length > 0) {
    throw new Error(`Oversize reference files (>${LIMITS['references']} B):\n${oversize.join('\n')}\nTrim or split the file.`);
  }
});

test('each skills/*/SKILL.md is within the 1024-byte budget', () => {
  const skillsDir = join(ROOT, 'skills');
  if (!existsSync(skillsDir)) return; // no skills directory yet
  const dirs = readdirSync(skillsDir).filter((d) => {
    try { return statSync(join(skillsDir, d)).isDirectory(); } catch { return false; }
  });
  const oversize = [];
  for (const d of dirs) {
    const skillMd = join(skillsDir, d, 'SKILL.md');
    if (!existsSync(skillMd)) continue;
    const size = fileSize(skillMd);
    if (size > LIMITS['skills']) {
      oversize.push(`  skills/${d}/SKILL.md: ${size} bytes`);
    }
  }
  if (oversize.length > 0) {
    throw new Error(`Oversize skill files (>${LIMITS['skills']} B):\n${oversize.join('\n')}\nTrim or split the file.`);
  }
});
