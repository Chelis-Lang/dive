#!/usr/bin/env python3
"""D3 spec for mkrecord.py.  d3spec.py <rows.json from drecords d3> <raw-sha256> <out-spec.json>"""
import json
import sys

rows, raw, out = sys.argv[1:4]
d = json.load(open(rows))
key = "CoffeeCan/CoffeeCan1000Beans (safety-only config: MaxBeanCount = 1000, INVARIANTS TypeInvariant)"
d = {(key if k == "COFFEE_SAFETY" else k): v for k, v in d.items()}
spec = {
    "measurement": "D3", "anchor": "d3-exact-versus-fingerprint-store",
    "command": "dive check <module.tla> --config <model.cfg> --workers 1 --seed 0 --fp 0 --store fp|exact "
               "--time-limit 1800s --max-memory 64g --output json under GNU /usr/bin/time -v; 3 repetitions, fp first "
               "on odd repetitions and exact first on even ones",
    "selection": "safety: the ten crates/dive-bench/benchmarks.toml models (CoffeeCan as the benchmark's safety-only "
                 "config); liveness: CoffeeCan1000Beans, ElevatorLivenessMedium, MCLiveInternalMemory, "
                 "MCFlashWithMutexLive, aba_asyn_byz",
    "previous_record": None,
    "raw": {"location": "dive evidence archive bucket (us-east-1), prefix release-perf/2026-11-18/small-node-raw.tar.gz",
            "sha256": raw},
    "deviations": ["Release binary built on one bench node and copied to the others (independent builds were not "
                   "byte-identical); dive_binary_sha256 names the copied binary."],
    "summary": {
        "models": len(d), "all_agree_under_both_stores": all(v["fp"]["agrees_with_golden"] and v["exact"]["agrees_with_golden"] for v in d.values()),
        "wall_exact_over_fp_range": [min(v["exact_over_fp"]["wall"] for v in d.values()), max(v["exact_over_fp"]["wall"] for v in d.values())],
        "rss_exact_over_fp_range": [min(v["exact_over_fp"]["rss"] for v in d.values()), max(v["exact_over_fp"]["rss"] for v in d.values())]},
    "rows": d,
}
json.dump(spec, open(out, "w"), indent=1)
