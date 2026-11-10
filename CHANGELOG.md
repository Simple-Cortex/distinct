# Changelog

All notable changes to Distinct are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.1] - 2026-11-10

### Added
- `references/critique.md`: self-critique protocol for logos and identity systems. Findings carry a claim class (MEASURED, JUDGED, OBSERVED), the critic runs in a fresh context on the frozen packet plus proof renders, and the protocol adds order-swapped and adversarial passes, a hidden-name competitor line-up, an anchored pointwise rubric, calibration seeds and a context gate (16/24 px, one-colour, reversed, real application) before any positive verdict.
- `references/marks-as-code.md`: construct marks as small generators from a grid and named constants, emitting engine-safe SVG (the elements `logo inspect` accepts); optional permissively licensed tooling (skia-pathops, picosvg, SVGO, fontTools, vtracer) and an avoid list with licence reasons.
- `references/design-tools.md` and `references/design-tools-canvases.md`: host-side guide for design-canvas MCPs (Pencil, Paper, OpenDesign, Doop). Canvas state is never evidence until exported and imported, and canvas approvals are never owner decisions.
- `references/critique.md` rubric items have stable ids (scored by the eval suite) and new items `negative_space_hazard`, `balance`, `colour_contrast`, `construction_quality`, `engine_safe`; the context gate adds mirrored, rotated and inverted renders, a secondary-figure screen and a cold read in a lockup on a busy plate. New rules: self-flagged risks are reworked or dropped in the same round, boards carry a QA checklist, at most three finalists with a rejected list, similarity is checked rather than asserted. `visual.md`, `inspiration.md` and `iteration.md` add a one-line idea per route before drawing, palette and carried-over decision checks each round, and derivation from a reference's reason rather than its technique.
- Calibration fixtures (`tests/fixtures/calibration`) and an eval suite (`tests/eval`): layer 1 (engine calibration and scorer tests) runs in CI, layer 2 (critic runs with a model) is host-run.
- `DISTINCT_PROBE_TIMEOUT_MS` env override (integer ms, default 20000; invalid values ignored) for the launcher's native protocol probe.

### Changed
- `SKILL.md`, `references/review.md`, `references/visual.md` and the `review` and `identity` route skills point to the new references; `runtime-manifest.json` lists them.
- `record import` and `patch` (add, replace, remove) refuse the engine-written kinds `logo_finding`, `logo_proof` and `color_check` (exit 3).
- `logo inspect` reports `native_proof: "available"` (was `"unavailable"`).
- All `color check` APCA Lc values change: they now match APCA-W3 0.1.9 (black on white is 106.04, was 114.00).
- The launcher's native probe timeout is 20 s (was 5 s). A timed-out probe exits 2 with the `native_probe_timeout` limitation and an error naming the probe; a native binary killed by a signal is reported on stderr.
- `logo proof` picks the `inspect-<hash12>` finding with the latest `created_at` (ties broken by record hash).

