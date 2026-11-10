// Deterministic generator for the synthetic calibration fixtures.
// Node built-ins only, fixed constants, no randomness, no clock.
// Usage: node generate.mjs [--out <dir>]   (default: this directory)
// Writes <out>/svg/*.svg and <out>/manifest.json.
import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const INK = '#111111';

// Number formatting: 2 decimals, trailing zeros trimmed, no "-0".
const n = (v) => {
  const s = (Math.round(v * 100) / 100).toFixed(2).replace(/\.?0+$/, '');
  return s === '-0' ? '0' : s;
};

const wrap = (id, body, w = 512, h = 512) =>
  `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">` +
  `<desc>${id}</desc>${body}</svg>\n`;

const rot = ([x, y], k) => {
  // rotate k quarter turns clockwise about (256,256)
  let px = x - 256;
  let py = y - 256;
  for (let i = 0; i < k; i++) [px, py] = [-py, px];
  return [px + 256, py + 256];
};
const poly = (pts) => 'M' + pts.map(([x, y]) => `${n(x)} ${n(y)}`).join(' L') + ' Z';

// ---- marks ----------------------------------------------------------------

function c1() {
  const ring = 'M56 256 a200 200 0 1 0 400 0 a200 200 0 1 0 -400 0 Z';
  const sq = 'M176 176 H336 V336 H176 Z';
  return wrap('C1', `<path d="${ring} ${sq}" fill="${INK}" fill-rule="evenodd"/>`);
}

function c2() {
  const body =
    `<rect x="208" y="56" width="96" height="400" fill="${INK}"/>` +
    `<rect x="56" y="208" width="400" height="96" fill="${INK}"/>` +
    [[112, 112], [400, 112], [112, 400], [400, 400]]
      .map(([x, y]) => `<circle cx="${x}" cy="${y}" r="48" fill="${INK}"/>`)
      .join('');
  return wrap('C2', body);
}

function twoPlate(id, accent) {
  const body =
    `<rect x="0" y="0" width="256" height="512" fill="#ffffff"/>` +
    `<rect x="256" y="0" width="256" height="512" fill="#101820"/>` +
    `<circle cx="128" cy="256" r="88" fill="#0b1f4d"/>` +
    `<rect x="320" y="192" width="128" height="128" fill="${accent}"/>`;
  return wrap(id, body);
}

function f1() {
  // Chiral C4 plate with four inward arrows knocked out of it. Each arrow is
  // offset off its own axis, so the set has rotation but no mirror symmetry.
  const x = 286;
  const arrow = [
    [x - 18, 70], [x + 18, 70], [x + 18, 150], [x + 60, 150],
    [x, 240], [x - 60, 150], [x - 18, 150],
  ];
  let d = 'M56 56 H456 V456 H56 Z';
  for (let k = 0; k < 4; k++) d += ' ' + poly(arrow.map((p) => rot(p, k)));
  return wrap('F1', `<path d="${d}" fill="${INK}" fill-rule="evenodd"/>`);
}

function arch(x0, y0, s) {
  // filled arch: outer radius 100s, inner radius 30s, legs to baseline
  const R = 100 * s;
  const r = 30 * s;
  const h = 150 * s; // leg height below the arc start
  return (
    `M${n(x0)} ${n(y0 + h)} V${n(y0)} a${n(R)} ${n(R)} 0 0 1 ${n(2 * R)} 0 V${n(y0 + h)} ` +
    `H${n(x0 + 2 * R - 70 * s)} V${n(y0)} a${n(r)} ${n(r)} 0 0 0 ${n(-2 * r)} 0 V${n(y0 + h)} Z`
  );
}

function f2() {
  const body =
    `<path d="${arch(60, 250, 1)}" fill="${INK}"/>` +
    `<path d="${arch(290, 250, 1)}" fill="${INK}"/>` +
    `<rect x="260" y="330" width="30" height="2" fill="${INK}"/>`;
  return wrap('F2', body);
}

function f2lockup() {
  const s = 0.5;
  const marks =
    `<path d="${arch(20, 78, s)}" fill="${INK}"/>` +
    `<path d="${arch(135, 78, s)}" fill="${INK}"/>` +
    `<rect x="120" y="116" width="15" height="1" fill="${INK}"/>`;
  // generic outlined wordmark: eight rectangles, no font
  let word = '';
  for (let i = 0; i < 8; i++) {
    const h = i % 2 === 0 ? 70 : 50;
    word += `<rect x="${250 + i * 32}" y="${153 - h}" width="22" height="${h}" fill="none" stroke="${INK}" stroke-width="3"/>`;
  }
  return wrap('F2-lockup', marks + word, 512, 192);
}

