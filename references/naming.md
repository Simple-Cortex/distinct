# Route: name

## When to use
Initiate the `name` route when the organization requires a new brand name, or when the existing name fundamentally blocks the newly approved strategy and positioning. Naming must occur after the core platform (central idea, attributes, and positioning) is defined, but strictly before visual identity work begins. Words constrain form; a name that fails legal or strategic screening will invalidate downstream visual work.

## Prerequisites and inputs
- **Approved platform:** Central idea (field 1), positioning core (fields 5-7), and brand attributes (field 9). If the platform is only in draft, the naming output remains a draft and cannot be used to initiate the `direct` or `identity` routes.
- **Naming need confirmation:** Explicit agreement from the owner that a rename is necessary, or a mandate to formally evaluate the current name against the new strategy.
- **Decider authority:** Identification of the human owner who holds final approval authority for the name.

## Honesty rules and guardrails
- **No legal claims:** Never claim, imply, or simulate trademark clearance, domain availability, or linguistic disaster checks. These are strictly host-guided and require external legal/domain tools.
- **No invented participants:** Do not fabricate focus groups, cold-read participants, customer quotes, or test results. Where audience input is required, it must be actual owner-supplied evidence; the agent does not invent counts or outcomes.
- **No model-as-owner:** Agent recommendations, scores, or preferences are never owner approvals. The agent only transcribes and records actual human instructions.
- **No visual shortcuts:** Do not generate visual logos or renderings for name candidates. Use text-based mockups and structural descriptions only.
- **No production stamp:** Never export a `production_ready` kit or claim certification. Naming in this alpha results in a `draft` or `concept_only` kit pending external legal clearance.

## Step-by-step procedure

### Step 1: Confirm need and draft the Naming Brief
Before generating any names, establish the strategic boundaries. If the criteria are unclear, ask the owner a maximum of three focused questions:
1. What specific strategic problem does the current name cause, or what specific conceptual territory must the new name occupy?
2. What are the absolute boundaries for the name (e.g., must retain legacy letters, must be a single word, must avoid specific industry jargon)?
3. Who holds the final decision authority, and what is the timeline for external legal screening?

Construct the Naming Brief using the 13 required elements:
1. Description of what is being named.
2. Conceptual territories to convey.
3. Name criteria (strategic fit, ease of spelling, global neutrality).
4. Name tonality (e.g., sophisticated vs. playful).
5. Themes/paths to explore.
6. Themes/paths to avoid.
7. Target audience(s).
8. Competitor and peer names.
9. Name likes/dislikes from the owner and why.
10. Domain requirements.
11. Trademark screening criteria.
12. Linguistic and cultural disaster check criteria.
13. Background materials and source references.

**Native command:**
```text
node <skill-dir>/bin/distinct.mjs record import --file <brief_record.json> --expected-state <hash>
```
*The record kind must be `naming_brief`. The brief must trace directly to the approved platform attributes.*

### Step 2: Generate concepts by pathway
Generate name candidates organized by distinct conceptual pathways (advisory course default: 100-200 ideas; owner may adopt a different scope based on project needs). Do not mix pathways in the initial presentation.
- **Descriptive:** Literal interpretation of the offering.
- **Suggestive:** Metaphorical or evocative of the benefit.
- **Abstract:** Empty vessels filled with brand meaning.
- **Compound:** Two existing words merged.
- **Coined:** Invented words or intentional misspellings.

**Native command:**
```text
node <skill-dir>/bin/distinct.mjs packet --task name --scope brief,platform,attributes --max-bytes 32768
```
*Use the packet to retrieve the strict context needed to generate the candidates without hallucinating platform details.*

### Step 3: Prescreen and silent nomination
Filter the generated list against the basic criteria in the Naming Brief (e.g., easy to spell, no obvious negative meanings in primary languages). 
Perform a silent nomination: select a shortlist of strong candidates across different pathways (advisory course default: 8-10; owner may adopt a different scope based on project needs). Do not rank them by agent preference. Present them neutrally to prevent groupthink or anchoring bias. Each candidate must include:
- The name.
- The conceptual pathway.
- A one-sentence strategic rationale tracing to the platform.
- A simple text-based mockup context (e.g., "Used as a email sign-off").

### Step 3a: Host-run availability screening

The host agent (not the native CLI, which makes no network calls) performs preliminary availability checks.

**Domain screening:** Fetch `https://data.iana.org/rdap/dns.json` to locate the TLD's RDAP server. Query `<server>domain/<name.tld>`. Interpret an HTTP 404 strictly as 'possibly unregistered'; never declare a domain 'available'. Enforce a maximum of 20 lookups per batch to respect rate limits.

**Trademark screening:** Provide the owner with manual search links for USPTO (`https://tmsearch.uspto.gov`), EUIPO eSearch (`https://euipo.europa.eu/eSearch`), and WIPO Global Brand Database (`https://branddb.wipo.int`). Supply the candidate names and relevant Nice classes alongside a manual checklist. Never claim clearance or simulate legal approval; explicitly recommend consulting qualified trademark counsel.

