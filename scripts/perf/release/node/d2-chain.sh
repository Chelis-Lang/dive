#!/bin/bash
# D2 liveness capture: wait for the node's chain, probe each PROPERTY model once,
# keep the ones whose report says check_kind temporal, then run three repetitions.
#   d2-chain.sh <models.json> <out-dir>
set -u
M=$1
O=$2
while [ ! -f "${3:-$HOME/m/CHAIN_DONE}" ]; do sleep 60; done
mkdir -p "$O"
P="numactl --cpunodebind=0 --membind=0"
echo "start d2probe $(date -u +%FT%TZ)" >> ~/chain.log
$P python3 ~/tools/dmeasure.py probe "$O/probe" "$M" >> ~/chain.log 2>&1
python3 - "$M" "$O/probe/rows.jsonl" "$O/live.json" <<'EOF'
import json, sys
models = {m["model"]: m for m in json.load(open(sys.argv[1]))}
keep = []
for line in open(sys.argv[2]):
    r = json.loads(line)
    if r.get("check_kind") == "temporal":
        keep.append(models[r["id"]])
json.dump(keep, open(sys.argv[3], "w"))
print(len(keep), "temporal")
EOF
echo "start d2live $(date -u +%FT%TZ) $(python3 -c "import json,sys;print(len(json.load(open(sys.argv[1]))))" "$O/live.json") models" >> ~/chain.log
$P python3 ~/tools/dmeasure.py d2live "$O/live" "$O/live.json" >> ~/chain.log 2>&1
echo "end d2live rc=$? $(date -u +%FT%TZ)" >> ~/chain.log
touch "$O/DONE"
