# References: Visual Routes

## Route: direct

**When to use**
Translating the approved platform (essence, attributes, voice) into concrete visual criteria, category mapping, and a Creative/Visual Brief. This route bridges abstract strategy to drawable form, ensuring every subsequent sketch starts from the organization's truth rather than subjective taste.

**Minimum inputs and prerequisite records**
- Approved platform record (essence, attributes with guardrails, tensions, positioning).
- Approved voice record (draft or approved).
- Approved name (if applicable and required by scope).
- Explicit scope ID.

**Procedure**
1. **Extract constraints:** Query the approved platform and voice records. Pull the central idea, 3-5 attributes with "but not" guardrails, tensions, and expression directives.
2. **Host-guided noun list generation:** Draft 15-25 concrete nouns (advisory target) grouped by category (People, Nature, Places, Objects, Letters). Present to the owner for elimination.
3. **Define metaphor and expressions:** Complete the sentence "The brand is like..." and list 3-6 drawable expressions (advisory target) of the central idea.
4. **Convert attributes to visual criteria:** Translate abstract words into visual properties (e.g., "straight lines, focal points, momentum") and create an avoid list of category clichés.
5. **Map the category:** Identify where competitors cluster on mark type, color, and metaphor. Name 1-3 open spaces (advisory target).
6. **Inventory owned assets (if refresh/rebrand):** Categorize existing elements as keep, evolve, or drop with explicit reasons.
7. **Draft the Visual Brief:** Compile inputs, nouns, metaphor, criteria, category map, and owned assets into a single `visual_brief` record.
8. **Request owner decision:** Ask the owner to review and sign off on the Visual Brief.

**Discovery questions (max 3 per batch)**
- "What concrete objects, places, or actions represent your central idea without using industry jargon?"
- "Which visual traits or competitor clichés must we absolutely avoid to remain distinct?"
- "If this brand were a physical structure or natural phenomenon, what would it be and why?"

**Artifact fields: `visual_brief.json`**
- `inputs`: central idea, attributes, tensions, expression directives.
- `noun_list`: categorized nouns, eliminated items with reasons, final 10-15 approved nouns (advisory).
- `metaphor`: single sentence and 3-6 expressions (advisory target).
- `design_criteria`: visual properties in tiers (primary, secondary, tertiary) and an avoid list.
- `category_map`: competitor clusters and named open spaces.
- `owned_assets`: keep/evolve/drop decisions with reasons.
- `sign_off`: owner name and date.

**Host-guided steps & fallback**
Noun lists, visual sandboxes, and mood boards require human curation or external design tools.
*Fallback:* If image generation or curation tools are unavailable, produce a text-based Visual Brief detailing the noun list, metaphor, typographic directions, color palettes, and category gaps. The owner must review the text criteria and record a decision before proceeding. Use `node <skill-dir>/bin/distinct.mjs record import --file visual-brief.json` to persist the text-based artifact.

**Native commands**
```bash
node <skill-dir>/bin/distinct.mjs query --kind platform
node <skill-dir>/bin/distinct.mjs query --kind voice
node <skill-dir>/bin/distinct.mjs record import --file visual-brief.json
node <skill-dir>/bin/distinct.mjs decision record --file decision.json --owner <name>
```

**Blockers**
- Platform essence or attributes are not approved.
- Name is required by scope but not approved.
- Owner refuses to eliminate nouns or provide constraints.

**Honesty rules**
- Never invent competitor logos or market data; if category mapping is unverified, label it as a host-guided assumption.
- Do not claim visual equivalence or optical balance without actual visual proof.
- Agent recommendations are not owner approvals; wait for explicit `decision record`.
- Unknowns stay unknown; do not fabricate audience reactions to noun lists.
- Do not force an emotional purpose, archaic archetype exercise, or numeric score threshold on every organization.

**Exit criteria**
- `visual_brief` record imported successfully.
- Owner decision recorded for the current scope.

**Example**
A B2B logistics platform needs to convey "precision and momentum." The agent extracts these attributes, proposes nouns like "compass," "trajectory," and "caliper." The owner eliminates "compass" as a cliché. The metaphor becomes "The brand is like a high-velocity rail network." Visual criteria are set to "diagonal momentum, monoline strokes, avoid generic globes." The Visual Brief is recorded and approved.


---

## Native commands for visual work
Exact syntax: `references/commands.md`.
- `logo inspect --file <svg>`: structural checks only (validity, viewBox, unsupported features).
- `logo proof --file <svg> --profile <file> --out <dir>`: real PNG renders at the profile's sizes and backgrounds, in an isolated worker with a memory cap, a 10 s timeout and an OS sandbox where available (`os_sandbox` is `seatbelt`, `host_inherited` when the host's own macOS sandbox refused nesting, or `none`). It needs the latest inspect verdict to be `inspected`. The raster results (`min_feature_px`, contrast) are advisory and never a pass.
- `color check --pairs <file>`: WCAG 2.x ratio, advisory APCA and colour-vision *simulation* for colour pairs. Advisory only.
- `logo review-packet`: freezes proofs and findings for a human or host visual review, with no verdict.
- Optical balance, curve quality, kerning and spelling in outlined marks remain host or owner visual judgments, made by looking at the renders.

