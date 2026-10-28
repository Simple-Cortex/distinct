# Discovery and Landscape Routes

These routes extract the foundational truths, tensions, and market context required before defining the brand platform or visual direction. They rely on candid evidence and structured observation, not validation or assumption. 

## Global Honesty and Authority Rules

- **No fabrication:** Never invent organizational facts, participant quotes, test results, tool runs, or counts. Unknowns must remain explicitly labeled as unknowns.
- **Agent limits:** Agent recommendations are not owner approvals. A simulated or fixture decision is labeled fixture-only and never commits canonical state.
- **Draft status:** Outputs from these routes are `draft` records. They do not confer legal clearance, production readiness, or participant evidence.
- **Host guidance:** Web crawling, PDF reading, OCR, and actual visual inspection are host-guided. If host tools are unavailable, record the blocker and proceed with manual partial deliverables. Do not invent the output of an unavailable tool.

---

## Route: discover

### When to use
Execute `discover` after `start` and `ingest` are complete. The goal is to surface hidden truths, tensions, equity, and perception gaps through candid stakeholder and customer evidence. Discovery is a search for what the organization actually is, not what it aspires to be on a wall poster.

### Minimum inputs and prerequisites
- Completed `start` and `ingest` routes.
- Permissioned context and defined scope.
- Access to leadership, employees, and loyal customers (host-guided).

### Step-by-step procedure

1. **Prepare written homework (Host-guided)**
   Distribute a fixed-format questionnaire to the core team and decider 1-2 weeks before interviews. Require separate, individual answers to stress-test alignment. Do not accept a stack of old research as a substitute for fresh written answers.

2. **Conduct leadership 1:1 interviews (Host-guided)**
   Interview every veto-holder individually. Record and transcribe. Listen more than you speak. Ask about what actually happened, not what might happen. Push past aspirational language to find authentic convictions.
   
   *Ask at most 3 focused questions per batch to maintain depth and avoid fatigue.*
   
   **Batch 1: Origin and Conviction**
   - "What is the purpose of the company, and what are you fighting against?"
   - "Why did we start this company, and what have we lost sight of?"
   - "What do we prioritize when our core values collide?"

3. **Conduct loyal customer interviews (Host-guided)**
   As an advisory course default, consider interviewing up to six loyal customers who fit the future ideal profile; the owner may adopt a different sample size for the scope. Do not ask them to design the brand; ask about their actual behavior and the "magnet" truths that attract them.
   
   **Batch 2: Magnet Truths**
   - "What first drew you to the organization?"
   - "What would a world without our organization look like?"
   - "Tell me about the last time you used us, and what you actually did."

4. **Synthesize findings**
   Turn raw transcripts into a structured synthesis. Sort findings into three bins: what is working, what is outdated, and what is missing. Write tensions as "X and Y" pairs. Restate the core problem and compare it to the initial project hypothesis.

### Native command calls

To request a bounded task packet for synthesizing interview transcripts:
```text
node <skill-dir>/bin/distinct.mjs packet --task discover --scope truths,tensions,equity --workspace <project> --brand <id>
```

To store the resulting synthesis as a draft record:
```text
node <skill-dir>/bin/distinct.mjs record import --file discovery_synthesis.json --workspace <project> --brand <id>
```

To check readiness before moving to the next phase:
```text
node <skill-dir>/bin/distinct.mjs gate --scope discover --workspace <project> --brand <id>
```

### Host-guided steps and fallbacks
- **Interviews and transcription:** Must be performed by the host. 
- **Fallback:** If stakeholders refuse interviews or the host cannot access recording tools, record the blocker in the project state. Continue useful host-guided drafts in a separate manual output area using the same conceptual fields. Do not fabricate canonical state commits or infer answers from silence. Stop dependent decisions until the blocker is resolved.

### Blockers and honesty rules
- **Blocker:** Missing interviews with veto-holders.
- **Honesty:** Never invent participant quotes or organizational facts. If a tension cannot be proven by evidence, do not include it. 

### Exit criteria
As advisory course defaults for a complete draft, readiness is decided by the native gate plus a recorded owner decision:
- A draft Discovery Synthesis record exists containing top truths, tensions, equity to keep, and the restated problem.
- At least one tension is written as an "X and Y" pair specific enough that no competitor could claim it.
- Every truth has a direct quote or proof point attached.

