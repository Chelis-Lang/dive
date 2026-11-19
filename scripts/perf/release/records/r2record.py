#!/usr/bin/env python3
"""Build the D1 R2 record from the driver summaries.

r2record.py <data-dir> <out.json>
data-dir holds: classify-summary.json, small-summary.json, shard{1..4}-summary.json,
classes.json, meta.json (source, sha, host fields, raw archive digest).
"""
import json
import math
import statistics
import sys

D, OUT = sys.argv[1:3]
AGREE = ("agree", "expected-divergence")
meta = json.load(open(f"{D}/meta.json"))
classes = json.load(open(f"{D}/classes.json"))
small = json.load(open(f"{D}/small-summary.json"))
models = {}
stopped = []

for m in classes["small"]:
    reps, cats = [], []
    for rep in small["reps"]:
        r = rep["models"][m]
        cats.append(r["core_outcome"])
        reps.append(r["ratio"])
    ok = all(c in AGREE for c in cats) and all(x is not None for x in reps)
    cat = "agreeing" if ok else (f"excluded:{cats[0]}" if len(set(cats)) == 1 else "mixed:" + ",".join(sorted(set(map(str, cats)))))
    row = {"cls": "small", "cat": cat, "ratio": round(statistics.median(reps), 4) if ok else None,
           "reps": [x for x in reps if x is not None] if ok else []}
    if not ok and any(c in AGREE for c in cats):
        row["rep_outcomes"] = cats
    models[m] = row

for shard in ["shard1", "shard2", "shard3", "shard4"]:
    s = json.load(open(f"{D}/{shard}-summary.json"))
    for m, fp in s["first_pass"].items():
        row = {"cls": "large", "shard": shard}
        pairs = s.get("pairs", {}).get(m)
        if fp["core_outcome"] in AGREE and fp["tlc_outcome"] in AGREE:
            row["cat"] = "agreeing"
            row["ratio"] = fp["ratio"]
        else:
            row["cat"] = f"excluded:{fp['core_outcome']}" if fp["core_outcome"] not in ("timeout", "stopped") else "stopped"
            row["ratio"] = None
            row["core_detail"] = (fp.get("core_detail") or "")[:200] or None
        if pairs:
            row["first_pass"] = fp["ratio"]
            row["pairs"] = [p["ratio"] for p in pairs]
            row["pair_orders"] = ["-".join(p["order"]) for p in pairs]
            good = [p["ratio"] for p in pairs if p["ratio"] is not None and p["core_outcome"] in AGREE]
            if len(good) == 3:
                row["cat"] = "agreeing"
                row["ratio"] = round(statistics.median(good), 4)
        row["tlc_ms"] = fp["tlc_ms"]
        row["core_ms"] = fp["core_ms"]
        if row["cat"] == "stopped":
            stopped.append(m)
        models[m] = row

agree = {m: r["ratio"] for m, r in models.items() if r["cat"] == "agreeing"}
geo = math.exp(sum(math.log(x) for x in agree.values()) / len(agree))
fixed = []
for m, r in models.items():
    if r["cat"] == "agreeing":
        fixed.append(r["ratio"])
    elif r["cat"] == "stopped":
        lb = (r.get("core_ms") or 0) / r["tlc_ms"] if r.get("tlc_ms") else 2
        fixed.append(max(2, lb))
    elif r["cat"].startswith("excluded"):
        fixed.append(2)
fgeo = math.exp(sum(math.log(x) for x in fixed) / len(fixed))
rec = {
    "gate": "R2",
    "procedure": "docs/developer/performance.md#r2-measurement-procedure",
    "plan": "docs/developer/release-performance-plan.md#d1-fresh-r2",
    "source": meta["source"],
    "dive_binary_sha256": meta["dive_binary_sha256"],
    "harness_binary_sha256": meta["harness_binary_sha256"],
    "oracle": "tlc-2026.10.06.014338",
    "corpus": "examples@6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f --tier all",
    "host": meta["host"],
    "classification": meta["classification"],
    "sharding": meta["sharding"],
    "summary": {"agreeing": len(agree), "small": sum(1 for r in models.values() if r["cls"] == "small" and r["cat"] == "agreeing"),
                "large": sum(1 for r in models.values() if r["cls"] == "large" and r["cat"] == "agreeing"),
                "geomean": round(geo, 4), "fixed_set_geomean": round(fgeo, 4),
                "above_1x": {m: x for m, x in sorted(agree.items()) if x > 1},
                "above_2x": sorted(m for m, x in agree.items() if x > 2), "stopped": stopped,
                "median": round(statistics.median(agree.values()), 4),
                "min": round(min(agree.values()), 4), "max": round(max(agree.values()), 4),
                "gate_result": "pass" if geo < 1 and not any(x > 2 for x in agree.values()) and not stopped else "fail"},
    "not_timed": meta["not_timed"],
    "attempts": meta["attempts"],
    "raw": meta["raw"],
    "deviations": meta["deviations"],
    "models": dict(sorted(models.items())),
}
json.dump(rec, open(OUT, "w"), indent=1)
print(json.dumps(rec["summary"], indent=1))
