# Route: voice

## When to use
Activate this route after the brand platform (essence, attributes, positioning) is drafted or approved. Verbal identity defines how the brand sounds, what it says, and in what order. It locks the constraints for visual work and ensures messaging remains consistent across channels. If the platform is only drafted, label all verbal outputs as `draft`.

## Minimum inputs and prerequisites
- **Required:** A platform record containing at least the central idea, positioning, and brand attributes (field 9).
- **Prerequisite state:** The platform must exist in the workspace. If it is not approved, the output voice and messaging records will inherit a `draft` status.
- **Context:** Access to existing brand messaging samples (if available) for auditing.

## Honesty rules and blockers
- **No fabrication:** Never invent organizational facts, customer quotes, test results, or competitor copy. If existing samples are unavailable, ask the user to provide them or proceed with category assumptions explicitly labeled as such.
- **No false approval:** Agent recommendations are not owner approvals. Do not mark voice pillars or messaging as approved unless a `decision record` explicitly authorizes it.
- **No production claims:** This is an alpha runtime. Never output a `production_ready` kit or claim legal clearance for names/taglines.
- **Blockers:** If the platform attributes are completely missing, block execution and request the `define` route first.

## Step-by-step procedure

### 1. Initialize and retrieve context
Fetch the current platform and create a bounded task packet for the voice phase.

```bash
node <skill-dir>/bin/distinct.mjs packet --workspace <project> --brand <id> --task voice --scope "platform,attributes,positioning" --max-bytes 16384
```

### 2. Batch 1: Audit and category habits (Max 3 questions)
Ask the user the following questions to establish the baseline and the category habit to break:
1. What are the top three inconsistencies in your current messaging across different channels?
2. What is the dominant, predictable habit or cliché in your competitor category that we must actively avoid?
3. Who is the primary audience for the core message, and what is their biggest misconception about your category?

*Host-guided fallback:* If web crawling or document search is unavailable to audit live sites, ask the user to paste 5-10 examples of current copy or competitor copy directly into the chat. Do not attempt to scrape without explicit host authorization.

### 3. Draft Voice Chart and Persona
Translate the platform attributes into voice pillars (advisory default: 3-5; owner may adjust for scope). Each pillar must have:
- **Pillar name:** A core adjective or trait.
- **Definition:** A concise phrase (advisory default: ~15 words; owner may adjust for scope) explaining the trait.
- **But not:** The guardrail preventing the trait from tipping into its negative extreme.
- **Sounds like:** One real example line demonstrating the trait.
- **Doesn't sound like:** One real example line demonstrating the guardrail.
- **Source attribute:** The specific platform field 9 attribute it traces to.

Draft a **Voice Persona**: A two-sentence description of the brand as a person, ensuring their traits match the pillars. Avoid generic "friendly expert" traps.

### 4. Batch 2: Mechanics, range, and ban list (Max 3 questions)
Ask the user to refine the practical application of the voice:
1. What are 2-3 specific channels or situations (e.g., error messages, social media, investor decks) where the tone must noticeably shift?
2. What 3 strict grammatical or structural rules (mechanics) should replace vague adjectives like "be bold" or "be human"?
3. What 5 specific words or phrases from the competitor audit are permanently banned from our copy?

### 5. Draft Vocal Range and Mechanics
Create a **Vocal Range Grid** mapping the channels/situations from Batch 1 against the voice pillars, marking each cell as "up", "normal", or "down". Ensure no tone shift contradicts the core personality.

Create a **Mechanics Guide** with actionable writing rules per pillar (advisory default: 5-10; owner may adjust for scope). Include before-and-after rewrites (advisory default: at least three) based on the audit samples. Compile the **Ban List** of category clichés.

### 6. Batch 3: Messaging hierarchy (Max 3 questions)
Ask the user to define the core messaging structure:
1. What is the single umbrella message that explains who we are, what we do, and why we are different?
2. What are the three supporting pillars that prove the umbrella message?
3. What is one verifiable, specific proof point for each of the three supporting pillars?

### 7. Draft Messaging Hierarchy
Build the five-level messaging structure:
1. **Central idea:** A brief phrase (advisory default: 2-4 words; owner may adjust for scope; internal compass, traces to platform field 1).
2. **Positioning core:** The umbrella message (who, what, why different).
3. **Pillars:** Three supporting messages.
4. **Proof:** Verifiable proof points per pillar (advisory default: 1-3; owner may adjust for scope).
5. **Audience messages:** Tailored message sets for specific audiences.

Include the four Key Business Statements (Who, What, Why, How) at level 2.

### 8. Record state and validate
Save the voice and messaging artifacts as typed records. Ensure all pillars and messages trace back to the platform fields.

```bash
node <skill-dir>/bin/distinct.mjs record import --workspace <project> --brand <id> --file voice_record.json --expected-state <hash>
```
```bash
node <skill-dir>/bin/distinct.mjs record import --workspace <project> --brand <id> --file messaging_record.json --expected-state <hash>
```

Verify the state to ensure dependencies are met and hashes align.

```bash
node <skill-dir>/bin/distinct.mjs verify --workspace <project> --brand <id>
```

## Host-guided fallbacks
- **No browser/PDF access:** If the user needs to audit external documents or websites and host tools are unavailable, instruct them to export the text to a local file and use `evidence import` to bring it into the workspace.
- **No design tool for mockups:** If visual mockups of the voice in context (e.g., website headers, email signatures) are needed but design tools are unavailable, output plain-text markdown wireframes or structured tables showing the copy in context.
- **Missing legal/trademark tools:** If the user asks to clear a tagline or name, explicitly state that legal clearance is host-guided and unavailable natively. Do not invent clearance results.

## Exit criteria
- Voice chart, persona, vocal range, and mechanics guide are complete.
- Five-level messaging hierarchy and Key Business Statements are drafted.
- Every voice pillar traces to a specific platform attribute.
- Every message level traces to a platform field.
- Records are successfully imported into the workspace state.
- If the platform was only drafted, the voice and messaging records are explicitly marked as `draft`.

## Fictional example
**Brand:** Lumina (B2B wealth management software)
**Category habit to break:** "Trustworthy but dusty" (overuse of words like *secure, legacy, stewardship*).
**Voice Pillar 1:** Sharp but not cynical.
- *Definition:* Insightful and precise without being condescending.
- *But not:* Arrogant or dismissive of traditional methods.
- *Sounds like:* "Your portfolio shouldn't require a decoder ring."
- *Doesn't sound like:* "Legacy advisors rely on obsolete heuristics."
**Mechanics Rule:** Use active voice. Lead with verbs. Never use the word "leverage" as a verb.
**Messaging Level 2 (Umbrella):** We give independent advisors the institutional-grade analytics they need to scale their practice without hiring a back office.

