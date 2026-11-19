#!/usr/bin/env python3
"""Build D2 liveness probe model lists (budget = max(60 s, 2.5 x D1 TLC median)).

d2probe.py <relperf-dir> [shard-summary.json ...]

<relperf-dir> holds lists/propmodels.txt (node/propmodels.py output), lists/cls/small.txt
(records/shard.py output) and data/small-summary.json (r2drive.py small); writes
lists/d2probe.json for node/d2-chain.sh.
"""
import json
import os
import statistics
import sys

S = sys.argv[1]
props = open(os.path.join(S, "lists/propmodels.txt")).read().split()
small = set(open(os.path.join(S, "lists/cls/small.txt")).read().split())
sm = json.load(open(os.path.join(S, "data/small-summary.json")))
tl, ok = {}, {}
for rep in sm["reps"]:
    for m, r in rep["models"].items():
        if r.get("tlc_ms"):
            tl.setdefault(m, []).append(r["tlc_ms"])
        ok.setdefault(m, True)
        ok[m] = ok[m] and r.get("core_outcome") in ("agree", "expected-divergence")
for p in sys.argv[2:]:
    sh = json.load(open(p))
    for m, r in sh.get("first_pass", {}).items():
        if r.get("tlc_ms"):
            tl.setdefault(m, []).append(r["tlc_ms"])
        ok[m] = r.get("core_outcome") in ("agree", "expected-divergence") or any(
            x.get("core_outcome") in ("agree", "expected-divergence") for x in sh.get("pairs", {}).get(m, []))
out, missing = [], []
for m in props:
    if m in tl and ok.get(m):
        out.append({"model": m, "budget_s": max(60, round(2.5 * statistics.median(tl[m]) / 1000))})
    elif m in tl:
        print("not agreeing:", m)
    else:
        missing.append(m)
json.dump(out, open(os.path.join(S, "lists/d2probe.json"), "w"), indent=0)
print(len(out), "ready;", len(missing), "without TLC time yet:", " ".join(x.split("/")[-1] for x in missing))
