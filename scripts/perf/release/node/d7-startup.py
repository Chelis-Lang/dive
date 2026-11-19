#!/usr/bin/env python3
"""d7-startup.py <dive-bin> <out.json>: 50 fresh runs each of dive --version, dive parse and
dive check of docs/user/examples/Counter.tla (run from the repository root); median, p90, max ms."""
import json, subprocess, sys, time, statistics, os
dive = sys.argv[1]; out = sys.argv[2]
cases = {"version": [dive, "--version"],
         "parse": [dive, "parse", "docs/user/examples/Counter.tla"],
         "check": [dive, "check", "docs/user/examples/Counter.tla", "--config", "docs/user/examples/Counter.cfg",
                   "--workers", "1", "--seed", "0", "--fp", "0", "--output", "json"]}
res = {}
for k, argv in cases.items():
    ts = []
    for i in range(50):
        t = time.perf_counter(); p = subprocess.run(argv, capture_output=True); ts.append((time.perf_counter() - t) * 1000)
        assert p.returncode == 0, (k, p.returncode)
    ts.sort()
    res[k] = {"n": 50, "median_ms": round(statistics.median(ts), 2), "p90_ms": round(ts[44], 2), "max_ms": round(ts[-1], 2)}
json.dump(res, open(out, "w"), indent=1); print(res)
