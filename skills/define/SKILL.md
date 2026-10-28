---
name: "distinct-define"
description: "Use when distilling brand essence and drafting the 15-field brand platform. Equivalent to /distinct define; routes through the root Distinct skill."
---

# /distinct define

This file is a thin trigger. The root `SKILL.md` (two folders up) holds routing, prerequisites and honesty rules, so read its `define` row first.

1. Resolve `<skill-dir>`, the folder that contains the root `SKILL.md`, and the project workspace (`--workspace`, default the current directory).
2. Run `node <skill-dir>/bin/distinct.mjs status --workspace <project> --brand <id>` to load the current state and prerequisites.
3. Read `<skill-dir>/references/strategy.md`, section `define`, and follow it.
4. Record outputs with the native commands that reference names, invoked through `bin/distinct.mjs`. Never write `.distinct/` files by hand.
