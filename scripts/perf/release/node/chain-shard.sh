#!/bin/bash
# After this shard's D1 large pass, run D4 worker scaling on the shard's models.
set -u
REV=${DIVE_REV:-$(git -C ~/dive rev-parse HEAD)}
while systemctl is-active --quiet r2-large; do sleep 60; done
rm -f ~/m/d4/runs.jsonl ~/m/d4/summary.json
mkdir -p ~/m/d4
python3 - <<'EOF'
import os
models = [m for m in open(os.path.expanduser("~/tools/shard.txt")).read().split() if m != "specifications/CheckpointCoordination/MCCheckpointCoordinationFailure.cfg"]  # dive-bench needs a success golden
with open(os.path.expanduser("~/m/d4/set.toml"), "w") as f:
    for m in models:
        name = m.replace("specifications/", "").replace("/", "_").replace(".cfg", "")
        f.write(f'[[bench]]\nname = "{name}"\nmodel = "{m}"\nwhy = "D1 large class"\n\n')
EOF
sha256sum ~/m/d4/set.toml > ~/m/d4/set.sha256
echo "start d4 $(date -u +%FT%TZ)" >> ~/chain.log
cd ~/dive
numactl --cpunodebind=0 --membind=0 env -i PATH=/usr/local/bin:/usr/bin:/bin HOME=$HOME DIVE_CACHE_DIR=$HOME/.cache/dive \
  ~/bins/dive-bench --set ~/m/d4/set.toml --dive-bin ~/bins/dive --dive-revision "$REV" \
  --profile default --workers 1,4,8 --reps 3 --timeout 3600 --xmx 24g \
  --runs ~/m/d4/runs.jsonl --summary ~/m/d4/summary.json > ~/m/d4/console.txt 2>&1
echo "end d4 rc=$? $(date -u +%FT%TZ)" >> ~/chain.log
touch ~/m/CHAIN_DONE
