#!/usr/bin/env python3
"""Compact dive-bench runs.jsonl: one line per run with the fields the records need.

benchruns.py <runs.jsonl> [--keys]   (--keys prints the key layout of the first row)
"""
import json
import sys

rows = open(sys.argv[1])
if "--keys" in sys.argv:
    r = json.loads(rows.readline())

    def walk(o, p=""):
        if isinstance(o, dict):
            for k, v in o.items():
                walk(v, f"{p}.{k}" if p else k)
        else:
            s = json.dumps(o)
            print(p, s[:80])
    walk(r)
    sys.exit()
for line in rows:
    r = json.loads(line)
    keep = {}
    for k in ("bench", "arm", "rep", "tool", "profile", "workers", "workers_requested", "workers_used", "outcome",
              "status", "wall_s", "cpu_s", "max_rss_bytes", "rss_bytes", "distinct", "generated", "depth",
              "sequential_reason", "verdict", "exit_code"):
        if k in r:
            keep[k] = r[k]
    for k in ("statistics", "measurement", "result", "observed"):
        if isinstance(r.get(k), dict):
            for kk, vv in r[k].items():
                if not isinstance(vv, (dict, list)) and kk not in keep:
                    keep[f"{kk}"] = vv
    print(json.dumps(keep))