**Record results:** Save all raw RDAP responses and owner screening notes as files. Import the evidence using the following command:
```text
node <skill-dir>/bin/distinct.mjs evidence import --workspace <project> --brand <id> --manifest <file> --source-root <dir>
```
Set the privacy class to `public`. Reference these files directly from the candidate record.

**Fallback:** If no network tool is available, produce the checklist and links only, and record a `blocker` for the missing network capability.

### Step 4: Round 1 presentation and learning
Present the shortlist to the owner. The goal of Round 1 is learning, not final selection. Ask for feedback on which conceptual pathways resonate and which feel misaligned. 
Record the owner's feedback, noting any shifts in the strategic criteria. If the owner defaults to "I'll know it when I see it" or demands a company-wide vote, pause and request a return to the platform attributes to resolve the mission clarity gap.

**Native command:**
```text
node <skill-dir>/bin/distinct.mjs record import --file <round1_feedback.json> --expected-state <hash>
```
*Record kind: `review_evidence`. Mark as owner-provided feedback.*

### Step 5: Refine, stress test, and Round 2
Based on Round 1 feedback, refine the shortlist to finalists (advisory course default: 2-3; owner may adopt a different scope based on project needs). 
Instruct the owner to perform the stress test in the real world:
- Speak the name aloud on a phone call.
- Visualize it on a t-shirt, an email header, and a physical sign.
- Check for tongue twisters or unintended meanings.

Instruct the owner to perform a cold read: show the finalists without context to trusted colleagues outside the project and record their immediate, unguided reactions. The agent does not invent participant counts or summarize nonexistent feedback.

**Native command:**
```text
node <skill-dir>/bin/distinct.mjs review create --file <stress_test_record.json>
```
*Record the structural stress test observations and any owner-supplied cold read results. Do not fabricate data.*

### Step 6: Final selection and decision recording
Once the owner selects the final name, record the decision. The agent cannot select the name. The decision record must explicitly name the human owner and reference the exact scope of the approved name.

**Native command:**
```text
node <skill-dir>/bin/distinct.mjs decision record --file <final_name_decision.json> --owner <owner_name>
```
*The agent must refuse to record this if the `--owner` is `agent` or `model`.*

## Host-guided steps and fallbacks
- **Domain and Trademark Checks:** Run Step 3a with the host's own network tools; the native CLI makes no network calls. If the host has none, fall back: Provide the owner with the exact list of finalists and a checklist of required domain extensions (e.g., .com, .io) and trademark classes. The owner must perform these checks externally.
- **Visual Mockups:** If design tools are unavailable, the agent cannot render logos. Fallback: Provide detailed text descriptions of how the name will function typographically (e.g., "The lowercase 'g' provides a natural anchor for the baseline") and rely on the owner to test this in their preferred word processor or design software. No visual verdict is issued without an actual render.
- **Linguistic Checks:** The agent cannot guarantee global linguistic safety. Fallback: Explicitly list the target international markets in the export kit and instruct the owner to consult a native speaker or linguistic screening service before finalizing.

## Blockers
- **Missing platform:** If the central idea and attributes are not approved, the naming brief cannot be validated. Halt and return to `define`.
- **Scope creep:** If the owner attempts to approve visual identity elements (colors, logo shapes) before the name is legally cleared, block the `identity` route.
- **Unresolved criteria:** If the owner cannot articulate what makes a name "good" beyond subjective taste, block generation until the Naming Brief criteria are explicitly documented.
- **Privacy disallowed:** If the Naming Brief contains confidential market strategies and the packet destination does not permit confidential records, block the packet generation.

## Exit criteria
The `name` route is complete when:
1. A Naming Brief is recorded and approved.
2. At least one name candidate has passed the owner's external legal and domain prescreen (owner-supplied evidence; agent does not fabricate clearance).
3. A final name is recorded via `decision record` with explicit owner authorization.
4. The candidate history and stress-test observations are stored as immutable records.
5. The state is ready to advance to the `direct` or `identity` routes.

Readiness is determined by the native gate state plus a recorded owner decision, not by advisory course defaults. No `production_ready` or certification claim is issued.

## Fictional example
**Context:** A B2B supply chain software company, "FreightFlow," is rebranding to reflect a shift from simple tracking to predictive logistics. 
**Procedure:** The agent drafts a Naming Brief focusing on concepts of "foresight," "fluidity," and "foundation." The agent generates candidates across 5 pathways (advisory default). 
**Round 1:** The agent presents a shortlist. The owner rejects the "coined" pathway (e.g., "Freightix") as too generic, but responds positively to the "abstract" pathway (e.g., "Vellum"). 
**Round 2:** The agent refines to finalists (advisory default: 2-3). The owner conducts a cold read with their board (owner-supplied evidence). "Plumb" is rejected because it sounds like plumbing. "Fathom" is rejected because it implies depth rather than forward motion. 
**Decision:** The owner selects "Vellum" and records the decision. The agent provides the external checklist for the owner to verify "vellum.com" availability and USPTO Class 9 trademark status before proceeding to visual identity. The kit exports as `draft` pending legal clearance.

