# Inspiration intake

Used by `start` (collect), `landscape` (competitor screenshots) and `direct` (mandatory before any visual work).

## Rule

No visual territory, visual brief or identity candidate before an inspiration board passes `gate --scope direction`. Never describe a reference you have not fetched and looked at in this session. Memory of a studio or brand is not a reference.

## Intake

Collect links or files from the user: work they like, work they dislike, and brands they admire outside the category. Ask why for each. Accept URLs, image files, PDFs, and Are.na, Pinterest or Savee boards.

## Fetch and look

For each reference, use the host's actual tools: open the page in a browser tool and screenshot it, or download the images (curl or WebFetch) into a fresh staging folder in the workspace, then:

```text
node <skill-dir>/bin/distinct.mjs evidence import --source-id insp-<ref_id>-<n> --file <file> --workspace <project> --brand <id>
```

Use a new source id for every file (`insp-r1-1`, `insp-r1-2`, ...): reusing an id with different bytes is refused (exit 3). List every id in the reference's `evidence_ids`.

View the image (vision) before writing observations. Fetch at least 3 references unless the user supplied fewer. For each, record at least 3 concrete visible observations (type, colour, composition, mark type, imagery, motion), not adjectives like "clean" or "modern" alone. Good: "condensed grotesk, all caps, 2-colour, heavy rules".

## Show back

Present the fetched set to the user with your observations and ask what they react to. Record `user_reaction` verbatim.

## No references

Offer the starter catalog below: pick 3-5 aesthetic directions plus 4-6 studios that fit the platform constraints (or the brief if no platform yet), with one-line reasons. The user picks. Then fetch real, current case studies from the chosen studios and galleries (search the studio name; the URLs below may be stale) and run the same fetch-and-look loop. Set `origin: starter_catalog`.

## Unreachable

If a site blocks fetching or no browser or network tool exists, mark the reference `unreachable` with the reason and ask the user to drop screenshots or images into the workspace. The text-only path is allowed only after the user has been asked for files. The gate still blocks until at least one reference is fetched, and warns `inspiration_unreachable` for each unreachable one.

## Rights and use

Inspiration is reference-only: never trace, copy or ship it. Directions must cite which observations they borrow and which they deliberately break. Keep inspiration and competitor evidence separate (competitors live in `landscape`).

## Record

Import one `source` record with `"privacy":"confidential"` (exports always exclude it, so reference lists and verbatim reactions never reach a client kit; the gate still reads it), then check the gate:

```json
{"kind":"source","id":"inspiration-board","data":{"type":"inspiration_board",
 "origin":"user_supplied|starter_catalog|mixed",
 "references":[{"ref_id":"r1","url":"https://...","label":"...","stance":"like|dislike|admired_outside_category",
   "status":"fetched|unreachable","evidence_ids":["insp-r1-1","insp-r1-2"],
   "observations":["condensed grotesk all caps, 2-colour, heavy rules"],
   "user_reaction":null,"unreachable_reason":null}]}}
```

(The record also carries the usual envelope fields: `schema_version`, `brand_id`, `privacy`, `created_at`, `created_by`, `dependencies`, `limitations`.)

```text
node <skill-dir>/bin/distinct.mjs record import --file inspiration-board.json --workspace <project> --brand <id>
node <skill-dir>/bin/distinct.mjs gate --scope direction --workspace <project> --brand <id>
```

Gate blockers: `inspiration_missing`, `inspiration_unseen`, `inspiration_evidence_dangling` (per reference, carries `ref_id`). `evidence_ids` must be ids from `evidence import`; forgetting that evidence makes the reference dangling.

## Starter catalog

> A starting point for users who don't yet have references. It is not evidence. Open each pick live and show real, current work; studios rename, merge and move. Choose by fit to the platform, not fame.

**Studios by tendency** (search the name; the domain hint may be stale)
| Tendency | Studios |
|---|---|
| Systems, modernist, typographic rigour | Pentagram (pentagram.com), Experimental Jetset (experimentaljetset.nl), Studio Dumbar (studiodumbar.com), Order (order.design), Base Design (basedesign.com) |
| Bold, culture-forward, expressive | Collins (wearecollins.com), Porto Rocha (portorocha.com), Mucho (wearemucho.com), &Walsh (andwalsh.com), Snask (snask.com), Hey Studio (heystudio.es) |
| Strategy-led, large-scale brand systems | Wolff Olins (wolffolins.com), Koto (koto.studio), DesignStudio (design.studio), Jones Knowles Ritchie (jkrglobal.com), Landor (landor.com), Moving Brands (movingbrands.com), Saffron (saffron-consultants.com) |
| Craft, editorial, heritage | Lundgren+Lindqvist (lundgrenlindqvist.se), Spin (spin.co.uk), Build (wearebuild.com), Kurppa Hosk (kurppahosk.com) |
| Tech and product brands | Ragged Edge (raggededge.com), Manual (manualcreative.com), Character (charactersf.com), Mother Design (motherdesign.com) |

**Galleries and archives:** Brand New (underconsideration.com/brandnew), BP&O (bpando.org), Fonts In Use (fontsinuse.com), Identity Designed (identitydesigned.com), The Brand Identity (the-brandidentity.com), Behance branding, Are.na, Savee, Cosmos, It's Nice That, Typewolf; for digital product surfaces: Siteinspire, Godly, Mobbin.

**Aesthetic directions** (name: visible traits — fits — risk)
1. Swiss modernist: strict grid, grotesk type, flush-left, few colours. Fits institutions and precise B2B. Risk: anonymous.
2. Warm editorial: bookish serifs, generous margins, paper tones. Fits publishers, food, considered retail. Risk: slow, precious.
3. Quiet luxury: restraint, wide tracking, muted palette, sparse imagery. Fits premium goods and services. Risk: indistinguishable from peers.
4. Neo-brutalist: raw system fonts, hard borders, exposed structure. Fits challengers and creative tools. Risk: hostile, dated fast.
5. Playful maximalist: saturated colour clashes, big type, illustration. Fits consumer and youth. Risk: noise over meaning.
6. Organic hand-made: drawn marks, imperfect edges, natural textures. Fits craft, wellness, food. Risk: twee, hard to scale.
7. Retro-futurist / Y2K: chrome, gradients, wide display type. Fits music, fashion, culture. Risk: trend half-life.
8. Technical / engineered: monospace, diagrams, data, rulers and grids. Fits dev tools, science, infrastructure. Risk: cold.
9. Heritage revival: archival marks, crests, period type. Fits old institutions and provenance stories. Risk: costume.
10. Experimental type / acid: distorted, variable and kinetic letterforms. Fits culture and events. Risk: illegible at small sizes.
11. Japanese-influenced minimal: negative space, quiet hierarchy, fine lines. Fits calm and precise products. Risk: appropriation and cliché; ground it in a real reason.
12. Bauhaus geometric: primary shapes and colours, constructivist layouts. Fits education and making. Risk: overused shorthand for "design".
13. Soft tech: rounded type, pastel gradients, friendly 3D. Fits consumer apps. Risk: the current SaaS default; rarely distinct.
14. Vernacular / signage: street lettering, hand-painted signs, local type. Fits place-based brands. Risk: borrowing a community's visual voice without a real tie.
