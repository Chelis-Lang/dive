#!/usr/bin/env bash
# Runs `dive check --compat=tlc` on a fixed list of models against the
# committed TLC goldens and fails unless every one of them agrees.
#
#   scripts/conformance-gate.sh conformance/gates/cm1.txt [--workers 1,4,8] [extra run args...]
# Set DIVE_GATE_CORPUS for another registered corpus (default: examples).
#
# Needs release builds of the harness and of dive
# (`cargo build --release --locked -p dive-conformance -p dive-cli`) and the
# fetched oracles and Examples corpus (the harness's `oracle fetch` and
# `corpus fetch --corpus examples`). It never runs TLC.
#
# A model passes only when its outcome is `agree`. Every counterexample is
# validated by the harness with the independent trace validator (WP-E1,
# ADR 0016), so `agree-pending-trace-validation` (a counterexample the
# validator could not confirm) fails, like `unsupported` and everything else.
set -euo pipefail

list="${1:?usage: conformance-gate.sh <model-list> [run args...]}"
shift
harness="${HARNESS:-target/release/dive-conformance}"
corpus="${DIVE_GATE_CORPUS:-examples}"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT

filters=()
expected=0
while IFS= read -r line; do
  line="${line%%#*}"
  line="$(echo "$line" | tr -d '[:space:]')"
  [ -z "$line" ] && continue
  filters+=(--filter "$line")
  expected=$((expected + 1))
done <"$list"
[ "$expected" -gt 0 ] || { echo 'gate: empty selection' >&2; exit 2; }

start=$(date +%s)
set +e
"$harness" run --impl dive --impl-profile tlc --corpus "$corpus" --tier all \
  --no-sany --no-summary --tlc-budgets --budget-cap 900 \
  --report-dir "$out" "${filters[@]}" "$@" \
  2>&1 | tee "$out/log"
status=${PIPESTATUS[0]}
set -e
elapsed=$(($(date +%s) - start))

seen=$(grep -cE '^\[[0-9]+/[0-9]+\] ' "$out/log" || true)
bad=$(grep -E '^\[[0-9]+/[0-9]+\] ' "$out/log" |
  grep -vE '^\[[0-9]+/[0-9]+\] agree ' || true)
echo "gate: $seen of $expected models run in ${elapsed}s"
if [ "$status" -ne 0 ] || [ "$seen" -ne "$expected" ] || [ -n "$bad" ]; then
  [ -n "$bad" ] && printf 'gate: not agreeing:\n%s\n' "$bad"
  echo "gate: FAILED (harness exit $status)"
  exit 1
fi
echo "gate: passed"
