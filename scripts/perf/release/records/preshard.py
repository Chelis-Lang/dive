#!/usr/bin/env python3
"""preshard.py <repo> <out>: LPT shards of the 2026-11-10 large class on golden TLC times."""
import json
import sys

repo, out = sys.argv[1:3]
d = json.load(open(f"{repo}/conformance/reports/perf/2026-11-10-r2-procedure-e3835968.json"))["models"]
large = [m for m, v in d.items() if v["cls"] == "large"]
est = {m: json.load(open(f"{repo}/conformance/golden/tlc-2026.10.06.014338/examples/{m}.json"))["provenance"]["elapsed_ms"]
       for m in large}
sh = [[] for _ in range(4)]
load = [0] * 4
for m in sorted(large, key=lambda m: -est[m]):
    i = load.index(min(load))
    sh[i].append(m)
    load[i] += est[m]
for i, s in enumerate(sh):
    open(f"{out}/shard{i+1}.txt", "w").write("\n".join(s) + "\n")
print([len(s) for s in sh], [round(x / 1000) for x in load])
json.dump({"rule": "LPT on golden TLC elapsed_ms over the 2026-11-10 large class",
           "shards": {f"shard{i+1}": s for i, s in enumerate(sh)}, "golden_load_s": [round(x / 1000) for x in load]},
          open(f"{out}/sharding.json", "w"), indent=1)
