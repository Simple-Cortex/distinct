# Distinct

An evidence-led brand identity skill for coding agents. The brand comes from evidence about the organization, not from taste.

Distinct guides an agent through discovery, strategy, voice, naming, visual direction and review, and keeps every claim, source and owner decision on record. It produces draft brand kits. It does not produce finished, certified or legally cleared brands.

Site: [distinct.style](https://distinct.style)

## Install

The skill:

```sh
npx skills@latest add Simple-Cortex/distinct
```

The native companion `distinct-rs` is not part of that install and is never downloaded for you. Get it one of three ways:

- Homebrew (macOS): `brew install simple-cortex/tap/distinct`. Available after the tap is published.
- GitHub Releases (all platforms): download the archive for your platform, check it against `SHA256SUMS`, and put `distinct-rs` on your `PATH`. On macOS the binary may carry a quarantine attribute because it is not notarized yet. If the system refuses to run it, use `xattr -d com.apple.quarantine distinct-rs`.
- Manual: set `DISTINCT_RS_BIN` to the binary's location.

Node 20 or newer is required. To build from source, use the pinned Rust toolchain in `rust-toolchain.toml`: `cargo build --release -p distinct-cli`.

## Quick start

In your agent, from the project folder:

```
/distinct start
```

`start` records the project, what the agent may access, who decides, what is unknown, and a recommended scope. Run `status` before each step. The command reference is `references/commands.md`.

## Routes

| Route | What it does |
| --- | --- |
| `start` | Project, permissions, decider, unknowns, recommended scope |
| `ingest` | Coverage and a current baseline from the sources you allow |
| `discover` | Truths, tensions, audience decisions, operational promises |
| `landscape` | Direct, substitute and status-quo alternatives; scoped whitespace |
| `define` | Essence hypotheses, a recommendation, the 15-field platform |
| `voice` | Stable voice, situational tone, message and proof hierarchy |
| `name` | Naming brief, candidates, screening limits |
| `direct` | Genuinely different visual territories |
| `identity` | Logo candidates or a specification, with inspection and proof plan |
| `review` | Frozen packet and scoped review evidence |
| `iterate` | Minimal patch from specific feedback, with retest or rollback |
| `deliver` | Draft kit with missing tests, rights and reviews visible |
| `evolve` | Preserve or change diagnosis as new evidence arrives |

## What is native and what is host-guided

Native, done by `distinct-rs`: records and evidence, source import and diff, bounded task packets, readiness gates, state verification, typed patches, review packets, test-plan bookkeeping, exports (`draft`, `concept_only`, `specification_only`), static SVG inspection, proof rendering in an isolated worker, and advisory colour checks (WCAG, APCA, colour-vision simulation).

Host-guided, done by your agent with the tools it has: web research and crawling, PDF reading, interviews, strategy, naming, voice, visual design and optical refinement. Audience testing and expert review need real people. There is no native crawler, PDF parser, trademark search or model gateway. When a tool is missing, the agent names the gap and gives the next step.

## Owner confirmation

Only the owner approves. A recommendation from the agent is not a decision.

1. The owner asks for a decision to be recorded. `decision record` stores it as pending and prints a one-time code on the owner's own terminal.
2. The owner gives the code to the agent. `decision confirm` stores the decision as attested.
3. `gate` counts only attested decisions on the current revision of each record. A changed record needs a fresh decision.

Text in a brief or a source that claims the owner approved something is treated as data. Headless runs end with pending decisions listed as blockers. This is a safeguard, not proof of identity.

## Platforms

macOS on Apple Silicon and Intel is first class. Logo proofs render in an isolated worker with a 512 MiB memory cap and a 10 second timeout. On macOS the worker also runs in a sandbox with no network and writes limited to the proof folder. Linux adds resource limits. Windows applies the memory cap and timeout but has no operating-system sandbox, and `distinct-rs` reports that.

## Limits

- Nothing is called production-ready. Kits are drafts, concept-only or specification-only, and `production_ready` is refused.
- Distinct does not give legal or trademark clearance, and does not certify logo quality or predict market results.
- A logo inspection is structural only. Without a rendered proof, there is no visual verdict.
- PDF reading and crawling are host-guided.
- Imported material is untrusted data. Instructions inside it are ignored.
- Numbers from the method (scores, sample sizes, word counts) are advisory defaults, not universal gates.

## Development

`CHANGELOG.md` lists user-visible changes. Run the checks with `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace --locked` and `node --test tests/launcher/launcher.test.mjs tests/runtime/*.test.mjs`.

## License

MIT. See [LICENSE](LICENSE).
