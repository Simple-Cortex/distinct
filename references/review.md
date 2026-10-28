# Review, Evaluate, and Decide

## When to use
Use this route when draft concepts (idea stage) or systems (system stage) are ready for evaluation. This route transforms subjective preference into structured, platform-aligned decisions. It governs the presentation, scoring, perception testing, and final locking of the visual identity.

## Minimum inputs and prerequisite records
- Approved Brand Platform (advisory gate reference: owner may adopt a Gate 3 equivalent; readiness determined by native gate plus recorded owner decision).
- Locked Verbal Identity (advisory gate reference: owner may adopt a Gate 4 equivalent; readiness determined by native gate plus recorded owner decision).
- Drafted logo concepts or identity systems (from `identity` route).
- Defined acceptance criteria and perception test protocols.

## Native commands for this route
Exact syntax: `references/commands.md`.
- `review-packet --scope <id> --out <file>`: freezes the inputs to review, with the privacy filter applied and builder rationale stripped.
- `logo review-packet` for marks: proofs plus inspection findings, with no verdict.
- `review create` stores review evidence. Only the owner approves, through `decision record`.
- Test bench: `test plan` (frozen protocol), `test packet` (neutral stimuli), `test import` (actual observations only), `test summarize` (counts and gaps, with no causal claims). Never invent participants or results.

## Step-by-step procedure

### 1. Prepare bounded review packets
Freeze the current state and generate bounded packets for reviewers. Do not include builder persuasion or unapproved strategic shifts.

```bash
node <skill-dir>/bin/distinct.mjs packet --task review --scope concept,system --brand <id> --workspace <project>
```

### 2. Present and collect private feedback
Present the work in two stages. 
- **Idea stage:** Show marks in black and white only, labeled with their core nouns or rationale. No color, typography, or mockups. 
- **System stage:** Show the top 2-3 finalists in context (applications, color, typography).

Ask decision-makers to review privately before group discussion. Limit your discovery questions to a maximum of three per batch to avoid leading the room:
1. "Does this concept express the central idea and attributes defined in our platform?"
2. "What specific problems or tensions do you see in this execution?"
3. "Can you picture this working across our primary touchpoints without losing its meaning?"

Collect a private top-three vote from each decision-maker before any discussion begins. Instruct them to sleep on the decision for 24 hours before the final call.

### 3. Evaluate with the Logo & Identity Scorecard
Have 1-4 independent scorers evaluate the finalists using the nine-criterion scorecard. The designer's scores are kept in a separate column and excluded from the average. 

**Criteria:** Semantic fit, Distinctiveness, Simplicity and recall, Scalability, Accessibility, Craft, System potential, Longevity, Protectability.
**Scale:** 1 (fails) to 5 (excellent).

**Structural floor (advisory default):** Semantic fit, Distinctiveness, and Simplicity are typically expected to score ≥ 4 for a concept to proceed to refinement; the owner may adopt different thresholds for a specific scope, with readiness determined by the native gate plus a recorded owner decision.
**Ship threshold (advisory default):** An overall average of ≥ 4.0 with no single criterion below 3.0 is a common target; the owner may adopt different thresholds for a specific scope, with readiness determined by the native gate plus a recorded owner decision.

Record the raw scorecards as evidence, then create the formal review record.

```bash
node <skill-dir>/bin/distinct.mjs evidence import --source-id scorecard-round1 --file <path/to/scorecards.json>
node <skill-dir>/bin/distinct.mjs review create --file <review-record.json>
```

### 4. Run perception tests (Brand Test Bench)
Test the *executions*, never the *direction*. Use target-audience members who were not involved in the process. 

- **One-second flash:** Show the mark for one second. Ask for two adjectives and category attribution. 
- **Seven-second concept poll:** Ask which adjectives come to mind and if they see the metaphor.
- **Negative-reading sweep:** Code open-ended "reminds me of" answers. Any negative reading that repeats stops that direction.
- **Context and scale:** Test at 16px favicon, email headers, and life-size print.

**Pass thresholds (advisory defaults):** Typical targets include Recall@1 ≥ 50%, misattribution < 10%, target-trait score > 0.5 above category median. The owner may adopt different thresholds for a specific scope; readiness is determined by the native gate plus a recorded owner decision.

Import the actual observation data. If a test fails, do not fabricate success; record the failure and adjust the execution.

```bash
node <skill-dir>/bin/distinct.mjs evidence import --source-id perception-test-01 --file <path/to/test-observations.json>
node <skill-dir>/bin/distinct.mjs record import --file <test-summary-record.json>
```