function powerGlyph(id, stroke, gapDeg, cy = 276, r = 150) {
  const a = (gapDeg * Math.PI) / 180;
  const sx = 256 + r * Math.sin(a);
  const sy = cy - r * Math.cos(a);
  const ex = 256 - r * Math.sin(a);
  const arc = `M${n(sx)} ${n(sy)} A${r} ${r} 0 1 1 ${n(ex)} ${n(sy)}`;
  return wrap(
    id,
    `<path d="${arc}" fill="none" stroke="${INK}" stroke-width="${stroke}" stroke-linecap="round"/>` +
      `<line x1="256" y1="${cy - r - 20}" x2="256" y2="${cy - 40}" stroke="${INK}" stroke-width="${stroke}" stroke-linecap="round"/>`,
  );
}

function f4() {
  return twoPlate('F4', '#5a6684');
}

function f5() {
  // solid outline ring plus a dot-shaded half disc at 6-unit pitch
  let dots = '';
  for (let y = 159; y <= 353; y += 6) {
    for (let x = 159; x <= 353; x += 6) {
      const dx = x - 256;
      const dy = y - 256;
      if (dx * dx + dy * dy <= 97 * 97 && dx + dy < 0) {
        dots += `<circle cx="${x}" cy="${y}" r="1.5" fill="${INK}"/>`;
      }
    }
  }
  const ring = `<circle cx="256" cy="256" r="190" fill="none" stroke="${INK}" stroke-width="28"/>`;
  return wrap('F5', ring + `<g>${dots}</g>`);
}

function f6() {
  const body =
    `<circle cx="300" cy="256" r="156" fill="${INK}"/>` +
    `<circle cx="96" cy="256" r="40" fill="${INK}"/>`;
  return wrap('F6', body);
}

function f7() {
  const pts = [[256, 56], [419.2, 156], [419.2, 356], [256, 456], [96, 356], [96, 156]];
  return wrap('F7', `<polygon points="${pts.map(([x, y]) => `${n(x)},${n(y)}`).join(' ')}" fill="${INK}"/>`);
}

function f8() {
  // a plain ring emitted as 3000 tiny chords
  const N = 3000;
  let body = `<g fill="none" stroke="${INK}" stroke-width="24" stroke-linecap="round">`;
  for (let i = 0; i < N; i++) {
    const a0 = (2 * Math.PI * i) / N;
    const a1 = (2 * Math.PI * (i + 1)) / N;
    body +=
      `<line x1="${n(256 + 200 * Math.cos(a0))}" y1="${n(256 + 200 * Math.sin(a0))}" ` +
      `x2="${n(256 + 200 * Math.cos(a1))}" y2="${n(256 + 200 * Math.sin(a1))}"/>`;
  }
  return wrap('F8', body + '</g>');
}

function f9() {
  const body =
    `<defs><clipPath id="c"><rect x="56" y="56" width="400" height="260"/></clipPath></defs>` +
    `<circle cx="256" cy="256" r="200" fill="${INK}" clip-path="url(#c)"/>`;
  return wrap('F9', body);
}

function r1(variant = false) {
  const body =
    `<rect x="96" y="296" width="80" height="120" fill="${INK}"/>` +
    `<rect x="216" y="216" width="80" height="200" fill="${INK}"/>` +
    `<rect x="336" y="136" width="80" height="280" fill="${INK}"/>` +
    `<circle cx="${variant ? 148 : 136}" cy="196" r="40" fill="${INK}"/>`;
  return wrap(variant ? 'F10' : 'R1', body);
}

function stockHome() {
  return wrap(
    'R-stock-home',
    `<polyline points="76,266 256,96 436,266" fill="none" stroke="${INK}" stroke-width="32" stroke-linejoin="round" stroke-linecap="round"/>` +
      `<polyline points="126,246 126,416 386,416 386,246" fill="none" stroke="${INK}" stroke-width="32" stroke-linejoin="round" stroke-linecap="round"/>`,
  );
}

function stockBrackets() {
  const L = (pts) =>
    `<polyline points="${pts}" fill="none" stroke="${INK}" stroke-width="32" stroke-linecap="square"/>`;
  return wrap(
    'R-stock-brackets',
    L('96,200 96,96 200,96') + L('312,96 416,96 416,200') + L('416,312 416,416 312,416') + L('200,416 96,416 96,312'),
  );
}

// ---- manifest -------------------------------------------------------------

const flaw = (type, rubric_item, detectable, note, future) => {
  const o = { type, rubric_item, detectable };
  if (future) o.future = future;
  o.note = note;
  return o;
};

const COLOURS_C3 = [
  { fg: '#0b1f4d', bg: '#ffffff', expect_lc_min: 101.6, expect_lc_max: 103.6 },
  { fg: '#ffd400', bg: '#101820', expect_lc_min: -82.9, expect_lc_max: -80.9 },
];
const COLOURS_F4 = [
  { fg: '#0b1f4d', bg: '#ffffff', expect_lc_min: 101.6, expect_lc_max: 103.6 },
  { fg: '#5a6684', bg: '#101820', expect_lc_min: -23.1, expect_lc_max: -21.1 },
];

// expect_engine values were taken from real engine output (see README).
const OK = { inspect_verdict: 'inspected', proof: 'ok' };

