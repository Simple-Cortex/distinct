# Route: Iterate

## When to use
Use this route when an owner has provided specific feedback on a locked concept or approved system, or when perception testing has revealed execution flaws. This route refines the chosen direction; it does not replace it. If the feedback demands a completely new concept, the process has broken down and requires a return to `review` or `define`. Iteration happens in short, focused rounds. The goal is improving, adopting, and polishing, but never changing the core idea.

## Minimum inputs and prerequisite records
- **Current parent record:** The approved or conditionally approved platform, direction, and candidate asset.
- **Specific feedback:** Typed review records, perception-test results, or explicit owner instructions detailing what needs adjustment.
- **Protected invariants:** The core idea, semantic fit, and distinctiveness attributes that must not change.
- **Prerequisite records:**
  - `decision` (lock statement confirming the concept to refine).
  - `review` (the feedback triggering iteration).
  - `candidate` (the current asset being patched).

## Native commands for this route
Exact syntax: `references/commands.md`.
- `patch --plan <file> --dry-run` first. Stop on any warning, then `--apply --expected-state <hash>`.
- Patch only edits content records. It never approves anything, and it refuses governance kinds.
- After a visual patch, re-run `logo inspect` and then `logo proof` to retest. Compare the renders before accepting.

## Step-by-step procedure

### 1. Triage the feedback
Classify every piece of feedback into one of three buckets before taking action:
- **Execution:** Craft, scalability, accessibility, or system application issues (e.g., stroke weight disappearing at 16px, secondary color clashing in dark mode, print strokes below 6pt). These are valid targets for iteration.
- **System:** Touchpoint layout, hierarchy, pattern alignment, or template issues.
- **Strategy:** Challenges to the core idea, attributes, or distinctiveness (e.g., "this doesn't feel innovative anymore"). Strategy changes require unlocking the concept and returning to `define`.

Ask the owner (max 3 questions per batch):
1. "Are there any execution or system constraints we missed that are blocking production?"
2. "Does this feedback challenge the core idea we just locked, or is it about how the idea is applied?"
3. "Which specific touchpoint, size, or context is failing the hard checks?"

### 2. Define protected invariants and the minimal patch
Identify what must stay exactly the same. The mark's silhouette, core metaphor, and semantic fit are protected invariants. Draft a minimal patch plan.
If the feedback is a "Frankenstein request" (a mash-up of two rejected concepts or a demand to combine unrelated elements), explain objectively why it fails. A mash-up rarely solves the underlying problem and usually destroys the craft and semantic fit. Propose a minimal alternative that solves the underlying constraint without breaking the lock.

**Artifact to produce:** `iteration-plan.json`
- `parent_candidate_id`: The record being patched.
- `feedback_classification`: Array of execution/system/strategy labels.
- `protected_invariants`: Array of locked attributes (e.g., "circular silhouette", "interlocking metaphor").
- `minimal_patch_description`: Text description of the exact change.
- `frankenstein_rejection_rationale`: Null, or text explaining why a mash-up was refused.

