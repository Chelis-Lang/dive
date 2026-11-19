#!/usr/bin/env python3
"""Split the D1 selection into the small class and four LPT large shards.

shard.py <classify-summary.json> <golden-dir> <out-dir>
small: completed agreeing TLC time < 10 s in the classification pass.
large: everything else that TLC did not fail; sharded by longest processing
time on the golden TLC elapsed_ms (classification stops at 15 s).
"""
import json
import os
import sys

summ, gold, out = sys.argv[1:4]
rows = json.load(open(summ))["rows"]
small, large, other = [], [], {}
for m, r in sorted(rows.items()):
    if r["outcome"] in ("agree", "expected-divergence") and r["elapsed_ms"] is not None and r["elapsed_ms"] < 10000:
        small.append(m)
    elif r["outcome"] in ("agree", "expected-divergence", "timeout") or "timeout" in str(r["outcome"]):
        large.append(m)
    else:
        other[m] = r["outcome"]
est = {m: json.load(open(os.path.join(gold, m + ".json")))["provenance"]["elapsed_ms"] for m in large}
shards = [[] for _ in range(4)]
load = [0] * 4
for m in sorted(large, key=lambda m: -est[m]):
    i = load.index(min(load))
    shards[i].append(m)
    load[i] += est[m]
os.makedirs(out, exist_ok=True)
open(os.path.join(out, "small.txt"), "w").write("\n".join(small) + "\n")
for i, s in enumerate(shards):
    open(os.path.join(out, f"shard{i+1}.txt"), "w").write("\n".join(s) + "\n")
json.dump({"small": small, "shards": {f"shard{i+1}": s for i, s in enumerate(shards)},
           "shard_golden_load_s": [round(x / 1000) for x in load], "other": other},
          open(os.path.join(out, "classes.json"), "w"), indent=1)
print(len(small), [len(s) for s in shards], [round(x / 1000) for x in load], other)
