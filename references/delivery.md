# Route: deliver

## When to use
- When the approved platform, voice, and identity direction (or specification) are locked, and the user requests a provisional handoff kit, guidelines draft, or rollout plan.
- Used to generate a bounded, immutable-revision draft or provisional kit for external partners, internal teams, or production vendors.
- Used to transition the brand from internal strategy and design into structured handoff artifacts.
- Never used to certify production readiness, issue final legal/trademark clearance, or declare a brand fully launched to the public.
- In this alpha, the agent only supports `draft`, `concept_only`, or `specification_only` exports.

## Minimum inputs and prerequisite records
- Approved or explicitly accepted `platform` and `voice` records.
- Approved or accepted `direction` and `identity` (or `specification_only`) records.
- Completed or explicitly deferred `review` records showing what is missing (e.g., audience testing, trademark clearance, optical proofing).
- A specific `--scope` or snapshot hash to export, matching the current `STATE.json`.
- An explicit owner decision if any mandatory prerequisites are being intentionally deferred.

## Native commands for this route
Exact syntax: `references/commands.md`.
- `export --kind draft|concept_only|specification_only --out <dir> [--tokens dtcg]`.
  - `concept_only` labels every visual record as a concept.
  - `specification_only` carries no binary or proof payloads.
- `--tokens dtcg` writes `tokens.json` in W3C DTCG format plus `GUIDELINES.md`. Both use **owner-approved** fields only. Unapproved, stale or unsupported fields are listed in the manifest under `tokens_omitted`. Tools such as Style Dictionary can consume `tokens.json`.
- Run `gate` first and show the missing proofs, reviews and rights in the hand-off. Never `production_ready`.

## Step-by-step procedure

### 1. Confirm scope and missing dependencies
Before assembling the kit, verify the governance state and identify missing reviews, proofs, or tests. The agent must not silently hide gaps or coerce status into a pass.

**Agent action:** Run a readiness check against the current scope.
**Native command:**
`node <skill-dir>/bin/distinct.mjs gate --scope <s>`

**Agent action:** Inspect the JSON output for `missing_reviews`, `missing_proofs`, `blockers`, and `stale` records. If the user is accepting a provisional kit despite blockers, the agent must record the explicit owner decision to proceed with visible gaps.

**Questions to ask (max 3):**
1. "Which export kind do you need: `draft` (full provisional kit), `concept_only` (strategy and direction), or `specification_only` (technical handoff without visuals)?"
2. "Are there specific missing reviews, audience tests, or proofs you are choosing to defer for this provisional export?"
3. "Where should the exported kit be written (target directory)?"

### 2. Draft the Guidelines Structure (Host-guided)
Native automation does not write prose guidelines, design templates, or machine-readable token packs. The agent must prepare a bounded packet containing the anchors, range-of-control map, and reader matrix for the host or user to draft the actual guidelines.

**Agent action:** Generate a bounded task packet for guidelines assembly.
**Native command:**
`node <skill-dir>/bin/distinct.mjs packet --task deliver --scope platform,voice,direction,identity --destination local --allow public,confidential --max-bytes 16384`

**Host-guided fallback:** If the user needs a human-readable markdown draft of the guidelines table of contents and the three core templates structure, the agent uses the packet anchors to outline the contents checklist and template selection logic. The agent outputs a structured markdown file in the workspace, explicitly labeled as a draft requiring host optical and strategic refinement.

**Questions to ask (max 3):**
1. "Based on the approved platform, which high-volume artifacts should we structure as core templates for your team?"
2. "How do you want to map the range of control (tightest external vs. loosest internal) for your specific organization's outputs?"
3. "Who is the named owner for the living guidelines once exported?"

### 3. Draft the Launch and Governance Plan (Host-guided)
Native automation does not schedule real-world events, assign human roles, or capture baseline metrics. The agent prepares a rollout planning packet to structure the handoff.

**Agent action:** Generate a bounded task packet for launch planning.
**Native command:**
`node <skill-dir>/bin/distinct.mjs packet --task deliver --scope identity,voice --destination local --allow public,confidential --max-bytes 16384`

**Host-guided fallback:** The agent uses the packet to draft an asset inventory template, a ripple sequence for the reveal, and a governance RACI matrix. The agent asks the user to fill in the actual dates, named people, and baseline metrics. The output is a markdown template saved in the workspace.

**Questions to ask (max 3):**
1. "What are the critical Day 1 touchpoints that need to be prioritized for the external reveal?"
2. "Who will serve as the Decision Maker and Ambassador for ongoing brand governance?"
3. "What baseline metrics (e.g., internal pulse survey, customer sentiment) can be captured before the internal launch?"

### 4. Execute the Export
Once the scope is confirmed and provisional decisions are recorded, export the immutable-revision kit.

