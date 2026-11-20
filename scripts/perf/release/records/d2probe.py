#!/usr/bin/env python3
"""Build D2 liveness probe model lists (budget = max(60 s, 2.5 x D1 TLC time)).

d2probe.py <relperf-dir> <r2-part.json> ...

<relperf-dir> holds lists/propmodels.txt (node/propmodels.py output); each part
is a `dive-conformance r2 small` or `r2 large` output (its models' `tlc_ms` is
the median TLC time of the small repetitions, or the large first pass). Writes
lists/d2probe.json for node/d2-chain.sh.
"""
import json
import os
import sys

S = sys.argv[1]
props = open(os.path.join(S, "lists/propmodels.txt")).read().split()
tl, ok = {}, {}
for p in sys.argv[2:]:
    for m, r in json.load(open(p))["models"].items():
        if r.get("tlc_ms"):
            tl[m] = r["tlc_ms"]
        ok[m] = r["cat"] == "agreeing"
out, missing = [], []
for m in props:
    if m in tl and ok.get(m):
        out.append({"model": m, "budget_s": max(60, round(2.5 * tl[m] / 1000))})
    elif m in tl:
        print("not agreeing:", m)
    else:
        missing.append(m)
json.dump(out, open(os.path.join(S, "lists/d2probe.json"), "w"), indent=0)
print(len(out), "ready;", len(missing), "without TLC time yet:", " ".join(x.split("/")[-1] for x in missing))
