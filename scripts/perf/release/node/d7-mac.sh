#!/bin/sh
# d7-mac.sh <out-dir>: D7 macOS spot check, run from the root of a dive checkout at the
# release revision after `cargo build --release --locked -p dive-cli --bins -p dive-bench`.
# Runs the six-model dive-bench selection at 1 and 4 workers with pinned TLC, then the
# 50-run startup timing (d7-startup.py). Records load averages in <out-dir>/log.txt,
# because the 0.1.0 run was on a laptop in use by other work.
set -u
OUT=${1:?usage: d7-mac.sh <out-dir>}
HERE=$(cd "$(dirname "$0")" && pwd)
REV=${DIVE_REV:-$(git rev-parse HEAD)}
mkdir -p "$OUT"
echo "start $(date -u +%FT%TZ) load $(sysctl -n vm.loadavg)" > "$OUT/log.txt"
shasum -a 256 target/release/dive target/release/dive-bench > "$OUT/sha.txt"
env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="$HOME" DIVE_CACHE_DIR="$HOME/.cache/dive" \
  target/release/dive-bench --dive-bin target/release/dive --dive-revision "$REV" \
  --profile default --workers 1,4 --reps 3 --tlc --xmx 3g --timeout 300 \
  --filter MCDisruptor_MPMC --filter SimpleRegular --filter MCbtree --filter MCBakery \
  --filter MCLamportMutex --filter CoffeeCan1000BeansSafety \
  --runs "$OUT/runs.jsonl" --summary "$OUT/summary.json" > "$OUT/console.txt" 2>&1
echo "bench rc=$? $(date -u +%FT%TZ) load $(sysctl -n vm.loadavg)" >> "$OUT/log.txt"
python3 -I "$HERE/d7-startup.py" target/release/dive "$OUT/startup.json" >> "$OUT/log.txt" 2>&1
echo "end $(date -u +%FT%TZ) load $(sysctl -n vm.loadavg)" >> "$OUT/log.txt"
