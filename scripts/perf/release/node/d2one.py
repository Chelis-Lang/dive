#!/usr/bin/env python3
"""d2one.py <out> <models.json>: like dmeasure.py d2live, but each entry may name "reps" (default [1, 2, 3])."""
import json, os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import dmeasure, runjobs
out = os.path.abspath(sys.argv[1]); os.makedirs(out, exist_ok=True)
jobs = []
for m in json.load(open(sys.argv[2])):
    mod = dmeasure.module_of(m["model"])
    for rep in m.get("reps", [1, 2, 3]):
        jobs.append({"id": f"{m['model']}|{rep}", "cwd": dmeasure.EX,
                     "argv": dmeasure.check(mod, m["model"], "--time-limit", f"{m['budget_s']}s", "--max-memory", "64g")})
runjobs.run_jobs(jobs, out)
