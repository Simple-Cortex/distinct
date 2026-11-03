# Changelog

All notable changes to Distinct are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
