// WO-14: Frontmatter check against the agentskills spec.
// Every SKILL.md must have YAML frontmatter with:
//   - name: string, ≤ 1024 chars
//   - description: string, ≤ 1024 chars
import { test } from 'node:test';
import { readFileSync, readdirSync, statSync, existsSync } from 'node:fs';
import { resolve, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..', '..');

function parseFrontmatter(content) {
  if (!content.startsWith('---')) return null;
  const end = content.indexOf('---', 3);
  if (end === -1) return null;
  const body = content.slice(3, end).trim();
  const fm = {};
  for (const line of body.split('\n')) {
    const m = line.match(/^(\w[\w-]*):\s*(.*)$/);
    if (m) fm[m[1]] = m[2].replace(/^["']|["']$/g, '');
  }
  return fm;
}

function checkSkillMd(path, label) {
  const content = readFileSync(path, 'utf-8');
  const fm = parseFrontmatter(content);
  if (!fm) return [`${label}: missing or malformed frontmatter`];
  const errors = [];
  if (!fm.name || typeof fm.name !== 'string') {
    errors.push(`${label}: frontmatter missing 'name'`);
  } else if (fm.name.length > 1024) {
    errors.push(`${label}: 'name' is ${fm.name.length} chars, max 1024`);
  }
  if (!fm.description || typeof fm.description !== 'string') {
    errors.push(`${label}: frontmatter missing 'description'`);
  } else if (fm.description.length > 1024) {
    errors.push(`${label}: 'description' is ${fm.description.length} chars, max 1024`);
  }
  return errors;
}

test('root SKILL.md has valid agentskills frontmatter', () => {
  const errors = checkSkillMd(join(ROOT, 'SKILL.md'), 'SKILL.md');
  if (errors.length > 0) throw new Error(errors.join('\n'));
});

test('each skills/*/SKILL.md has valid agentskills frontmatter', () => {
  const skillsDir = join(ROOT, 'skills');
  if (!existsSync(skillsDir)) return;
  const dirs = readdirSync(skillsDir).filter((d) => {
    try { return statSync(join(skillsDir, d)).isDirectory(); } catch { return false; }
  });
  const allErrors = [];
  for (const d of dirs) {
    const skillMd = join(skillsDir, d, 'SKILL.md');
    if (!existsSync(skillMd)) continue;
    allErrors.push(...checkSkillMd(skillMd, `skills/${d}/SKILL.md`));
  }
  if (allErrors.length > 0) throw new Error(allErrors.join('\n'));
});