### Fixed
- `color check` APCA Lc now matches the APCA-W3 reference (`apca-w3@0.1.9`, 0.0.98G-4g): black soft-clamp, 2.4-exponent luminance, low-clip and offsets, delta-Y minimum. Black on white is 106.04 (was 114.00). Corrected the sign-convention doc comment and added golden tests (#1).
- Exports no longer leak local paths: `specification_only` drops `logo_proof` and `color_check`; `draft` and `concept_only` strip `proofs_dir` from `logo_proof` and reduce `color_check.pairs_file` to its file name. Stored records are unchanged. Added a test that scans every exported file for absolute paths (#2).
- `logo proof` now looks up the finding by the deterministic `inspect-<hash12>` id (and full hash) instead of the first matching record, and `record import` refuses the engine-written kinds `logo_finding`, `logo_proof` and `color_check` (exit 3), so the inspection gate cannot be forged (#3).
- `logo inspect` reports `native_proof: "available"` (native proof exists via `logo proof`) instead of `"unavailable"` (#3).
- `references/commands-assets.md` and `references/commands.md` now list the three `logo inspect` verdicts and supported elements, and describe the real `specification_only` exclusions (#4).
- `references/visual.md` documents the visual brief as a `source` record with `data.type: "visual_brief"` (the `visual_brief` kind did not exist, so `record import` rejected it) (#4).

## [0.4.0] - 2026-11-03

### Added
- `gate` landscape rule for `direction`, `deliver` and the default scope: blocks `landscape_missing`, `landscape_unseen` and `landscape_evidence_dangling` until at least one competitor was fetched, looked at and imported as evidence. `not_applicable_reason` passes with an advisory; `landscape_unreachable` and `landscape_thin` are advisories only.
- Agency process study: inspiration references accept `kind` and `process_sources`, and `gate` blocks `agency_process_unread` for a fetched agency reference whose case studies or process writing were not read. Boards also carry `process_principles`.
- Logo breakdowns: boards accept `logo_breakdowns`, and `gate --scope logo_intent` blocks `logo_breakdown_missing` until a liked logo is broken down across at least four facets with transferable principles and `do_not_copy` entries. Identity candidates must cite the principles they use.

## [0.3.0] - 2026-11-02

### Added
- `name check`: offline naming screen that reports NFKC changes, Unicode scripts (Japanese and Korean count as single-script), confusable lookalikes (UTS #39) and hits from a small, incomplete English flag list for each candidate. Advisory only: no verdict, no network calls, not trademark clearance.
- `evidence import --replace` to overwrite a source id on purpose; the result reports `replaced` and `previous_byte_hash`.

### Changed
- `evidence import` refuses to reuse a source id for different bytes (exit 3) instead of silently replacing the earlier entry. Re-importing identical bytes succeeds with `unchanged: true`.
- The command reference is split: logo, colour and naming commands moved verbatim to `references/commands-assets.md`, so routes load only the commands they use.
- Launcher trace capture (`DISTINCT_TRACE`) adds an `args` field with flag names only; every value is `<redacted>` except `--scope` and an `insp-*` marker for inspiration evidence.

## [0.2.1] - 2026-11-02

### Added
- Reference intake: `gate` now blocks `direction`, `deliver` and the default scope until an inspiration board (a `source` record of type `inspiration_board`) holds a fetched, observed reference whose evidence is in the index. New `references/inspiration.md` with the fetch-and-look procedure and a starter catalog of studios, galleries and aesthetic directions.

## [0.2.0] - 2026-10-27

First public release.

### Added
- The `distinct` skill: thirteen routes (`start`, `ingest`, `discover`, `landscape`, `define`, `voice`, `name`, `direct`, `identity`, `review`, `iterate`, `deliver`, `evolve`) that take a brand from sources and interviews to a draft brand kit, with evidence tracked at each step. Install with `npx skills@latest add Simple-Cortex/distinct`.
- `distinct-rs`, a native companion binary with records, evidence import and forget, source ingestion planning and import, metadata queries, bounded task packets, readiness gates, state verification, and draft, concept-only and specification-only exports. It is distributed through GitHub Releases, with Homebrew planned.
- Owner decisions: `decision record` stores a decision as pending and shows a one-time confirmation code only on the owner's own terminal. `decision confirm` makes it count.
- `gate` blocks on missing or out-of-date owner approval for each record in scope.
- Native SVG logo inspection and proof rendering in an isolated worker, with a memory cap and time limit. Proofs run inside an operating-system sandbox on macOS; Linux applies resource limits; Windows applies the memory and time limits but has no operating-system sandbox, and reports that.
- Advisory colour-pair checks for contrast (WCAG and APCA) and colour-vision simulation.
- Typed content patches with dry-run preview, check plans and results, review records, frozen review packets with builder rationale stripped, and a test bench that records plans, observations and counts without inferring cause.
- Draft kit export includes design tokens and guidelines built only from owner-approved fields.
- Opt-in session trace capture via `DISTINCT_TRACE`.

### Changed
- Coverage is stated plainly. Native: records, evidence, packets, gates, verification, exports, SVG inspection and proofing, colour checks, patches and the test bench. Host-guided: research, crawling, PDF reading, interviews, strategy, naming, voice, visual design and optical refinement, and any audience testing or expert review. There is no native crawler or PDF parser.
- Kits export only as `draft`, `concept_only` or `specification_only`. `production_ready` is refused, and Distinct does not certify logo quality, give legal or trademark clearance, or predict market results.
- The skill launcher never downloads the binary. Point `DISTINCT_RS_BIN` at it, or put `distinct-rs` on `PATH`.

### Security
- Export and ingestion paths are validated, so a source id, payload path or output path cannot escape its directory.
- Exports leave out confidential and restricted records and quotes that lack consent.
- Decisions need owner confirmation on the owner's terminal. Text in a brief or source that claims an owner decision is treated as data, not as approval. This is a safeguard, not proof of identity.
- Logo proofs render in a sandboxed worker with no network access and write access limited to the proof directory (macOS), plus a memory cap and time limit. When Distinct already runs inside a host's sandbox, which does not allow nesting, the proof relies on that sandbox and records `os_sandbox: host_inherited`.