**Agent action:** Run the export command using the exact invocation form.
**Native command:**
`node <skill-dir>/bin/distinct.mjs export --kind draft --out <dir>`
*(Note: use `concept_only` or `specification_only` if selected in step 1).*

**Agent action:** Verify the export manifest and ensure no restricted or confidential records leaked into the output directory.
**Native command:**
`node <skill-dir>/bin/distinct.mjs status`

**Host-guided fallback:** If the export fails due to missing mandatory proofs that the user refuses to defer, the agent cannot produce the `draft` kit. The agent must downgrade to `specification_only` or halt and request the missing `record import` or `review import` before retrying.

## Record keeping during deliver
- The `deliver` phase does not create new strategic records. It only reads existing approved records and packages them.
- If the host-guided guidelines or launch plan templates are saved as files in the workspace, they are treated as unstructured host artifacts, not canonical STATE records.
- Any owner decisions to defer tests or accept provisional states must be imported via `decision import` or recorded via `decision record` before the export command is run.
- The export manifest will always reflect the exact `content_snapshot_hash` at the moment of execution. Subsequent changes to the brand records will require a new export.

## Blockers
- **Missing approved prerequisites:** Cannot export a `draft` kit if the `platform` or `direction` is still in `draft` status without an explicit owner decision accepting the draft state.
- **Production certification request:** If the user requests a `production_ready` export or claims the kit is legally/trademark cleared, the agent must refuse and output `concept_only` or `draft` with missing proofs visible. No production certification is available in this alpha.
- **Unresolved mandatory checks:** If a configured check plan row is blocked or unresolved, the `gate` command will flag it. Readiness is determined by the native gate result plus a recorded owner decision; the export cannot proceed without explicit owner acceptance of the gaps.
- **State hash mismatch:** If the `--scope` or snapshot hash does not match the current `STATE.json`, the command will fail. The agent must run `verify` and reconcile governance before exporting.
- **Path traversal / unsafe output:** If `--out <dir>` is outside the permitted workspace or attempts to overwrite without permission, the command will reject the path. The agent must ask for a safe, workspace-relative or explicitly permitted destination.
- **Budget overflow:** If the `packet` commands exceed the 16,384-byte default cap, the agent must either drop examples or request a split, never replacing required anchors with file pointers.

## Honesty rules
- Never invent organization facts, participant quotes, or test results for the guidelines or launch plan.
- Unknowns (e.g., trademark clearance, actual human adoption metrics, optical balance of templates) stay unknown and are listed in the export manifest as missing proofs.
- Agent recommendation is not owner approval. The agent can draft a rollout plan, but the owner must explicitly record the decision via `decision record`.
- No `production_ready` stamp. The kit is strictly provisional/draft.
- Never claim native optical verification of guidelines or templates; they are structural handoffs requiring human review.
- Do not fabricate canonical STATE commits or native verification hashes if falling back to manual markdown outlines.
- Preserve consent and quote rights across all derivatives; exclude any quote without explicit consent from the export manifest.

## Exit criteria
- A provisional kit (`draft`, `concept_only`, or `specification_only`) is successfully written to the user-selected directory.
- The `manifest.json` in the export directory accurately reflects the snapshot hash, included records, and explicitly lists missing tests, rights, or reviews.
- The user has a host-guided markdown outline for Guidelines (anchors, range, templates) and Launch (inventory, ripple sequence, governance roles).
- No false final assets or public launch claims are present in the exported records.
- All restricted and confidential records are excluded from the export directory.
- The agent exits with code 0 for utility completion, even if readiness remains blocked by deferred items, as domain gate status is a result field.

## Fictional example
**Context:** A B2B SaaS company, "Nimbus", has approved its platform and visual direction. The founder wants the handoff kit to give to an external design agency for final production and guidelines creation.

**Procedure:**
1. Agent runs `node <skill-dir>/bin/distinct.mjs gate --scope nimbus-core`. Output shows `missing_reviews: ["audience_test_v1"]` and `missing_proofs: ["trademark_clearance"]`.
2. Agent asks: "Which export kind do you need, and are you deferring the audience test and trademark clearance for this provisional export?" Founder replies: "Export `draft`, we will do the audience test and trademark search next quarter."
3. Agent records the deferral via `node <skill-dir>/bin/distinct.mjs decision record --file defer_audience.json --owner "Founder"`.
4. Agent generates a `deliver` packet to outline the Guidelines TOC and Launch Plan.
5. Agent runs `node <skill-dir>/bin/distinct.mjs export --kind draft --out ./handoff-nimbus-v1`.
6. Export succeeds. The `manifest.json` lists the approved platform, voice, and direction, but explicitly flags `audience_test_v1` and `trademark_clearance` as missing. The agent provides the markdown outlines for the agency to use as a foundation, clearly labeled as drafts requiring host refinement.