const FIXTURES = [
  { id: 'C1', role: 'control', make: c1, flaws: [], expect_engine: OK, colours: null },
  { id: 'C2', role: 'control', make: c2, flaws: [], expect_engine: OK, colours: null },
  { id: 'C3', role: 'control', make: () => twoPlate('C3', '#ffd400'), flaws: [], expect_engine: OK, colours: COLOURS_C3 },
  {
    id: 'F1', role: 'flawed', make: f1, expect_engine: OK, colours: null,
    flaws: [flaw('negative_space_reading', 'negative_space_hazard', 'judged', 'chiral C4 plate whose gaps read as a pinwheel of inward arrows')],
  },
  {
    id: 'F2', role: 'flawed', make: f2, expect_engine: OK, colours: null,
    flaws: [flaw('small_size_misread', 'small_size_legibility', 'judged', 'thin bridge vanishes at 24 px, two arches read as two letters', 'measured@0.5')],
  },
  {
    id: 'F2-lockup', role: 'flawed', make: f2lockup, expect_engine: OK, colours: null,
    flaws: [flaw('small_size_misread', 'small_size_legibility', 'judged', 'same flaw as F2 checked beside a rectangle wordmark', 'measured@0.5')],
  },
  {
    id: 'F3', role: 'flawed', make: () => powerGlyph('F3', 40, 35), expect_engine: OK, colours: null,
    flaws: [flaw('stock_icon_collision', 'distinctiveness', 'judged', 'ring with a top gap and a vertical bar reads as a generic power toggle', 'measured@0.6')],
  },
  {
    id: 'F4', role: 'flawed', make: f4, expect_engine: OK, colours: COLOURS_F4,
    flaws: [flaw('accent_vanishes_on_dark', 'colour_contrast', 'measured', 'accent against the dark plate is near 3:1 WCAG equivalent')],
  },
  {
    id: 'F5', role: 'flawed', make: f5, expect_engine: OK, colours: null,
    flaws: [flaw('detail_collapse', 'small_size_legibility', 'judged', 'dot shading at a 6-unit pitch collapses below 64 px. Judged, not measured: min_feature_px counts axis-run length and does not detect dot or hatch collapse', 'measured@0.5')],
  },
  {
    id: 'F6', role: 'flawed', make: f6, expect_engine: OK, colours: null,
    flaws: [flaw('off_centre_mass', 'balance', 'judged', 'bbox centred but visual centroid sits about 8% off centre', 'measured@0.5')],
  },
  {
    id: 'F7', role: 'flawed', make: f7, expect_engine: OK, colours: null,
    flaws: [flaw('near_symmetry', 'balance', 'judged', 'near-symmetric form with one side 2% wider', 'measured@0.5')],
  },
  {
    id: 'F8', role: 'flawed', make: f8, expect_engine: OK, colours: null,
    flaws: [flaw('path_bloat', 'construction_quality', 'judged', 'simple ring emitted as 3000 tiny segments', 'measured@0.5')],
  },
  {
    id: 'F9', role: 'flawed', make: f9, expect_engine: { inspect_verdict: 'unsupported_content', proof: 'blocked' }, colours: null,
    flaws: [flaw('unsupported_construct', 'engine_safe', 'measured', 'uses clipPath, outside the supported profile')],
  },
  {
    id: 'F10', role: 'flawed', make: () => r1(true), expect_engine: OK, colours: null,
    flaws: [flaw('near_copy', 'distinctiveness', 'judged', 'copy of reference R1 with one minor change', 'measured@0.6')],
  },
  { id: 'R1', role: 'reference', make: () => r1(false), flaws: [], expect_engine: OK, colours: null },
  { id: 'R-stock-power', role: 'reference', make: () => powerGlyph('R-stock-power', 30, 40), flaws: [], expect_engine: OK, colours: null },
  { id: 'R-stock-home', role: 'reference', make: stockHome, flaws: [], expect_engine: OK, colours: null },
  { id: 'R-stock-brackets', role: 'reference', make: stockBrackets, flaws: [], expect_engine: OK, colours: null },
];

/** Returns { files: Map<relpath, string> } without touching the disk. */
export function generate() {
  const files = new Map();
  // Rubric items the engine measures today (single source for the critic scorer).
  const manifest = { version: 1, measured_items: ['colour_contrast', 'engine_safe'], fixtures: [] };
  for (const f of FIXTURES) {
    const file = `svg/${f.id}.svg`;
    files.set(file, f.make());
    manifest.fixtures.push({
      id: f.id,
      file,
      role: f.role,
      flaws: f.flaws,
      expect_engine: f.expect_engine,
      colours: f.colours,
    });
  }
  files.set('manifest.json', JSON.stringify(manifest, null, 2) + '\n');
  return { files };
}

export function writeTo(outDir) {
  mkdirSync(join(outDir, 'svg'), { recursive: true });
  for (const [rel, content] of generate().files) writeFileSync(join(outDir, rel), content);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const i = process.argv.indexOf('--out');
  const out = i > 0 ? resolve(process.argv[i + 1]) : dirname(fileURLToPath(import.meta.url));
  writeTo(out);
  process.stdout.write(`wrote fixtures to ${out}\n`);
}