### Fictional example
**Brand:** Helios Solar
**Process:** Leadership interviews revealed that while the company prides itself on rapid installation, customers are secretly terrified of roof leaks. 
**Output:** A draft synthesis record highlighting the tension: "Speed of deployment and care for the home." The restated problem shifts from "we need to look more modern" to "we need to prove our speed doesn't compromise structural integrity."


---

## Route: landscape

### When to use
Execute `landscape` after `discover` is complete. The goal is to map the competitive environment, identify category conventions, and find whitespace. This route separates strategic/verbal positioning from visual topology to ensure the brand can own a distinct space.

### Minimum inputs and prerequisites
- Draft or approved Discovery Synthesis.
- Defined scope, category, and market hypotheses.
- Host access to competitor websites, marketing materials, and product offerings.

### Step-by-step procedure

1. **Define the category and select competitors (Host-guided)**
   As an advisory course default, consider identifying 5-7 deep strategic competitors (for positioning, voice, and messaging) and approximately 20 shallow visual competitors (for visual topology); the owner may adopt different ranges for the scope. Do not obsess over every player; stop before brand paralysis sets in.

2. **Run the strategic and verbal audit (Host-guided)**
   Analyze the 5-7 deep competitors. Map their primary archetypes, promises, and messaging. Identify the "sameness" in one line (e.g., "sans serif, geometric type, loads of blue, lots of jargon"). 

3. **Run the visual topology audit (Host-guided)**
   Analyze the ~20 shallow competitors. Record the dominant shape families, color palettes (using HSL bins if possible), and metaphors. Look for the "lonely but fitting" visual combinations that the category ignores.

4. **Run the cover-the-logo test (Host-guided)**
   Conceptually place the client's current presence next to competitors. Cover the logos and ask:
   - *Look:* Are four out of five companies utilizing the same color palette?
   - *Sound:* Does anyone sound different, or do they all use the same jargon?
   - *Stand for:* Does anyone actually stand for something distinct?

5. **Synthesize the landscape**
   Document the category codes to keep (for legibility), the codes to break (to achieve differentiation), and the identified whitespace. Keep inspiration and competitor facts strictly separated.

### Native command calls

To store raw competitor evidence and screenshots provided by the host:
```text
node <skill-dir>/bin/distinct.mjs evidence import --source-id comp_audit_01 --file audit_manifest.json --workspace <project> --brand <id>
```

To store the structured landscape record:
```text
node <skill-dir>/bin/distinct.mjs record import --file landscape_record.json --workspace <project> --brand <id>
```

To query stored competitor metadata without opening full details:
```text
node <skill-dir>/bin/distinct.mjs query --kind competitor --limit 10 --workspace <project> --brand <id>
```

### Host-guided steps and fallbacks
- **Web crawling and visual scraping:** The native runtime does not crawl the web or parse PDFs. The host must gather competitor URLs, screenshots, and copy, then import them via `evidence import`.
- **Fallback:** If the host cannot access competitor sites or provide visual assets, manually input the 5-7 strategic competitors based on the client's provided list. Record the limitation. Do not invent competitor visual codes, features, or market positions.

### Blockers and honesty rules
- **Blocker:** Inability to define the primary category or identify at least 3 direct competitors.
- **Honesty:** Never invent competitor features, visual codes, or market positions. Do not claim trademark clearance or legal freedom to operate based on a landscape audit. 

### Exit criteria
As advisory course defaults for a complete draft, readiness is decided by the native gate plus a recorded owner decision:
- A draft Landscape record exists detailing both the strategic/verbal and visual topology audits.
- Category codes to keep and codes to break are explicitly listed with rationales.
- Whitespace is identified based on evidence, without making universal completeness claims about the entire market.

### Fictional example
**Brand:** Helios Solar
**Process:** The visual audit of 20 regional solar installers revealed a sea of bright green leaf motifs and aggressive red "sale" typography. The strategic audit showed everyone claiming to be the "most affordable" while hiding installation times in the footer.
**Output:** A draft landscape record defining codes to keep (clean lines, trust indicators) and codes to break (green leaves, red sales banners). The identified whitespace is "architectural minimalism," appealing to high-end homeowners who view solar as a home improvement rather than a utility hack.

