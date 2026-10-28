# Changelog

All notable changes to Distinct are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
