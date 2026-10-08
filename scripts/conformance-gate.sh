#!/usr/bin/env bash
# Runs `dive check --compat=tlc` on a fixed list of Examples models against the
# committed TLC goldens and fails unless every one of them agrees.
#
#   scripts/conformance-gate.sh conformance/gates/cm1.txt [--workers 1,4] [extra run args...]
#
# Needs release builds of the harness and of dive
# (`cargo build --release --locked -p dive-conformance -p dive-cli`) and the
# fetched oracles and Examples corpus (the harness's `oracle fetch` and
# `corpus fetch --corpus examples`). It never runs TLC.
#
# A model passes when its outcome is `agree`, or `agree-pending-trace-validation`
# while the independent trace validator (WP-E1) does not exist. Anything else,
# including `unsupported`, fails the gate.
set -euo pipefail

list="${1:?usage: conformance-gate.sh <model-list> [run args...]}"
shift
harness="${HARNESS:-target/release/dive-conformance}"
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

start=$(date +%s)
set +e
"$harness" run --impl dive --impl-profile tlc --corpus examples --tier all \
  --no-sany --no-summary --report-dir "$out" "${filters[@]}" "$@" \
  2>&1 | tee "$out/log"
status=${PIPESTATUS[0]}
set -e
elapsed=$(($(date +%s) - start))

seen=$(grep -cE '^\[[0-9]+/[0-9]+\] ' "$out/log" || true)
bad=$(grep -E '^\[[0-9]+/[0-9]+\] ' "$out/log" |
  grep -vE '^\[[0-9]+/[0-9]+\] (agree|agree-pending-trace-validation) ' || true)
echo "gate: $seen of $expected models run in ${elapsed}s"
if [ "$status" -ne 0 ] || [ "$seen" -ne "$expected" ] || [ -n "$bad" ]; then
  [ -n "$bad" ] && printf 'gate: not agreeing:\n%s\n' "$bad"
  echo "gate: FAILED (harness exit $status)"
  exit 1
fi
echo "gate: passed"
