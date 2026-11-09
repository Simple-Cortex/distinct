# Marks as code

Construct logo candidates from a small generator script instead of hand-written path strings. Re-running with changed constants is the refinement step. Verdicts come from the critic in `references/critique.md`, not from the builder.

Evidence for the approach is thin: arXiv 2509.05208 (abstract only) reports that RL training on SVG-as-program tasks induces decomposition into primitives; this is not shown as a property of frontier models generally. No source isolates "write a generator" from "write SVG directly". Treat this as a working method, not a proven gain.

## Generator

Write a short Python or JS script in the workspace that outputs one SVG.
- Declare one grid unit (for example a 24 or 48 unit module) and derive every dimension from named constants (`U`, `STROKE`, `GAP`, `RADIUS`). No magic numbers in path data.
- Build from primitives: circle, rectangle, arc, line. Combine with booleans (union, difference, intersection) and offsets for cutouts and rounding.
- Emit a fixed `viewBox`, filled paths, no scripts, no external images, no live `<text>` (see Engine-safe SVG). `logo proof` uses resvg, which has no native text rendering, so live text renders wrong or missing. Outline wordmarks.
- Keep one generator per candidate, parameters at the top, a comment per constant saying what it controls.
- Import both the generator source and the SVG with `evidence import --file <f> --source-id <id>` (use `--replace` only when you mean to overwrite an id). Then record the SVG with `logo inspect`.

## Engine-safe SVG

`logo inspect` returns `inspected`, `unsupported_content` or `invalid`. Allowed elements only: `svg g path rect circle ellipse line polyline polygon title desc metadata defs`. Anything else (`text`, `image`, `use`, `symbol`, `style`, `clipPath`, `mask`, `linearGradient`, `radialGradient`, `script`) gives `unsupported_content`, and `logo proof` refuses it. Generators must emit flat filled paths or primitives with presentation attributes, outline all text, and use no gradients, clips, masks, `use` or `style`.

## Tools

Optional. Check availability first. Never install anything without the user's permission.

```bash
python3 -c "import pathops"      # skia-pathops
python3 -c "import picosvg"     # picosvg
python3 -c "import fontTools"   # fontTools
which svgo vtracer
```

| Tool | Licence | Use |
|---|---|---|
| skia-pathops | BSD-3 | booleans, simplify, stroke-to-fill; prebuilt wheel for your Python and chip is unverified |
| picosvg | Apache-2.0 | normalise (flatten transforms, strokes to outlines, only paths) before `logo inspect`; it rejects features it cannot flatten, a useful strictness gate |
| SVGO | MIT | final export only (see below) |
| fontTools | MIT | outline wordmark glyphs to paths; exact pen API names unverified, check docs |
| vtracer | MIT | optional: trace a rough sketch raster, then clean up and rebuild in the generator |

Fonts for wordmarks: OFL fonts only. Record the font name and its licence in the evidence. Inter (rsms/inter) and Source Sans (adobe-fonts/source-sans) were verified as OFL-1.1; for any other font read its own licence file. Read the OFL text before relying on its terms for outlined marks (unverified details).

Order: generator -> SVG -> picosvg (if available) -> `logo inspect` -> `logo proof`. Run SVGO only on the final export, never before inspect: it strips IDs and grouping that review relies on. SVGO changes bytes and hash, so re-run `logo inspect` and `logo proof` on the optimised file and import it as new evidence. Set `multipass` and path precision deliberately, keep `viewBox`, and diff proof renders before and after. Normalisers can shift geometry slightly.

Fallback when no tool is available or permitted: write the SVG by hand from the same constants, with the arithmetic done in the generator or stated in comments. Say which tools were missing.

Traced output is for ideation and rebuild-from-reference, not final art. It is node-heavy and soft-cornered.

## Avoid

| Item | Reason |
|---|---|
| potrace, autotrace (GPL) | GPL; do not link or run in-process; external CLI only if counsel agrees |
| Inkscape CLI (GPL v2+) | same; heavy install |
| OmniSVG 3B/4B | base model Qwen2.5-VL-3B is under the non-commercial `qwen-research` licence |
| OmniSVG training data (MMSVG) | CC-BY-NC-SA-4.0; downstream status of weights unresolved, needs counsel |
| SVGDreamer, VectorFusion, DiffSketcher | depend on Stable Diffusion weights, whose licence terms restrict use; get counsel before relying on them |
| Chat2SVG, RefineSVG, VCode code | no licence detected on GitHub; reference reading only |
| LIVE, DeepSVG, IconShop, SVGFusion | stale or icon-scale research models |
| Clipper2, rustybuzz, svgwrite, scour | archived or stale |
| StarVector 8B | base `bigcode/starcoder2-7b`; its licence terms were not opened (unverified) |

## Render, look, fix loop

1. Run the generator, then `logo inspect --file <svg>`. Fix structural findings first.
2. `logo proof` with a profile that includes 16, 24, 32 and 512 px, transparent and `#ffffff` backgrounds.
3. Open the PNGs and look. Name one defect and its class (MEASURED from the proof record, or JUDGED from viewing).
4. Change constants in the generator, not individual nodes in the output. Hand-nudged nodes break the construction and the next re-run loses them.
5. Re-render and compare numerically where you can (pixel or edge-map difference against the previous round or a reference; no engine command does this, so it is a host script) instead of "looks fine".
6. Cap at 3-4 rounds per candidate, then hand the proofs to the critic per `references/critique.md`. The critic's findings, not your own read, decide whether the candidate advances.
7. Layered building helps: render after each primitive group, not only at the end.

## Optical checks (JUDGED)

Look at these in the renders. They are prompts for the critic, not rules.
- Round forms may need slight overshoot against flat ones to look equal. Typical type-design values are around 1-3% (unsourced rule of thumb); adjust one constant and compare.
- Stroke contrast and thin parts at 16 and 24 px: do hairlines close or vanish.
- Counters and gaps that fill in at small size.
- Visual centre: area-weighted balance can differ from the bounding box centre. Report as an observation, not an error.
- Curve joins: kinks where arcs meet lines.