### 5. Owner decision and concept lock
The named decision-maker (not the agent) reviews the scorecard averages, test results, and private tallies. The agent may transcribe the instruction, but the agent cannot act as the owner. 

Once the decision-maker chooses, write the Decision Record. The record must include the rationale in platform language (quoting attributes and central idea), the rejected options, and the lock statement. 

**Lock rule:** Refinements may improve the chosen concept but may not swap it. 

```bash
node <skill-dir>/bin/distinct.mjs decision record --file <decision-record.json> --owner <name>
# then, with the code the owner reads from their terminal:
node <skill-dir>/bin/distinct.mjs decision confirm --id <pending_id> --code <code>
```

### 6. Verify readiness
Check that all required reviews, test results, and approvals are recorded and fresh.

```bash
node <skill-dir>/bin/distinct.mjs gate --scope concept
```

## Host-guided fallbacks
- **No rendering/design tool:** If `logo proof` reports unavailable (for example `os_sandbox_failed`), or the asset is not SVG, provide detailed structural specifications (clear space, minimum sizes, color codes) and explicitly label the artifact as `specification_only`. Do not claim a native render ran; no visual verdict is possible without an actual render.
- **No testing platform:** If external perception testing tools are unavailable, conduct manual unaided comprehension tests with 3-5 unfamiliar individuals. Record their raw responses verbatim. Note the limitation of the small sample size in the review record.
- **No owner available:** If the decision-maker is unreachable, record the blocker and halt. Do not proceed with assumptions or simulated approvals.

## Blockers
- **Missing platform:** Cannot evaluate concepts without an approved Brand Platform and verbal lock.
- **Missing tests:** A missing check result for a planned perception test is a blocker. It is never a pass.
- **Structural floor failure (advisory):** If Semantic fit, Distinctiveness, or Simplicity scores below the adopted threshold, the concept may be blocked for refinement; the owner may accept a lower score with documented rationale.
- **Repeated negative readings:** If a negative perception test reading occurs more than once, that direction is blocked.
- **Stale dependencies:** If the platform or verbal identity was updated after the concepts were drafted, the review is blocked until concepts are updated.

## Honesty rules
- **Never invent facts:** Do not invent organization history, participant quotes, test results, tool runs, or sample sizes. 
- **Unknowns stay unknown:** If a test is inconclusive or skipped, report it as `not_run` or `inconclusive`. Do not sample until a favorite wins.
- **Agent ≠ Owner:** The agent's recommendation is not owner approval. A fixture owner is fictional; every simulated decision must be labeled `fixture-only`. The agent may record an actual owner instruction, but local files cannot cryptographically prove a human source was not forged.
- **No production readiness:** This alpha exports `draft`, `concept_only`, or `specification_only` kits. Never claim `production_ready` status, legal trademark clearance, or universal quality certification.
- **Test execution, not direction:** Perception tests evaluate whether the chosen execution delivers on the approved idea. Never use testing to crowd-source the core brand direction.

## Exit criteria
- The chosen option meets the adopted scoring thresholds (advisory defaults: average ≥ 4.0, no criterion below 3.0), with owner decision recorded.
- The structural floor (advisory defaults: Semantic, Distinctiveness, Simplicity ≥ 4) is met or owner has accepted variance with documented rationale.
- All hard checks pass (B/W/one-color versions work, 16px favicon works, adopted perception thresholds met, no repeated negative readings).
- The rationale is written in platform language, avoiding taste-based preferences.
- The concept is locked in writing via a signed `decision record`.
- `gate --scope concept` returns `ready_for_owner_decision` or `approved_for_declared_scope`.

## Fictional example
**Context:** "Lumina Health" is refreshing its identity. The platform emphasizes "calm authority" and "accessible expertise."
**Procedure:** The agent generates a `review` packet containing three black-and-white concepts. The decision-maker and two board members score them privately. Concept B (a stylized, minimal lotus) scores 4.5 on Semantic fit but 2.0 on Distinctiveness (too similar to a wellness competitor). Concept C (an abstract, interlocking 'L' and cross) scores 4.2 on Semantic fit, 4.5 on Distinctiveness, and 4.0 on Simplicity. 
**Testing:** A 1-second flash test with 30 target users shows Concept C achieves 60% recall and 5% misattribution. 
**Decision:** The agent records the scorecard evidence and imports the test results. The CEO signs the `decision record`, selecting Concept C. The agent locks Concept C and blocks any requests to mash it up with Concept B's lotus shape. The `gate` command confirms the concept is approved for system refinement.

