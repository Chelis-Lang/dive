#!/usr/bin/env bash
# Runs `dive-conformance shadow` (core vs reference engine,
# docs/developer/core/08-correctness.md §4) on the models of a list file, one
# model path per line, `#` comments allowed. Extra arguments go to the
# harness. CI's `Conformance gate` job runs
#   scripts/shadow-gate.sh conformance/gates/shadow-smoke.txt
# and `cargo xtask gate --full` runs it on conformance/gates/cm1.txt at 1, 4
# and 8 workers (not strict) before shadowing the whole small tier. By
# default the gate runs `--strict`: a listed model the core refuses, or a
# timeout, fails it. Every core run also compares the successors of 32
# sampled states with the reference evaluator's (`--sample`). Later
# arguments win, so `--workers 1,4,8` replaces the default worker counts.
#
#   HARNESS           harness binary (default target/release/dive-conformance)
#   DIVE_GATE_CORPUS  corpus of the listed models (default examples)
#   SHADOW_STRICT     0 to count refusals and timeouts instead of failing
set -euo pipefail

list="${1:?usage: shadow-gate.sh <model-list> [shadow args...]}"
shift
harness="${HARNESS:-target/release/dive-conformance}"
corpus="${DIVE_GATE_CORPUS:-examples}"
strict=(--strict)
[ "${SHADOW_STRICT:-1}" = 0 ] && strict=()

filters=()
expected=0
while IFS= read -r line; do
  line="${line%%#*}"
  line="$(echo "$line" | tr -d '[:space:]')"
  [ -z "$line" ] && continue
  filters+=(--filter "$line")
  expected=$((expected + 1))
done <"$list"
[ "$expected" -gt 0 ] || { echo 'shadow gate: empty selection' >&2; exit 2; }

start=$(date +%s)
set +e
"$harness" shadow ${strict[@]+"${strict[@]}"} --corpus "$corpus" --tier all --workers 1,4 \
  --tlc-budgets --budget-cap 900 "${filters[@]}" "$@"
status=$?
set -e
echo "shadow gate: $expected model(s) in $(($(date +%s) - start))s"
if [ "$status" -ne 0 ]; then
  echo "shadow gate: FAILED (harness exit $status)"
  exit 1
fi
echo "shadow gate: passed"
