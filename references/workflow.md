# Workflow Routes: start, evolve

## Global Execution Rules

**Mode and Depth Selection**
Every engagement begins by declaring the mode and depth. These parameters bound the scope of all subsequent packets and gates.
- **Modes:** `create` (new brand), `refresh` (evolution of expression), `rebrand` (revolution of strategy and essence), `extend` (architecture and sub-brands), `review` (audit only).
- **Depths:** `essential` (minimum viable), `standard` (full system), `complex` (multi-brand or regulated environments).
Selection must be confirmed by the owner. If the user requests a mode that contradicts the diagnosed problem (e.g., requesting `refresh` when the core strategy is broken), the agent must flag the contradiction and require an explicit owner decision before proceeding.

**Headless and No-Answer Behavior**
If required inputs are missing and cannot be securely inferred from existing records, the agent must record a blocker or assumption in the governance log and halt dependent decisions.
- Never fabricate answers to fill gaps in the strategy or identity platform.
- Never interpret silence, missing data, or delayed feedback as owner approval.
- If a native tool is unavailable, do not fake a successful run or canonical STATE commit. Record the limitation in the user-facing result and proceed with manual fallbacks.

---

## Route: `start`

**When to use**
At project kickoff to establish the decision architecture, diagnose the real problem, and set the boundaries of the engagement before any strategy, naming, or design work begins.

**Minimum Inputs and Prerequisites**
- User target, brief, or raw source materials.
- No prerequisite records; this route initializes the workspace.

**Step-by-Step Procedure**
1. **Initialize Workspace:** Create the brand project snapshot with the declared mode and depth.
   ```bash
   node <skill-dir>/bin/distinct.mjs init --name <text> --mode <mode> --workspace <project> --brand <id>
   ```
2. **Establish Decision Architecture:** Ask the user to identify the single decision-maker, the project driver, and the core review team. Ensure the core team is kept to 3-5 people to prevent review latency.
3. **Diagnose the Real Problem:** The client's stated request is often a symptom. Ask up to 3 questions to translate the symptom into a testable hypothesis:
   - "What is the specific business friction you are experiencing right now, rather than just the visual fatigue?"
   - "If we solve the stated design problem but the business metrics do not improve, what would that tell us about the underlying positioning?"
   - "Are there any pending leadership changes, mergers, or funding rounds that might alter these goals in the next six months?"
4. **Define Scope and Cadence:** Establish the approval gates, feedback protocols (requiring one consolidated voice from the driver), and review latency math. 
5. **Persist the Charter:** Import the Project Charter record, which includes the decider, veto map, problem hypothesis, and gate calendar.
   ```bash
   node <skill-dir>/bin/distinct.mjs record import --file <charter.json> --workspace <project> --brand <id>
   ```
6. **Record Owner Authorization:** The owner must explicitly approve the charter. The agent cannot record this on behalf of the owner.
   ```bash
   node <skill-dir>/bin/distinct.mjs decision record --file <decision.json> --owner <name> --workspace <project> --brand <id>
   ```
   The owner reads the one-time code from their terminal and gives it to you; then run `decision confirm --id <pending_id> --code <code>`. Headless runs end with the pending decision listed as a blocker.

**Host-Guided Fallback**
If no strategy or document-generation tool is available to produce the formal charter, draft the charter as a plain markdown file in a designated manual output area (e.g., `manual/charters/`). Do not commit this to the canonical STATE engine. The next step is to manually import it via `record import` once native tools are restored. If a browser or PDF parser is needed to ingest initial sources but is unavailable, rely only on manually copied text provided by the user and record the ingestion limitation.

**Honesty Rules**
- Never invent the names of the decider, team members, or veto-holders.
- Never fabricate client quotes or assume a problem diagnosis without explicit evidence.
- Unknowns (e.g., pending mergers, unmeasured baselines) must remain unknown and logged as risks.
- An agent recommendation for a mode (e.g., suggesting `rebrand` over `refresh`) is not an owner decision. It requires explicit `decision record` authorization.

