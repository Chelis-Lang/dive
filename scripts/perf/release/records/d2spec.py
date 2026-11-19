#!/usr/bin/env python3
"""D2 spec for mkrecord.py.

d2spec.py <bench-agg.json> <bench-set-sha256> <raw-sha256> <out-spec.json> <live-rows.json>...
"""
import json
import sys

agg_p, set_sha, raw, out = sys.argv[1:5]
agg = json.load(open(agg_p))
by = {(r["bench"], r["impl"]): r for r in agg}
bench = []
for b in sorted({r["bench"] for r in agg}):
    for w in (1, 8):
        t = by.get((b, f"tlc-w{w}"))
        for prof in ("default", "tlc"):
            d = by.get((b, f"dive-{prof}-w{w}"))
            if not d:
                continue
            bench.append({"model": b, "profile": prof, "workers": w, "reps": d["reps"],
                          "dive_rss_median": d["rss_median"], "dive_rss_min": d["rss_min"], "dive_rss_max": d["rss_max"],
                          "dive_wall_ms_median": round(d["wall_s_median"] * 1000, 1),
                          "dive_wall_ms_range": [round(x * 1000, 1) for x in d["wall_s_range"]],
                          "tlc_rss_median": t["rss_median"] if t else None,
                          "tlc_rss_range": [t["rss_min"], t["rss_max"]] if t else None,
                          "tlc_wall_ms_median": round(t["wall_s_median"] * 1000, 1) if t else None,
                          "all_correct": d["all_correct"] and (t["all_correct"] if t else True),
                          "workers_used": d["workers_used"]})
live = {}
for p in sys.argv[5:]:
    live.update(json.load(open(p)))
spec = {
    "measurement": "D2", "anchor": "d2-memory",
    "command": {
        "benchmark_set": "dive-bench --dive-bin ~/bins/dive --dive-revision 08a14fbac83ff71a3247623fb10bf0584bc83be5 "
                         "--profile both --workers 1,8 --reps 5 --tlc --timeout 900 --xmx 3g (crates/dive-bench/benchmarks.toml)",
        "liveness": "dive check <module.tla> --config <model.cfg> --workers 1 --seed 0 --fp 0 --time-limit <max(60s, 2.5 x D1 TLC "
                    "time)> --max-memory 64g --output json under GNU /usr/bin/time -v; 3 repetitions"},
    "benchmark_set_sha256": set_sha,
    "tlc_memory_note": "TLC peak RSS depends on -Xmx3g (dive-bench --xmx 3g); it is the JVM's footprint, not TLC's minimum.",
    "liveness_selection": "Every R1 agreeing model whose config has PROPERTY or PROPERTIES was run once (the probe, at the same "
                          "budget); those whose report says check_kind temporal were kept. Refinement-only and safety-reducible "
                          "properties report safety and drop out.",
    "raw": {"location": "dive evidence archive bucket (us-east-1), prefix release-perf/2026-11-18/small-node-raw.tar.gz",
            "sha256": raw},
    "deviations": ["Release binary built on one bench node and copied to the others (independent builds were not "
                   "byte-identical); dive_binary_sha256 names the copied binary."],
    "summary": {
        "benchmark_set_cells": len(bench), "benchmark_set_all_correct": all(r["all_correct"] for r in bench),
        "liveness_models": len(live), "liveness_all_agree": all(r["agrees_with_golden"] for r in live.values()),
        "liveness_stopped": sorted(m for m, r in live.items() if r["stopped"]),
        "liveness_max_peak_rss": max((r["peak_rss_bytes"]["median"], m) for m, r in live.items()) if live else None},
    "rows": {"benchmark_set": bench, "liveness": dict(sorted(live.items()))},
}
json.dump(spec, open(out, "w"), indent=1)
