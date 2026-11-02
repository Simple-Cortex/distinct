# Command reference: assets (logo, colour, naming)

Run `status` first. Exit codes: 0 ok, 1 check failed, 2 unavailable/blocked, 3 invalid input.

Notation: `D` = `node <skill-dir>/bin/distinct.mjs`; `G` = `--workspace <p> --brand <id>`. Output: one JSON envelope `{protocol_version, engine_version, execution, result, limitations[], changed[], error}`. Use absolute paths. Global: `--max-bytes N` (min 1024, default 16384; bounds `packet`, `test packet`, `review-packet`); `--format` has no effect.

Core commands (records, evidence, ingest, gate, decisions, export): `references/commands.md`.

### `logo inspect`
`D logo inspect G --file <logo.svg>` — static structural SVG check; stores a `logo_finding`. Mutates: yes. Exit 0 (read `verdict`: `inspected|invalid`); 3 on oversize, depth or element limits.
Notes: no optical or brand judgment. Scripts and external images give verdict `invalid`.

### `logo proof`
`D logo proof G --file <logo.svg> --out <dir> [--profile <profile.json>] [--force]` — render PNG proofs in a sandboxed resvg worker (10 s, memory cap); stores a `logo_proof`. Mutates: yes (PNGs to `<dir>`). If the host refuses nesting: `os_sandbox: host_inherited` + limitation `os_sandbox_host_inherited`. Exit 0; 2 if the latest inspect verdict for this file hash is not `inspected` or the OS sandbox fails; 3 on bad profile, `..` in `--out`, or non-empty `--out` without `--force`.
Profile: `{"renders":[{"size_px":128,"background":"transparent"},{"size_px":512,"background":"#ffffff"}]}`
Notes: 1 to 24 renders, `size_px` 1 to 4096 (under 16 MP), background `transparent` or `#rrggbb`; unknown fields rejected. Default: 128 and 256 transparent, 512 on `#ffffff`.

### `logo review-packet`
`D logo review-packet G` — list frozen logo findings (id, verdict, file hash, counts) and proofs (renderer, render count) for review. Mutates: no.
Notes: no verdict.

### `color check`
`D color check G --pairs <pairs.txt>` — advisory WCAG contrast, APCA Lc and protan/deutan/tritan simulation per foreground/background pair; stores a `color_check`. Mutates: yes. Exit 0; 3 on zero valid pairs.
Input (text, one pair per line, hex WITH or WITHOUT `#`; `//` lines are comments): `#5b3f8c #f4efe6` then `c9a227 ffffff`
Notes: `#rrggbb` and `#rgb` accepted. Response has only `pair_count`/`check_id`; numbers are in the stored `color_check` record. Advisory, not certification.

### `name check`
`D name check G --candidates <f.json>` — offline red-flag screen; read-only; 3 on bad input.
Input: `{"candidates":["Larkspur","Lärkspur"],"flag_lists":["default"]}` (lists optional).
Output per candidate: `normalized`, `changed_by_nfkc`, `scripts`, `mixed_script`, `confusable_skeleton`, `confusable_with`, `non_ascii`, `length_chars`, `flag_hits[{list,term,match:exact|word|substring}]`; fixed `limitations` (heuristic, not clearance, incomplete).
Notes: no verdict or score; no hits is not safety. No network.