## Route: identity

**When to use**
Generating, inspecting, and systematizing the mark and broader identity (typography, color, imagery, motion) based on the approved Visual Brief. Includes structural SVG inspection, intent capture, and proof planning.

**Minimum inputs and prerequisite records**
- Approved `visual_brief` record.
- Approved platform and voice records.
- Explicit scope ID.

**Procedure**
1. **Ideation & Sketching (host-guided):** Generate 30-50 rough thumbnails (advisory target) across different nouns and combinations.
2. **Shortlist & Vectorization (host-guided):** Select 3-15 distinct ideas (advisory target), vectorize with minimal anchor points, and refine optically.
3. **Capture Logo Intent:** Record the approved concept, defining features, intended construction, protected geometry, and target media into a `logo_intent` record.
4. **Structural Inspection:** Run native SVG inspection on the candidate file to verify XML validity, viewBox, finite coordinates, and element counts.
5. **Proof Plan & Generation (host-guided):** Plan proofs for whole mark, small sizes (16px, 24px) (advisory defaults), reversals, and real applications. Generate or import actual renders.
6. **Identity System Spec:** Define the DNA card (mark traits), typography, color layers, imagery categories, and iconography rules. Document fixed vs. flex points.
7. **Record Findings & Reviews:** Import structural findings and host-guided optical reviews.
8. **Request owner decision:** Ask the owner to review the mark candidate and system spec.

**Discovery questions (max 3 per batch)**
- "What specific geometric traits or construction rules define the chosen mark's DNA?"
- "Which elements of the identity system must remain fixed for decades, and which can flex by context?"
- "What are the minimum required sizes and reverse-mode behaviors for the primary mark?"

**Artifact fields: `logo_intent.json`**
- `concept`: approved concept and defining features.
- `construction`: intended geometry (e.g., geometric, organic, typographic).
- `relationships`: required repeated components, axes, and intervals.
- `protected_geometry`: approved asymmetries, overshoot, and negative-space features.
- `target_media`: minimum sizes, light/dark behaviors, responsive variants.
- `tolerances`: advisory construction grid (if defined) and physical production limits.

**Artifact fields: `system_spec.json`**
- `dna_card`: 5-8 measurable traits of the mark (advisory target).
- `typography`: logotype, display, text, and UI font selections with pairing rules.
- `color_system`: primary, secondary, functional/UI, and dark-mode values with contrast results.
- `imagery`: photography categories and illustration rules.
- `iconography`: derivation rules from the mark's grid and stroke (if applicable).
- `fixed_vs_flex`: matrix defining what is rigid and what is flexible.

**Host-guided steps & fallback**
Sketching, vector editing, and optical refinement require external design tools.
*Fallback:* If design tools are unavailable, produce a detailed text-based System Spec and construction rules (e.g., "a dot that's two widths, negative space ring that's one width"). Import host-supplied SVGs via `evidence import` and inspect them natively. Use `node <skill-dir>/bin/distinct.mjs record import --file system-spec.json` to persist the text-based artifact.

**Native commands**
```bash
node <skill-dir>/bin/distinct.mjs record import --file logo-intent.json
node <skill-dir>/bin/distinct.mjs logo inspect --file <logo.svg>
node <skill-dir>/bin/distinct.mjs evidence import --source-id <id> --file <path>
node <skill-dir>/bin/distinct.mjs record import --file system-spec.json
node <skill-dir>/bin/distinct.mjs check result record --file inspection-result.json
node <skill-dir>/bin/distinct.mjs decision record --file decision.json --owner <name>
```

**Blockers**
- Visual Brief not approved.
- SVG inspection fails XML validity or contains unsupported elements (e.g., scripts, external resources) without explicit owner acceptance.
- Logo intent not recorded before inspection.

**Honesty rules**
- Native `logo inspect` is structural, not optical. Never claim a mark is "visually balanced" or "craft-resolved" based solely on node counts or XML validity.
- Do not auto-symmetrize or force uniform radii; organic asymmetry and intentional overshoot are valid.
- Do not invent `production_ready` status; this alpha only exports `draft`, `concept_only`, or `specification_only`.
- Unknown relationships in intent remain unknown; do not auto-correct or guess intended symmetry.
- A model or parser result never sets an owner decision.
- Never silently drop unsupported SVG features and inspect the simplified result as if it were the original.

**Exit criteria**
- Mark candidate structurally inspected.
- `system_spec` record drafted and imported.
- Owner decision recorded for the current scope.

**Example**
For a chocolate brand, the agent captures intent for a mark combining six cocoa pods and a star of life into a flower shape. The SVG is inspected natively, confirming valid geometry and no unsupported filters. The host generates proofs at 24px and on a dark background, revealing a closed counter. The agent records the optical finding, proposes a localized patch, and drafts the System Spec with a custom serif wordmark. The owner approves the concept for refinement.