### 3. Host-guided revision and inspection
The agent cannot natively edit Bézier curves, adjust optical weight, or perform vector topology repairs.
- **Host-guided step:** Instruct the user or host design tool to apply the minimal patch. Provide the iteration plan.
- **Native step:** Once the revised SVG is saved, run native inspection to ensure no structural regressions occurred (e.g., the host didn't accidentally introduce a `<filter>` or external `<image>`).
  ```bash
  node <skill-dir>/bin/distinct.mjs logo inspect --file <logo.svg> --workspace <project> --brand <id>
  ```
- Import the revised evidence bytes and metadata:
  ```bash
  node <skill-dir>/bin/distinct.mjs evidence import --source-id <id> --file <path> --workspace <project> --brand <id>
  ```
- Record the new candidate revision, linking it to the parent:
  ```bash
  node <skill-dir>/bin/distinct.mjs record import --file <candidate-revision.json> --workspace <project> --brand <id>
  ```

### 4. Re-evaluation and targeted testing
If the iteration addressed accessibility or scalability, verify the hard checks natively where possible (e.g., checking contrast ratios in the proof profile). If the iteration altered the mark's shape significantly, re-run the perception tests to ensure semantic fit and distinctiveness were not degraded.
- Import test results from the host:
  ```bash
  node <skill-dir>/bin/distinct.mjs record import --file <test-results.json> --workspace <project> --brand <id>
  ```
- Create a scoped review projection comparing the revision against the original scorecard thresholds:
  ```bash
  node <skill-dir>/bin/distinct.mjs review create --file <iteration-review.json> --workspace <project> --brand <id>
  ```

### 5. Decision and lock (or rollback)
Present the refined asset against the scorecard thresholds adopted for this scope—advisory course defaults (e.g., average ≥4.0, no criterion below 3.0, structural floors ≥4.0) may be used if the owner has adopted them. Confirm hard checks pass natively. Readiness is determined by the native gate plus a recorded owner decision.
If the owner decides it meets the scoped requirements, record the owner's approval of the refinement. The concept lock remains intact, but the approved candidate ID advances.
If the owner decides it fails or degrades the core idea, rollback to the previous parent record and re-triage.
- Record the decision:
  ```bash
  node <skill-dir>/bin/distinct.mjs decision record --file <refinement-decision.json> --owner <name> --workspace <project> --brand <id>
  ```

## Native command calls
Only use the exact invocation forms below. Replace placeholders with actual workspace/brand IDs and file paths.

- **Inspect revised SVG:**
  `node <skill-dir>/bin/distinct.mjs logo inspect --file <logo.svg> --workspace <project> --brand <id>`
  *Purpose:* Validates XML, viewBox, finite coordinates, and unsupported elements. Returns a `LogoFinding` record.

- **Record a new revision/candidate:**
  `node <skill-dir>/bin/distinct.mjs record import --file <revision.json> --workspace <project> --brand <id>`
  *Purpose:* Atomically stores the typed candidate record, updating the content snapshot.

- **Record a review of the iteration:**
  `node <skill-dir>/bin/distinct.mjs review create --file <review.json> --workspace <project> --brand <id>`
  *Purpose:* Stores the inspection/test evidence and scope reconciliation.

- **Record owner approval of the refinement:**
  `node <skill-dir>/bin/distinct.mjs decision record --file <decision.json> --owner <name> --workspace <project> --brand <id>`
  *Purpose:* Captures the explicit owner authorization over the exact current scope. The agent must not act as the owner.

- **Check readiness for export:**
  `node <skill-dir>/bin/distinct.mjs gate --scope <s> --workspace <project> --brand <id>`
  *Purpose:* Read-only reconciliation of plan/result/review/approval. Confirms no blockers remain.

## Host-guided fallbacks
- **Vector Editing / Optical Refinement:** Native automation of Bézier curve editing, kerning, or optical weight balancing is unavailable.
  - *Fallback:* Export a draft kit with the original asset and a written patch plan.
  - *Next step:* Wait for the host to import the revised SVG via `evidence import`.
- **Perception Testing:** Native recruitment and participant testing are unavailable.
  - *Fallback:* Provide the perception-test protocol script (1-second flash, 7-second poll, negative-reading sweep) and a blank results template.
  - *Next step:* Import the host's actual observations via `record import`.
- **Optical Verification:** The agent cannot judge if a curve looks "smooth" or if kerning is "optically balanced".
  - *Fallback:* Flag these as `needs_review` in the governance record. Require host optical verification before accepting the patch.

## Blockers
- **Strategy drift:** If the requested patch alters the core idea, semantic fit, or distinctiveness, block the iteration. Demand a return to `review` or `define`.
- **Missing lock:** If there is no signed `decision` record locking the current concept, refuse to iterate. Iteration requires a locked parent.
- **Frankenstein mash-up:** If the owner requests combining two rejected concepts, block and provide the objective rationale for why the mash-up fails the scorecard criteria.
- **Unsupported SVG features:** If the host's revision introduces unsupported SVG elements (e.g., `<filter>`, `<script>`, external `<image>`, `<text>`), native inspection will flag it as `unsupported_content`. Block export until the host resolves the structural validity.
- **Missing participant evidence:** If a shape change was made and the owner claims it passed perception tests without providing imported results, block approval. Unknowns stay unknown.

## Honesty rules
- Never invent organization facts, participant quotes, test results, or tool runs.
- Unknowns stay unknown. If a perception test hasn't been run, do not assume it passed.
- Agent recommendation is not owner approval. Never auto-approve a refinement or record an owner decision on their behalf.
- Never claim `production_ready`. The alpha only exports `draft`, `concept_only`, or `specification_only`.
- Do not fabricate canonical STATE commits, engine hashes, or native verification results.
- If a review is missing or stale, mark the scope as blocked; do not silently pass it.
- Do not invent causal analysis for test results. Record deterministic counts and gaps only.

## Exit criteria
The `iterate` route is complete when:
1. The refined candidate passes all hard checks (B/W works, 16px favicon works), and perception results are accepted by owner decision if re-tested.
2. The scorecard thresholds adopted for this scope are met—advisory course defaults (e.g., average ≥4.0, structural floors ≥4.0, no criterion below 3.0) may be used if the owner has adopted them; readiness is determined by the native gate plus a recorded owner decision.
3. The owner has signed a `decision` record explicitly approving the refined scope.
4. The concept lock remains intact (no strategy drift).
5. The final state passes `gate` without blockers.

## Fictional example
**Context:** The locked logo for "Ollion" (a civic tech platform) failed the 16px favicon test because the interlocking rings blurred into a solid blob.
**Triage:** Feedback is classified as Execution (scalability/accessibility).
**Invariants:** The interlocking ring metaphor (semantic fit) and the circular silhouette (distinctiveness) are protected.
**Patch Plan:** Increase the negative space between the rings by 1.5px and simplify the inner stroke weight. Refuse the owner's side request to add a gradient to make it "pop".
**Action:** The host designer applies the patch. The agent runs `logo inspect` and confirms XML validity and finite coordinates. The agent records the new revision.
**Re-test:** The host runs a 1-second flash test. Recall@1 is 52% and misattribution is 4%. The owner decides the results meet the scoped thresholds and approves the refinement.
**Decision:** The owner signs the refinement decision. The agent records it and verifies `gate` passes. The export is permitted as `draft`.

