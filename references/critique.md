# Self-critique protocol for logos and brand systems

Use in `direct`, `identity`, `review` and `iterate`. Commands: `references/commands.md`, `references/commands-assets.md`. The owner decision flow: `references/review.md`. Fix classes: `references/iteration.md`.

## Core finding

Models are weak judges of their own visual work. Evidence, with its limits:
- arXiv 2608.28678: off-the-shelf VLMs in an iterative SVG refinement harness showed "a deficit in visual reasoning and self-correction" (abstract only; no numbers read). Looping alone does not fix taste.
- Position bias depends on judge, task and quality gap (arXiv 2406.07791; text tasks).
- Same-family self-preference is reported (arXiv 2604.22891; search snippet only, unverified beyond that).
- Pairwise preferences flipped in about 35% of cases under distractor features versus about 9% for absolute scores (arXiv 2504.14716, "Pairwise or Pointwise?"; text tasks, not images).
- VLM judges often attend little to the image (arXiv 2604.17768, VQA setting, not logos). A cspaper.org summary about image-generation judges says they miss fine detail; secondary, verify originals before quoting. Judge reliability varies by model and dimension, so check each critic (see Calibration).
- No validated logo-quality score exists. Never report one aesthetic number.

Critique quality comes from measurement, independent critics and people, not from a better prompt.

## Claim classes

Every review finding carries one class.
- MEASURED: engine command output (`logo inspect`, `logo proof`, `color check`). Supports claims about the file only. Raster results such as `min_feature_px` and contrast are advisory, never a pass.
- JUDGED: a critic scored a named rubric item from the frozen proofs. State who judged, blind or not, and whether the order-swapped run agreed.
- OBSERVED: real participants, stored with `test import`. Only this class supports "memorable", "distinctive" or "recognisable". A JUDGED guess is written "predicted memorability, untested".

A verdict may claim only the strongest class that backs it. Thresholds are conventions unless the owner adopts them.

## Pass contract (in `direct`, before drawing)

Before any identity candidate is drawn, write the pass criteria and thresholds candidates must meet: rubric items below, sizes (16 and 24 px), one-colour and reverse behaviour, required contexts, similarity boundaries from the board `do_not_copy` and landscape marks, and which items need OBSERVED evidence.
- In `direct`, store it in the direction record's data, or a `source` record with `data.type: "pass_contract"`. Do NOT create a `check_plan` yet: governance blocks every gate scope while any `check_plan` lacks a `check_result`.
- At the start of `review`, create the `check_plan` (`check plan create`; rows: `row_id`, `check`, `method`; threshold in `check`; `method` e.g. `engine`, `host_review`, `test`), record its results with `check result record`, then run `gate`.
- The owner may adopt it; otherwise it is advisory.
- Candidates are scored against the stored contract. Changing a threshold after seeing candidates needs a recorded reason.

## Critic setup

1. Fresh context. Run the critic as a subagent that receives only the frozen packet (`logo review-packet` output, or the `review-packet` file) plus the image files: PNGs from `logo proof --out <dir>` and imported mockup or application renders (packets are metadata only, no image bytes), the rubric and the contract. Never pass builder rationale, sketches or chat history. If you cannot spawn one, record limitation `same_model_critic` or `same_context_critic`.
2. Different model family where the host has one. Otherwise record limitation `same_model_critic`.
3. Shuffle candidate order, score, then re-score in reversed order. The engine has no seed option, so note the order used in the review. Report items where the two runs differ by more than 1 point as unstable and treat them as low confidence.
4. Evidence required: each score names the proof image and region it rests on. Scores without a pointer are dropped.
5. Adversarial critic: a separate fresh context whose only task is reasons to reject (looks like X, unreadable at size, wrong or offensive reading, too complex to reproduce, needs colour to work). Each reason cites a proof. The builder may rebut in a separate step; the rebuttal never goes back to the scoring critic.
6. Competitor line-up: render the candidate with 3-5 fetched competitor marks from the landscape in one grid, same optical size, names and identifiers hidden. Ask the critic which one is the candidate and which marks it could be confused with. If candidates cannot be told from category conventions, record "category-generic risk". Use only marks you actually fetched and opened.
7. Aggregate per item: scores, run agreement, strongest unresolved objections, class. No composite score.

## Rubric

This is the agent critic's rubric: pointwise, per item, no composite. The nine-criterion average scorecard in `references/review.md` is the human-panel and owner instrument; do not merge the two.

Score pointwise, anchored 1-5 or binary, one item at a time. Use pairwise only to break ties between close candidates, and run it in both orders (the supporting study is on text tasks).

