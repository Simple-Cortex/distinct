#!/usr/bin/env bash
# EXAMPLE host runner for the critic eval. Not run in CI. Costs tokens.
# Usage: run-claude-code.example.sh <model> <seed> [--lineup]
#   e.g. run-claude-code.example.sh sonnet 1 --lineup
# The critic is a fresh headless Claude Code run. It sees only packet/ and the
# rubric in references/critique.md. It never gets key.json or manifest.json.
set -euo pipefail

MODEL="${1:?model required}"
SEED="${2:?seed required}"
shift 2
EXTRA=("$@")

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../../.." && pwd)"
# The rubric ids come from references/critique.md (docs PR Simple-Cortex/distinct#5).
if [ ! -f "$REPO/references/critique.md" ]; then
  echo "error: $REPO/references/critique.md not found. It ships in Simple-Cortex/distinct#5; merge or check out that branch first." >&2
  exit 1
fi

RUN="$REPO/tests/eval/results/$(date +%Y%m%dT%H%M%S)-seed${SEED}"
mkdir -p "$RUN"

run_critic() {  # $1 = suffix ("" or "-rev"), $2.. = extra build-packet flags
  local suffix="$1"; shift
  local out="$RUN/packet${suffix}"
  node "$HERE/build-packet.mjs" --seed "$SEED" --out "$out" "${EXTRA[@]}" "$@"
  # Isolated working dir: packet/ and the rubric only.
  local sandbox; sandbox="$(mktemp -d)"
  cp -R "$out/packet" "$sandbox/packet"
  cp "$REPO/references/critique.md" "$sandbox/rubric.md"
  (
    cd "$sandbox"
    claude -p --output-format json --model "$MODEL" --allowedTools "Read" \
      "You are reviewing logo candidates. Read rubric.md, then review every candidate in packet/index.md by opening its PNG files. \
Reply with ONLY JSON: {\"candidates\":[{\"label\":\"K-01\",\"verdict\":\"present|rework|drop\",\"findings\":[{\"item\":\"<rubric item>\",\"class\":\"MEASURED|JUDGED|OBSERVED\",\"severity\":\"minor|major|blocker\",\"text\":\"...\"}]}]}. \
Only candidates labelled K-xx are candidates; E-xx are existing marks for comparison." \
      | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const r=JSON.parse(s).result;process.stdout.write(typeof r==="string"?r.replace(/^\s*```(?:json)?\s*|\s*```\s*$/g,""):JSON.stringify(r))})' \
      > "$RUN/critic${suffix}.json"
  )
  cp "$out/key.json" "$RUN/key${suffix}.json"
}

run_critic ""
run_critic "-rev" --reverse

node "$HERE/score.mjs" --key "$RUN/key.json" --critic "$RUN/critic.json" \
  --reverse-critic "$RUN/critic-rev.json" | tee "$RUN/report.json"
echo "results: $RUN" >&2
