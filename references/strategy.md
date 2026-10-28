# Strategy and Platform Definition (Route: define)

## Route Overview
**When to use:** Execute this route after discovery and landscape analysis are complete. Use it to distill raw evidence into a central brand idea and formalize it into a signed, one-page platform that governs all subsequent naming, voice, and visual identity work.
**Minimum inputs:** Current evidence records, landscape/competitor audit, and user target/brief from prior routes.
**Prerequisite records:** Approved baseline and discovery records. Draft exploration may proceed with labeled assumptions, but approved dependent work requires current approved prerequisite fields.
**Outputs:** Essence Draft (scored candidate ideas, attributes, positioning) and the Brand Platform One-Pager (15 fields) with Acceptance Criteria.

## Step-by-Step Procedure

### Phase 1: Distill the Essence
1. **Build the truth pile:** Extract verifiable facts, magnet quotes, and observed behaviors from the baseline records. Tag each as proven, believed, or aspirational. Ensure coverage across product, experience, price, channel, and purpose.
2. **Name the tension:** Identify the primary duality the brand must hold (e.g., clinical accuracy vs. human comfort).
3. **Draft the four positioning answers:** Define the core conviction, best-customer audience and value, key difference against a named alternative, and verifiable proof points.
4. **Generate and screen central ideas:** Produce candidates (advisory target: at least 20) across four forms (motto, concept, metaphor, resolved tension). Apply the seven-test battery as advisory course defaults:
   - *True (gate):* Builds on a verifiable heart truth.
   - *Compelling:* Matters to the target audience.
   - *Distinctive:* Avoids category conventions.
   - *10-year-old:* Easily understood by a child.
   - *Omnidirectional:* Demands something internally, promises something externally.
   - *Competitor swap (gate):* A top competitor would refuse to claim it.
   - *Generative:* Sparks distinct visual and verbal expressions.
5. **Shortlist and score:** Select top candidates (advisory target: three). Score them 1-5 on tests 2-5 and 7 as advisory metrics. Tests 1 and 6 are advisory pass/fail gates the owner may adopt for a scope; readiness is determined by the native gate command and a recorded owner decision, not automatic test passage.
6. **Define attributes and boundaries:** Select attributes (advisory target: 3-5) with custom definitions and "but not" guardrails. Mark the differentiator. Write a purpose (advisory target: max 15 words, no customers/profits/products), values with observable behaviors and a collision rule, three sacrifices, and core vs. playground lists.

### Phase 2: The One-Page Platform
Transfer the essence into the canonical 15-field platform. Apply word limits for each field as advisory course defaults.
- **A. The Idea:** 1. Central idea (with inside/outside readings), 2. Core truth.
- **B. Why We Exist:** 3. Purpose, 4. Ambition.
- **C. Where We Stand:** 5. Audiences (what to signal), 6. Promise, 7. Difference (named alternative), 8. Proof points.
- **D. How We Behave:** 9. Attributes (with differentiator), 10. Values in action (with collision rule).
- **E. Boundaries:** 11. Sacrifices, 12. Core and playground.
- **F. Handoff:** 13. Expression directives (meaning-based, no specific colors/fonts), 14. Architecture note, 15. Sign-off.
Derive an acceptance criteria list (advisory default: 7 rows) mapping each platform field to a verifiable review question.

## User Inquiry Batch
When requesting user input to resolve ambiguities in the essence or platform, ask **at most 3 focused questions** per batch:
1. What is the primary duality or tension your brand must hold in the market?
2. Which specific alternative or category frame are you most frequently compared against by your best customers?
3. What is one verifiable operational fact that proves your proposed central idea is already true today?

## Native Command Calls
Use these exact invocations to manage state and records. Do not invent subcommands.

- Retrieve discovery evidence:
  `node <skill-dir>/bin/distinct.mjs query --kind baseline --limit 20`
- Create a bounded task packet for strategy generation:
  `node <skill-dir>/bin/distinct.mjs packet --task define --scope essence,platform --max-bytes 16384`
- Store the drafted essence:
  `node <skill-dir>/bin/distinct.mjs record import --file essence-draft.json --expected-state <hash>`
- Store the 15-field platform draft:
  `node <skill-dir>/bin/distinct.mjs record import --file platform-draft.json --expected-state <hash>`
- Check readiness for owner decision:
  `node <skill-dir>/bin/distinct.mjs gate --scope platform`
- Record the owner's actual decision (agent cannot be the owner):
  `node <skill-dir>/bin/distinct.mjs decision record --file platform-decision.json --owner "Jane Doe"`, then ask the owner for the code shown on their terminal and run `decision confirm --id <pending_id> --code <code>`

## Host-Guided Steps and Fallbacks
- **Visual Expression Directives:** If a design tool is unavailable to generate mood boards for field 13, fallback to text-based descriptive directives (e.g., "authoritative → restrained palette, high contrast"). Do not claim a visual render was produced.
- **Internal Pressure Testing:** If you cannot run a live workshop with 5-10 cross-functional participants, fallback to a structured text-based review with the user. Explicitly label the output as `manual_pressure_test` and note the limitation in the record.
- **Archetype Mapping:** Keep archetype mapping in manual backing notes. Do not force an archetype name into the 15-field platform.

## Blockers and Honesty Rules
- **No invention:** Never invent organization facts, participants, quotes, test results, tool runs, or counts. If a verifiable proof point is unknown, mark it as `unknown` and block the platform sign-off.
- **Unknowns stay unknown:** Do not guess competitor reactions or audience sentiments. Use the user inquiry batch to gather actual data.
- **Agent recommendation != owner approval:** The agent can recommend a central idea and draft the platform, but it cannot sign field 15 or record a decision. A fixture owner is fictional; every simulated decision must be labeled `fixture-only`.
- **No production-ready claims:** This route produces a draft or concept-only kit. Never claim the platform is `production_ready` or that it confers legal/trademark clearance.
- **Blockers:** A missing check result for a planned check is a blocker. A required record whose privacy class is disallowed produces a `privacy_disallowed` blocker.

## Exit Criteria
The route is complete when:
1. The 15-field platform is drafted within word limits.
2. The central idea is evaluated against the True and Competitor-swap tests (advisory gates from course defaults); readiness is determined by the native gate command and a recorded owner decision, not automatic test passage.
3. An acceptance criteria list is attached.
4. The record is imported, and the `gate` command returns `ready_for_owner_decision`.
5. An actual owner instruction is recorded via `decision record`, upgrading the status to `approved_for_declared_scope`.

## Fictional Example
**Context:** Lumina Health, a B2B remote patient monitoring SaaS.
**Tension:** Clinical accuracy vs. human comfort.
**Central Idea:** "Care, Quantified"
- *Inside reading:* Demand relentless clinical precision in every data point.
- *Outside reading:* Promise patients they are safely and accurately watched over.
**Attributes:** Clinical (but not cold), Accessible (but not casual), Vigilant (but not anxious). *Differentiator: Vigilant.*
**Purpose:** To make healing invisible and inevitable. 
**Expression Directives:** Vigilant → unblinking, steady visual rhythms; Accessible → warm, human-centered typography.
**State:** Drafted via `record import`. Owner "Dr. Aris Thorne" signs via `decision record`.