Criteria sources (secondary, not primary texts):
- Paul Rand, via a 1971 Graphis summary on logodesignlove.com: "distinctive, memorable, and reflect in some way, however abstractly, the nature of the product or service"; "practical and easily adapted to a variety of applications"; reproducible "in one or two colours, in positive and reverse form, and in sizes as large as building signs and as small as, or smaller than, calling cards".
- Sagi Haviv, via an expertmedia.lt article: appropriate, distinctive (memorable), simple; draw-from-memory test.
- Chermayeff & Geismar & Haviv: appropriate, distinctive, memorable. Seen only in low-quality snippets; the book "Identify" was not opened. Do not attribute more.
- Squint, fax and reduction tests are folk practice, not sourced doctrine. Call them "our operationalisation".

| Item | Anchor for 5 / pass | Class |
|---|---|---|
| Appropriate to the organisation | reflects activity, values and audience without explaining itself | JUDGED |
| Simple to reproduce | plausible to redraw from memory; few distinct parts | JUDGED; draw test is OBSERVED |
| Distinct in category | critic picks the candidate out of the line-up | JUDGED; market distinctiveness needs OBSERVED |
| Legible at 16 and 24 px | features survive; no merged counters | MEASURED where `logo proof` reports `min_feature_px`, else JUDGED |
| Works in one colour | no meaning lost without colour | JUDGED |
| Works reversed | holds on dark ground | JUDGED; contrast pairs MEASURED via `color check` |
| Silhouette | shape still reads when blurred or filled solid | JUDGED |
| Adaptable across applications | holds in the mockup contexts | JUDGED |
| Durability | low dependence on a current trend | JUDGED, weakest item |
| Memorable | recall by real people | OBSERVED only |

## Calibration

Before trusting a critic in a session, have it score 3-6 seed marks you make yourself: a few known-good (simple, distinct, legible) and a few known-bad (over-detailed, hairline strokes, near-duplicate of a generic shape). No third-party trademarks, and no LLD or LogoDet data (LLD is academic-only; LogoDet-3K licence is ambiguous). If the critic misranks seeds, record that as a limitation on every review it touches. Passing says little; failing is informative.

## Context gate

No positive verdict until imported evidence shows renders at 16 px and 24 px, one-colour, reversed, and at least one real application mockup. The critic must name one failure per context or state "none found" with a pointer. Missing contexts stay listed as gaps.

## Similarity

- Compare visually now, host-guided, against the board `do_not_copy` items and landscape marks you fetched. List the nearest few and why.
- Trademark register search is a documented manual human step. WIPO Global Brand Database terms (Nov 2022) prohibit automated queries, bulk download and scraping; USPTO terms say its web databases are not for bulk download and heavy automated access may be blocked. Give the owner the registers to search by hand or via counsel.
- Make no clearance claim. Say "not a trademark clearance".

## Iteration

- Cap at 3 critique -> fix rounds per candidate, then escalate to the owner with the unresolved objections.
- Each round names one defect and its class (MEASURED, JUDGED). Classify each fix as execution, system or strategy per `references/iteration.md`; strategy changes go back to `define`.
- Re-run the checks that failed and the contexts the fix could affect. Compare renders numerically where you can.

## Recording (existing commands only)

| Step | Command |
|---|---|
| Contract | `check plan create --file <check_plan.json>` (at start of `review`) |
| Measurements | `logo inspect --file <svg>`, `logo proof --file <svg> --profile <p> --out <dir>` (include 16 and 24 px), `color check --pairs <f>` |
| Frozen input for critics | `logo review-packet`; `review-packet --scope <id> --out <file>` for non-logo records |
| Contract results | `check result record --file <check_result.json>` (`check_plan_id`, observations by `row_id`) |
| Critic findings | `review create --file <review.json>` |
| Audience evidence | `test plan --request`, `test packet --test`, `test import --results`, `test summarize --run` (no built-in templates; 5-second recall, draw from memory and preference are protocols written as free-text `method` values in `test plan`) |
| Renders, mockups, critic outputs | `evidence import --file <f> --source-id <id>` |

Findings sit in `data.findings` as shown in `references/commands.md` (`id`, `severity`, `text`). The engine does not validate review `data`, so add `class` and `item` fields directly:

```json
{"id":"f3","severity":"major","class":"JUDGED","item":"legible-24px","text":"Critic other-family, runs disagree. Counter closes in 24px render (proof-24.png, upper right)."}
```

End each review with what was not checked (trademark, cultural reading in unfamiliar markets, print production, accessibility beyond colour) and the critic identity. A review is evidence, not approval; only the owner decides.
