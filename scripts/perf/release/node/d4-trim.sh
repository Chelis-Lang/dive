#!/bin/bash
# d4-trim.sh <rows>: stop this shard's D4 once <rows> runs are recorded (later reps run on extra nodes).
set -u
while [ "$(wc -l < ~/m/d4/runs.jsonl)" -lt "$1" ]; do sleep 15; done
pkill -f 'bins/dive-bench'; sleep 1; pkill -f 'bins/dive check'
echo "d4 trimmed at $1 rows $(date -u +%FT%TZ)" >> ~/chain.log
touch ~/m/d4/TRIMMED
