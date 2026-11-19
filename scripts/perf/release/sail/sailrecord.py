#!/usr/bin/env python3
"""Compact Sail S1-S5 record from sail_temporal.py JSONL rows.

sailrecord.py <dir-with-s1..s5.jsonl> <meta.json> <out.json>
"""
import json
import os
import statistics
import sys

D, META, OUT = sys.argv[1:4]
meta = json.load(open(META))


def load(s):
    p = os.path.join(D, f"{s}.jsonl")
    return [json.loads(l) for l in open(p)] if os.path.exists(p) else []


def med(xs):
    xs = [x for x in xs if isinstance(x, (int, float))]
    return round(statistics.median(xs), 2) if xs else None


def p90(xs):
    xs = sorted(x for x in xs if isinstance(x, (int, float)))
    return round(xs[max(0, int(round(0.9 * len(xs))) - 1)], 2) if xs else None


rec = dict(meta)
s1 = {}
for r in load("s1"):
    s1.setdefault(r["fixture"], []).append(r)
rec["s1"] = {}
for f, rs in s1.items():
    ph = [r.get("phases", {}) for r in rs]
    cli = med(r["cli"]["wall_ms"] for r in rs)
    se = med(r["sail_execute"]["wall_ms"] for r in rs)
    emb = med(p.get("embedded_ms") for p in ph)
    rec["s1"][f] = {
        "kind": rs[0]["kind"], "reps": len(rs),
        "cli_ms": cli, "cli_range_ms": [min(r["cli"]["wall_ms"] for r in rs), max(r["cli"]["wall_ms"] for r in rs)],
        "cli_verdict": rs[0]["cli"].get("verdict"),
        "embedded_job_ms": emb, "sail_runtime_execute_ms": se,
        "sail_execute_range_ms": [min(r["sail_execute"]["wall_ms"] for r in rs), max(r["sail_execute"]["wall_ms"] for r in rs)],
        "difference_ms": round(se - cli, 1) if se is not None and cli is not None else None,
        "ratio_vs_cli": round(se / cli, 3) if se and cli else None,
        "phases_ms": {k: med(p.get(k) for p in ph) for k in
                      ["verify_copy_ms", "spawn_ms", "spawn_exit_ms", "encode_ms", "dive_call_ms", "worker_body_ms",
                       "decode_validate_ms", "embedded_ms"]},
        "outcome": sorted({str(r["sail_execute"].get("outcome")) for r in rs}),
        "execution": sorted({str(r["sail_execute"].get("execution")) for r in rs}),
        "completion": sorted({str(r["sail_execute"].get("completion")) for r in rs}),
        "distinct": rs[0]["sail_execute"].get("distinct"), "depth": rs[0]["sail_execute"].get("depth"),
        "embedded_outcome": sorted({str(p.get("outcome"))[:160] for p in ph}),
    }
rec["s2"] = {}
for r in load("s2"):
    k = f'{r["fixture"]}|{r["path"]}|{r["state"]}'
    rec["s2"].setdefault(k, []).append(r)
for k, rs in list(rec["s2"].items()):
    row = {"reps": len(rs), "median_ms": med(r["wall_ms"] for r in rs), "p90_ms": p90(r["wall_ms"] for r in rs),
           "outcomes": sorted({json.dumps(r.get("outcome")) for r in rs})}
    if "mcp_init_ms" in rs[0]:
        row["mcp_init_median_ms"] = med(r["mcp_init_ms"] for r in rs)
        row["is_error"] = sorted({str(r.get("is_error")) for r in rs})
    if "daemon_socket_ms" in rs[0]:
        row["daemon_start_median_ms"] = med(r["daemon_socket_ms"] for r in rs)
    rec["s2"][k] = row
rec["s3"] = {}
for r in load("s3"):
    rec["s3"].setdefault(r["fixture"], []).append(r)
for k, rs in list(rec["s3"].items()):
    if "jobs" in rs[0]:
        rec["s3"][k] = rs[0]
        continue
    rec["s3"][k] = {"reps": len(rs), "wall_ms": med(r["wall_ms"] for r in rs),
                    "wall_range_ms": [min(r["wall_ms"] for r in rs), max(r["wall_ms"] for r in rs)],
                    "tree_peak_kb": med(r.get("tree_peak_kb") for r in rs),
                    "outcome": sorted({str(r.get("outcome")) for r in rs}),
                    "execution": sorted({str(r.get("execution")) for r in rs}),
                    "completion": sorted({str(r.get("completion")) for r in rs}),
                    "distinct": rs[0].get("distinct"), "depth": rs[0].get("depth")}
s4 = load("s4")
rec["s4"] = {"seed": next((r["seed"] for r in s4 if "seed" in r), None)}
groups = {}
for r in s4:
    if "case" not in r:
        continue
    k = r["case"] + (f'|{r["deadline_ms"]}' if "deadline_ms" in r else "")
    groups.setdefault(k, []).append(r)
for k, rs in groups.items():
    row = {"reps": len(rs)}
    for f in ["after_deadline_ms", "exit_ms", "reaped_ms", "reaped_after_exit_ms", "admitted_ms", "worker_start_ms"]:
        if any(f in r for r in rs):
            row[f] = {"median": med(r.get(f) for r in rs), "max": max((r.get(f) for r in rs if isinstance(r.get(f), (int, float))), default=None),
                      "missing": sum(1 for r in rs if not isinstance(r.get(f), (int, float)))}
    for f in ["execution", "third_execution"]:
        if any(f in r for r in rs):
            row[f] = sorted({str(r.get(f)) for r in rs})
    rec["s4"][k] = row
rec["s5"] = load("s5")
json.dump(rec, open(OUT, "w"), indent=1)
print(os.path.getsize(OUT))
