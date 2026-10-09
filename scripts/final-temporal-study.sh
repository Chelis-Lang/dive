#!/usr/bin/env bash
# Runs a separate, non-gating final/short study for an exact model list.
# Usage: scripts/final-temporal-study.sh <list> [study options...]
# HARNESS and DIVE_CACHE_DIR work like the ordinary harness.
set -euo pipefail
list="${1:?usage: final-temporal-study.sh <list> [study options...]}"
shift
harness="${HARNESS:-target/release/dive-conformance}"
filters=()
while IFS= read -r line; do
  line="${line%%#*}"
  line="$(echo "$line" | tr -d '[:space:]')"
  [ -z "$line" ] && continue
  filters+=(--filter "$line")
done < "$list"
[ "${#filters[@]}" -gt 0 ] || { echo 'study: empty selection' >&2; exit 2; }
"$harness" study final --corpus examples --tier all "${filters[@]}" "$@"