**Exit Criteria**
- Workspace initialized with correct mode and depth.
- Project Charter record imported and hashed.
- Owner decision recorded for the charter.
- Problem hypothesis is distinct from the initial symptom.
- Veto map includes booked interview dates or explicit "N/A" with a scoped reason.

**Fictional Example**
*NovaTech* requests a "new logo" because their current one looks dated (symptom). The agent probes and discovers the real problem is audience confusion following a recent merger of two product lines. The agent recommends mode `rebrand`. The agent drafts the charter, records the hypothesis, and requests the CEO to sign the decision. The agent does not proceed to the `discover` route until the CEO signs the charter and the veto map is populated.


---

## Route: `evolve`

**When to use**
Post-launch, when monitoring evidence indicates brand drift, market shifts, or when executing a scheduled governance review (e.g., 90-day or annual cadence). This route preserves earned equity while addressing new business friction.

**Minimum Inputs and Prerequisites**
- Current approved platform and identity records.
- New evidence (e.g., observation records, market shifts, adoption metrics).
- No new `init` is required; this operates on the existing brand state.

**Step-by-Step Procedure**
1. **Gather and Import Evidence:** Import any new observation records, survey results, or market data.
   ```bash
   node <skill-dir>/bin/distinct.mjs record import --file <evidence.json> --workspace <project> --brand <id>
   ```
2. **Run Evolve-or-Reset Triage:** Evaluate the evidence against the core idea across monitoring lenses (internal adoption, customer sentiment, sales alignment). Ask up to 3 questions to determine the necessary intervention:
   - "What is the specific business imperative driving this proposed change, and is it rooted in friction or just visual fatigue?"
   - "Which distinctive assets are still recognized and true, and must be preserved to protect existing equity?"
   - "Are we over-correcting based on a fad, or under-correcting by ignoring a fundamental market shift?"
3. **Classify the Intervention:** Determine if the change is evolutionary (preserving the core idea and tweaking the system) or revolutionary (resetting the strategy). If revolutionary, the agent should recommend transitioning to a `rebrand` or `refresh` mode—this is advisory, not an automatic mode change; the owner must decide and authorize the new mode via the `start` route.
4. **Persist the Diagnosis:** Record the preserve/change plan and update the governance calendar.
   ```bash
   node <skill-dir>/bin/distinct.mjs record import --file <evolve-diagnosis.json> --workspace <project> --brand <id>
   ```
5. **Generate Bounded Packet:** If host-guided design iteration is needed, emit a bounded packet for the visual updates. Ensure no confidential records are leaked and the byte limit is respected.
   ```bash
   node <skill-dir>/bin/distinct.mjs packet --task evolve --scope platform,identity --workspace <project> --brand <id> --max-bytes 16384
   ```

**Host-Guided Fallback**
If analytics, survey, or rendering tools are unavailable, rely strictly on manually imported observation records (e.g., a user pasting a CSV of survey results or manually describing optical flaws). Do not fabricate metrics, conversion rates, or tool runs to fill the gap. If optical verification of the evolved assets is required but the native SVG renderer is unavailable, flag `native_proof: unavailable` in the governance log and require host-guided visual inspection in a suitable editor.

**Honesty Rules**
- Never invent survey results, customer sentiment scores, or conversion metrics.
- Never assume a brand is failing due to "visual fatigue" without evidence of actual business friction.
- Unknowns (e.g., lack of baseline metrics from launch) must be recorded as blockers to the evolve decision.
- No `production_ready` certification is ever issued; evolved assets remain `draft` or `concept_only` until explicitly approved by the owner.

**Exit Criteria**
- Evidence records imported and hashed.
- Triage completed with a clear preserve/change diagnosis.
- Owner decision recorded for the evolve path.
- Governance cadence updated if the intervention shifts the review timeline.

**Fictional Example**
*Lumina*, a B2B SaaS, notices a slight dip in external perception but high internal adoption. The agent imports the pulse survey data. The triage questions reveal the core idea is still strong, but the visual system feels inconsistent across new product UI. The agent classifies this as an evolutionary tweak to the identity system, preserving the core mark and voice. The agent records the diagnosis, updates the 90-day review calendar, and emits a bounded packet for the design team to adjust the UI color palette, ensuring no confidential strategy records are exposed in the packet.

