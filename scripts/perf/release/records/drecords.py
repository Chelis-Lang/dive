#!/usr/bin/env python3
"""Compact per-model rows for D2 (liveness), D3, D5, D6 from runjobs rows.jsonl.

drecords.py d3|d5a|d5b|d6|d2live <rows.jsonl> <golden-dir>  -> JSON on stdout
"""
import json
import os
import statistics
import sys

mode, rows_path, gold = sys.argv[1:4]
rows = [json.loads(l) for l in open(rows_path)]
MODMAP = {"COFFEE_SAFETY": "specifications/CoffeeCan/CoffeeCan1000Beans.cfg"}


def golden(cfg):
    cfg = MODMAP.get(cfg, cfg)
    g = json.load(open(os.path.join(gold, cfg + ".json")))["result"]
    return {"verdict": g["verdict"], "distinct": g["counts"]["distinct_states"], "depth": g["counts"]["depth"],
            "generated": g["counts"]["states_generated"]}


def med(xs):
    xs = [x for x in xs if x is not None]
    return round(statistics.median(xs), 2) if xs else None


def rng(xs):
    xs = [x for x in xs if x is not None]
    return [min(xs), max(xs)] if xs else None


def agrees(r, g):
    v = {"success": "success"}.get(r.get("verdict"), r.get("verdict"))
    ok_v = (v == "success") == (g["verdict"] == "success")
    return ok_v and r.get("distinct") == g["distinct"] and r.get("depth") == g["depth"]


def group(key):
    out = {}
    for r in rows:
        out.setdefault(key(r), []).append(r)
    return out


res = {}
if mode == "d3":
    for (cfg, store), rs in sorted(group(lambda r: tuple(r["id"].split("|")[:2])).items()):
        g = golden(cfg)
        res.setdefault(cfg, {})[store] = {
            "reps": len(rs), "wall_ms": med(r["wall_ms"] for r in rs), "wall_range_ms": rng(r["wall_ms"] for r in rs),
            "peak_rss_bytes": med(r["peak_rss_bytes"] for r in rs), "rss_range": rng(r["peak_rss_bytes"] for r in rs),
            "options_store": sorted({r.get("store") for r in rs}, key=str), "check_kind": rs[0].get("check_kind"),
            "verdict": rs[0].get("verdict"), "distinct": rs[0].get("distinct"), "depth": rs[0].get("depth"),
            "agrees_with_golden": all(agrees(r, g) for r in rs)}
    for cfg, v in res.items():
        if "fp" in v and "exact" in v and v["fp"]["wall_ms"] and v["fp"]["peak_rss_bytes"]:
            v["exact_over_fp"] = {"wall": round(v["exact"]["wall_ms"] / v["fp"]["wall_ms"], 3),
                                  "rss": round(v["exact"]["peak_rss_bytes"] / v["fp"]["peak_rss_bytes"], 3)}
elif mode == "d2live":
    for cfg, rs in sorted(group(lambda r: r["id"].split("|")[0]).items()):
        g = golden(cfg)
        d = rs[0].get("distinct")
        rss = med(r["peak_rss_bytes"] for r in rs)
        st = [r.get("stopped") for r in rs if r.get("stopped")]
        res[cfg] = {"model": cfg, "workers": 1, "store": rs[0].get("store"), "reps": len(rs),
                    "check_kind": rs[0].get("check_kind"),
                    "peak_rss_bytes": {"median": rss, "min": rng(r["peak_rss_bytes"] for r in rs)[0],
                                       "max": rng(r["peak_rss_bytes"] for r in rs)[1]},
                    "observed_max_bytes": med(r.get("observed_max_bytes") for r in rs),
                    "known_retained_lb": med(r.get("known_retained_lb") for r in rs),
                    "distinct": d, "wall_ms": med(r["wall_ms"] for r in rs),
                    "bytes_per_state": round(rss / d, 1) if rss and d else None,
                    "agrees_with_golden": all(agrees(r, g) for r in rs),
                    "stopped": [{"phase": s.get("phase"), "resource": s.get("resource")} for s in st] or None}
elif mode == "d5a":
    for kind, rs in group(lambda r: r["id"].split("|")[0] if not r["id"].startswith("simulate") else r["id"].rsplit("|", 1)[0]).items():
        w = sorted(r["wall_ms"] for r in rs)
        row = {"runs": len(rs), "median_ms": med(w), "p90_ms": round(w[int(0.9 * len(w)) - 1], 2) if len(w) >= 10 else None,
               "max_ms": max(w), "rc": sorted({r["rc"] for r in rs})}
        if kind == "check" or kind.startswith("simulate"):
            row["peak_rss_bytes"] = med(r["peak_rss_bytes"] for r in rs)
        if kind.startswith("simulate"):
            row["result"] = sorted({str(r.get("result") or r.get("verdict")) for r in rs})
            row["raw_stats"] = rs[0].get("summary") or rs[0].get("stats")
        res[kind] = row
elif mode == "d5b":
    for kind, rs in group(lambda r: r["id"]).items():
        w = sorted(r["wall_ms"] for r in rs)
        res[kind] = {"runs": len(rs), "median_ms": med(w), "max_ms": max(w), "total_ms": round(sum(w), 1),
                     "results": {str(k): sum(1 for r in rs if str(r.get("result")) == str(k)) for k in {r.get("result") for r in rs}},
                     "rc": sorted({r["rc"] for r in rs})}
elif mode == "d6":
    for key, rs in sorted(group(lambda r: r["id"].rsplit("|", 1)[0]).items()):
        res[key] = {"reps": len(rs), "wall_ms": [r["wall_ms"] for r in rs], "rc": [r["rc"] for r in rs],
                    "stopped": [(r.get("stopped") or {}).get("phase") for r in rs],
                    "stopped_elapsed_ms": [(r.get("stopped") or {}).get("elapsed_ms") for r in rs],
                    "stopped_resource": [(r.get("stopped") or {}).get("resource") for r in rs],
                    "stopped_observed": [(r.get("stopped") or {}).get("observed") for r in rs],
                    "verdict": [r.get("verdict") for r in rs], "distinct": [r.get("distinct") for r in rs],
                    "depth": [r.get("depth") for r in rs], "elapsed_ms": [r.get("elapsed_ms") for r in rs]}
print(json.dumps(res, indent=1))
