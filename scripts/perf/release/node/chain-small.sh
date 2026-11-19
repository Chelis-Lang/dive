#!/bin/bash
# Runs D5, D3, D6 and the D2 benchmark-set part one after another on the small node,
# after the D1 small class finishes. Each step logs to ~/chain.log.
set -u
REV=${DIVE_REV:-$(git -C ~/dive rev-parse HEAD)}
while systemctl is-active --quiet r2-small; do sleep 30; done
B=numactl
P="numactl --cpunodebind=0 --membind=0"
for step in d5 d3 d6; do
  echo "start $step $(date -u +%FT%TZ)" >> ~/chain.log
  $P python3 ~/tools/dmeasure.py $step ~/m/$step >> ~/chain.log 2>&1
  echo "end $step rc=$? $(date -u +%FT%TZ)" >> ~/chain.log
done
echo "start d2bench $(date -u +%FT%TZ)" >> ~/chain.log
mkdir -p ~/m/d2bench
cd ~/dive
$P env -i PATH=/usr/local/bin:/usr/bin:/bin HOME=$HOME DIVE_CACHE_DIR=$HOME/.cache/dive \
  ~/bins/dive-bench --dive-bin ~/bins/dive --dive-revision "$REV" \
  --profile both --workers 1,8 --reps 5 --tlc --timeout 900 --xmx 3g \
  --runs ~/m/d2bench/runs.jsonl --summary ~/m/d2bench/summary.json > ~/m/d2bench/console.txt 2>&1
echo "end d2bench rc=$? $(date -u +%FT%TZ)" >> ~/chain.log
touch ~/m/CHAIN_DONE
