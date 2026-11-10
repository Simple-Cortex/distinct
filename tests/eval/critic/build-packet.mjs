#!/usr/bin/env node
// Layer 2: build a blind critic packet from the calibration fixtures.
// Usage: node build-packet.mjs --seed N --out <dir> [--lineup] [--reverse]
// Needs the built engine (renders go through `logo proof`); no network, no model.
// Writes <dir>/packet/ (PNGs + neutral index.md) and <dir>/key.json (label -> fixture id).
// key.json and anything else outside packet/ must never be shown to the critic.
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { SKIP_REASON, freshWorkspace, inspectAndProof, loadFixtures, nativeBinary, tmp } from '../lib/engine.mjs';

const INK = '#111111';
const SIZES = [16, 24, 64, 512];
const PLATES = { light: '#ffffff', dark: '#101820' };

function parseArgs(argv) {
  const a = { lineup: false, reverse: false, seed: null, out: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--seed') a.seed = Number(argv[++i]);
    else if (argv[i] === '--out') a.out = argv[++i];
    else if (argv[i] === '--lineup') a.lineup = true;
    else if (argv[i] === '--reverse') a.reverse = true;
    else throw new Error(`unknown argument ${argv[i]}`);
  }
  if (!Number.isInteger(a.seed)) throw new Error('--seed N (integer) is required');
  if (!a.out) throw new Error('--out <dir> is required');
  return a;
}

