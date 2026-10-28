---
name: "distinct-deliver"
description: "Use when exporting a draft or provisional brand kit with coverage gaps visible. Equivalent to /distinct deliver; routes through the root Distinct skill."
---

# /distinct deliver

This file is a thin trigger. The root `SKILL.md` (two folders up) holds routing, prerequisites and honesty rules, so read its `deliver` row first.

1. Resolve `<skill-dir>`, the folder that contains the root `SKILL.md`, and the project workspace (`--workspace`, default the current directory).
2. Run `node <skill-dir>/bin/distinct.mjs status --workspace <project> --brand <id>` to load the current state and prerequisites.
3. Read `<skill-dir>/references/delivery.md`, section `deliver`, and follow it.
4. Record outputs with the native commands that reference names, invoked through `bin/distinct.mjs`. Never write `.distinct/` files by hand.
