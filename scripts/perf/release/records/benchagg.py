#!/usr/bin/env python3
"""Aggregate dive-bench runs.jsonl per (bench, impl): RSS median/min/max, wall median/range, correctness.

benchagg.py <runs.jsonl>  -> JSON on stdout
"""
import json
import statistics
import sys

g = {}
for line in open(sys.argv[1]):
    r = json.loads(line)
    k = (r["bench"], r["impl"])
    pp = (r.get("run") or {}).get("physical_process") or {}
    g.setdefault(k, []).append({
        "rep": r.get("rep"), "rss": r.get("max_rss_bytes"), "wall": pp.get("elapsed_seconds"),
        "timed_out": pp.get("timed_out"), "exit": pp.get("exit_code"), "correct": r.get("correct"),
        "distinct": r.get("distinct"), "note": r.get("note"),
        "workers_used": r.get("workers_used") or (r.get("report") or {}).get("workers_used"),
        "sequential_reason": r.get("sequential_reason")})
out = []
for (b, impl), rs in sorted(g.items()):
    rss = [x["rss"] for x in rs if x["rss"]]
    w = [x["wall"] for x in rs if x["wall"] is not None]
    out.append({"bench": b, "impl": impl, "reps": len(rs),
                "rss_median": statistics.median(rss) if rss else None, "rss_min": min(rss) if rss else None,
                "rss_max": max(rss) if rss else None,
                "wall_s_median": round(statistics.median(w), 3) if w else None,
                "wall_s_range": [round(min(w), 3), round(max(w), 3)] if w else None,
                "all_correct": all(x["correct"] for x in rs), "timed_out": sum(1 for x in rs if x["timed_out"]),
                "notes": sorted({x["note"] for x in rs if x["note"]}),
                "workers_used": sorted({json.dumps(x["workers_used"]) for x in rs}),
                "sequential_reason": sorted({str(x["sequential_reason"]) for x in rs if x["sequential_reason"]})})
print(json.dumps(out, indent=0))
