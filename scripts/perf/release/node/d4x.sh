#!/bin/bash
# d4x.sh <chunk-file>: run D4 chunks one after another, one dive-bench --reps 1 call per line.
# Each line: <tag> <workers-csv> <model-cfg>...   Output: ~/m/d4x/<tag>/runs.jsonl
set -u
REV=${DIVE_REV:-$(git -C ~/dive rev-parse HEAD)}
while read -r tag workers models; do
  [ -z "$tag" ] && continue
  d=~/m/d4x/$tag; mkdir -p $d
  : > $d/set.toml
  for m in $models; do
    name=$(echo "$m" | sed 's#specifications/##; s#/#_#g; s#\.cfg$##')
    printf '[[bench]]\nname = "%s"\nmodel = "%s"\nwhy = "D1 large class"\n\n' "$name" "$m" >> $d/set.toml
  done
  sha256sum $d/set.toml > $d/set.sha256
  echo "start $tag w$workers $(date -u +%FT%TZ)" >> ~/d4x.log
  cd ~/dive
  numactl --cpunodebind=0 --membind=0 env -i PATH=/usr/local/bin:/usr/bin:/bin HOME=$HOME DIVE_CACHE_DIR=$HOME/.cache/dive \
    ~/bins/dive-bench --set $d/set.toml --dive-bin ~/bins/dive --dive-revision "$REV" \
    --profile default --workers $workers --reps 1 --timeout 3600 --xmx 24g \
    --runs $d/runs.jsonl --summary $d/summary.json > $d/console.txt 2>&1
  echo "end $tag rc=$? $(date -u +%FT%TZ)" >> ~/d4x.log
done < "$1"
touch ~/d4x.DONE