// mulberry32: small seeded PRNG, deterministic across platforms.
function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function shuffle(arr, rand) {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

function dims(svg) {
  const m = /viewBox="\s*[-\d.]+[\s,]+[-\d.]+[\s,]+([\d.]+)[\s,]+([\d.]+)\s*"/.exec(svg);
  return m ? [Number(m[1]), Number(m[2])] : [512, 512];
}
const split = (svg) => {
  const open = /<svg\b[^>]*>/.exec(svg);
  const end = svg.lastIndexOf('</svg>');
  return { head: svg.slice(0, open.index + open[0].length), body: svg.slice(open.index + open[0].length, end), tail: svg.slice(end) };
};

/** Transformed SVG copies. No new engine features: a mirror group and a plate rect. */
export function mirrorSvg(svg) {
  const [w] = dims(svg);
  const { head, body, tail } = split(svg);
  return `${head}<g transform="translate(${w} 0) scale(-1 1)">${body}</g>${tail}`;
}
export function invertSvg(svg) {
  if (svg.includes('#101820') || svg.includes('#ffffff')) {
    // Colour fixtures carry their own two plates: swap the plates, keep the marks.
    return svg.split('#ffffff').join('@@P@@').split('#101820').join('#ffffff').split('@@P@@').join('#101820');
  }
  const [w, h] = dims(svg);
  const { head, body, tail } = split(svg);
  return `${head}<rect x="0" y="0" width="${w}" height="${h}" fill="${INK}"/>${body.split(INK).join('#f5f5f5')}${tail}`;
}

function profile(sizes, plates) {
  const renders = [];
  const names = [];
  for (const plate of plates) {
    for (const size of sizes) {
      renders.push({ size_px: size, background: PLATES[plate] });
      names.push({ size, plate });
    }
  }
  return { profile: { renders }, names };
}

function renderVariant(w, workDir, tag, svgText, sizes, plates) {
  const svgPath = join(workDir, `${tag}.svg`);
  writeFileSync(svgPath, svgText);
  const { profile: p, names } = profile(sizes, plates);
  const outDir = join(workDir, `${tag}-png`);
  const r = inspectAndProof(w, svgPath, outDir, p);
  if (r.proof !== 'ok') return null;
  const files = readdirSync(outDir);
  return names.map((n, i) => {
    const f = files.find((x) => x.startsWith(`render_${i}_`));
    return { ...n, file: join(outDir, f) };
  });
}

export function main(argv) {
  const args = parseArgs(argv);
  const bin = nativeBinary();
  if (!bin) throw new Error(SKIP_REASON);
  const out = resolve(args.out);
  const packetDir = join(out, 'packet');
  if (existsSync(packetDir) && readdirSync(packetDir).length) throw new Error(`${packetDir} is not empty`);
  mkdirSync(packetDir, { recursive: true });

  const fixtures = loadFixtures().sort((a, b) => a.id.localeCompare(b.id));
  const w = freshWorkspace(bin, 'evalpacket');
  const work = tmp('distinct-eval-work-');
  const skipped = [];

  const render = (f, full) => {
    const svg = readFileSync(f.path, 'utf8');
    const tag = f.id.replace(/[^A-Za-z0-9]/g, '_');
    const base = renderVariant(w, work, `${tag}-base`, svg, full ? SIZES : [64, 512], full ? ['light', 'dark'] : ['light']);
    if (!base) return null;
    const entry = { id: f.id, variants: { base } };
    if (full) {
      const m = renderVariant(w, work, `${tag}-mirror`, mirrorSvg(svg), SIZES, ['light']);
      if (m) entry.variants.mirror = m;
      const inv = invertSvg(svg);
      const v = inv ? renderVariant(w, work, `${tag}-invert`, inv, SIZES, ['light']) : null;
      if (v) entry.variants.invert = v;
    }
    return entry;
  };

  const cands = [];
  for (const f of fixtures.filter((x) => x.role === 'control' || x.role === 'flawed')) {
    const e = render(f, true);
    if (e) cands.push(e);
    else skipped.push(f.id);
  }
  const refs = [];
  if (args.lineup) {
    for (const f of fixtures.filter((x) => x.role === 'reference')) {
      const e = render(f, false);
      if (e) refs.push(e);
      else skipped.push(f.id);
    }
  }

  const rand = rng(args.seed);
  const order = shuffle(cands, rand);
  const refOrder = shuffle(refs, rand);
  const key = {};
  const labelled = [];
  const emit = (entries, prefix) => {
    entries.forEach((e, i) => {
      const label = `${prefix}-${String(i + 1).padStart(2, '0')}`;
      key[label] = e.id;
      const files = [];
      for (const [variant, list] of Object.entries(e.variants)) {
        for (const r of list) {
          const name = `${label}_${variant}_${r.size}px_${r.plate}.png`;
          copyFileSync(r.file, join(packetDir, name));
          files.push(name);
        }
      }
      labelled.push({ label, files, prefix });
    });
  };
  emit(order, 'K');
  emit(refOrder, 'E');

  const present = (prefix) => {
    const list = labelled.filter((x) => x.prefix === prefix);
    return args.reverse ? [...list].reverse() : list;
  };
  const section = (title, list) =>
    [`## ${title}`, '', ...list.flatMap((x) => [`### ${x.label}`, '', ...x.files.map((f) => `- ${f}`), ''])].join('\n');
  let md =
    '# Candidate marks\n\n' +
    'File names follow `<label>_<variant>_<size>_<plate>.png`. Variants: base, mirror (flipped), invert (light on dark).\n\n' +
    section('Candidates', present('K'));
  if (args.lineup) md += '\n' + section('Existing marks (for comparison only, not candidates)', present('E'));
  writeFileSync(join(packetDir, 'index.md'), md);

  writeFileSync(join(out, 'key.json'), JSON.stringify(key, null, 2) + '\n');
  writeFileSync(join(out, 'skipped.json'), JSON.stringify(skipped, null, 2) + '\n');
  process.stdout.write(
    `packet: ${order.length} candidates${args.lineup ? `, ${refOrder.length} existing marks` : ''}` +
      `${skipped.length ? `, ${skipped.length} not renderable (skipped)` : ''}\n` +
      `packet dir: ${packetDir}\nkey (keep private): ${join(out, 'key.json')}\n`,
  );
  return { key, skipped };
}

import { fileURLToPath } from 'node:url';
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    process.stderr.write(`build-packet: ${e.message}\n`);
    process.exit(2);
  }
}
