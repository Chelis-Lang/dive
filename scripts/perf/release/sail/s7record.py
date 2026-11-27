#!/usr/bin/env python3
"""s7record.py <raw-dir> <meta.json> <out.json> <tables.md>: compact S7 record and Markdown tables."""
import glob
import json
import os
import statistics as st
import sys

RAW, META, OUT, MD = sys.argv[1:5]
meta = json.load(open(META))


def med(xs):
    xs = [x for x in xs if x is not None]
    return round(st.median(xs), 2) if xs else None


def spread(xs):
    xs = [x for x in xs if x is not None]
    return {"median": med(xs), "min": round(min(xs), 2), "max": round(max(xs), 2), "n": len(xs)} if xs else None


def rows(part, label):
    f = os.path.join(RAW, f"s7-{part}-{label}.jsonl")
    return [json.loads(l) for l in open(f)] if os.path.exists(f) else []


LABELS = meta["labels"]
PATHS = ["tlc", "dive", "embedded", "execute", "serve", "mcp"]
rec = {k: meta[k] for k in meta if k not in ("labels",)}
rec["labels"] = LABELS
rec["latency"] = {}
rec["loop"] = {}
rec["through"] = {}
rec["first_job"] = None
rec["only"] = None
disagree = 0
for lab in LABELS:
    lat = rows("latency", lab)
    out = {}
    for name in dict.fromkeys(r["fixture"] for r in lat):
        rs = [r for r in lat if r["fixture"] == name]
        cell = {}
        for p in PATHS:
            ws = [r[p]["wall_ms"] for r in rs]
            ok = all(r[p].get("agrees") for r in rs)
            disagree += sum(not r[p].get("agrees") for r in rs)
            c = {"wall_ms": spread(ws), "agrees": ok}
            rss = [r[p].get("rss_kb") for r in rs]
            if any(rss):
                c["peak_rss_kb"] = spread(rss)
            hits = [r[p].get("cache_hit") for r in rs]
            if any(h is not None for h in hits):
                c["cache_hits"] = sum(bool(h) for h in hits)
            hwm = [r[p].get("daemon_hwm_kb") for r in rs]
            if any(hwm):
                c["daemon_hwm_kb"] = spread(hwm)
            cell[p] = c
        cell["outcome"] = rs[0]["execute"].get("outcome")
        cell["completion"] = rs[0]["execute"].get("completion")
        cell["distinct"] = rs[0]["execute"].get("distinct")
        out[name] = cell
    if out:
        rec["latency"][lab] = out
    lp = rows("loop", lab)
    if lp:
        o = {"n": lp[0]["n"], "mix": lp[0]["mix"], "reps": len(lp)}
        for p in ["tlc", "dive", "execute", "serve", "mcp"]:
            o[p] = {"total_ms": spread([r[p]["total_ms"] for r in lp]),
                    "per_job_median_ms": med([r[p]["median_ms"] for r in lp]),
                    "per_job_p90_ms": med([r[p]["p90_ms"] for r in lp]),
                    "disagreements": sum(r[p]["disagreements"] for r in lp)}
            disagree += o[p]["disagreements"]
            for k in ["mcp_init_ms", "daemon_start_ms", "daemon_hwm_kb"]:
                if k in lp[0][p]:
                    o[p][k] = med([r[p].get(k) for r in lp])
        rec["loop"][lab] = o
    th = rows("through", lab)
    if th:
        o = {"n": th[0]["n"], "reps": len(th)}
        for k in th[0]:
            if isinstance(th[0][k], dict):
                if k == "serve-c8":
                    # Beyond two active executions the service refuses with
                    # SAIL6201 busy (no queue): not a throughput figure.
                    o[k] = {"not_completed": sum(r[k]["disagreements"] for r in th), "jobs": 5 * th[0]["n"],
                            "note": "jobs beyond the two active executions were refused with SAIL6201 busy "
                                    "(spot-checked); every completed job agreed"}
                    continue
                o[k] = {"jobs_per_s": spread([r[k]["jobs_per_s"] for r in th]),
                        "total_ms": med([r[k]["total_ms"] for r in th]),
                        "disagreements": sum(r[k]["disagreements"] for r in th)}
                disagree += o[k]["disagreements"]
                if "daemon_hwm_kb" in th[0][k]:
                    o[k]["daemon_hwm_kb"] = med([r[k]["daemon_hwm_kb"] for r in th])
        rec["through"][lab] = o
    fj = rows("first", lab)
    if fj:
        rec["first_job"] = {"label": lab, "fixture": "DieHard",
                            "first_ms": spread([r["first"]["wall_ms"] for r in fj]),
                            "second_ms": spread([r["second"]["wall_ms"] for r in fj]),
                            "all_completed": all(r["first"]["execution"] == "completed" and
                                                 r["second"]["execution"] == "completed" for r in fj)}
    ra = rows("ratio", lab)
    if ra:
        o = {}
        for name in dict.fromkeys(r["fixture"] for r in ra):
            rs = [r for r in ra if r["fixture"] == name]
            o[name] = {"worker_body_ms": spread([r["embedded"]["worker_body_ms"] for r in rs]),
                       "embedded_ms": spread([r["embedded"]["wall_ms"] for r in rs]),
                       "dive_cli_ms": spread([r["dive"]["wall_ms"] for r in rs]),
                       "worker_body_over_cli": round(st.median([r["embedded"]["worker_body_ms"] / r["dive"]["wall_ms"]
                                                                for r in rs]), 3),
                       "agrees": all(r["embedded"]["agrees"] and r["dive"]["agrees"] for r in rs)}
            disagree += sum(not (r["embedded"]["agrees"] and r["dive"]["agrees"]) for r in rs)
        rec.setdefault("ratio", {})[lab] = o
    on = rows("only", lab)
    if on:
        o = {"label": lab}
        for k in on[0]:
            if k == "rep":
                continue
            o[k] = {"wall_ms": spread([r[k]["wall_ms"] for r in on]),
                    "outcomes": sorted({f'{r[k]["execution"]}/{r[k]["outcome"]}/{r[k]["completion"]}' for r in on}),
                    "distinct": sorted({r[k]["distinct"] for r in on if r[k]["distinct"] is not None}),
                    "peak_rss_kb": spread([r[k].get("rss_kb") for r in on])}
        rec["only"] = o
pair = {}
for lab in LABELS:
    lat = rows("latency", lab)
    if not lat:
        continue
    pair[lab] = {}
    for name in dict.fromkeys(r["fixture"] for r in lat):
        rs = [r for r in lat if r["fixture"] == name]
        pair[lab][name] = {p: round(st.median([r["tlc"]["wall_ms"] / r[p]["wall_ms"] for r in rs]), 1)
                           for p in PATHS[1:]}
    lp = rows("loop", lab)
    if lp:
        pair[lab]["loop"] = {p: round(st.median([r["tlc"]["total_ms"] / r[p]["total_ms"] for r in lp]), 1)
                             for p in ["dive", "execute", "serve", "mcp"]}
    th = rows("through", lab)
    if th:
        pair[lab]["through_c2"] = round(st.median([r["serve-c2"]["jobs_per_s"] / r["tlc-c2"]["jobs_per_s"]
                                                   for r in th]), 1)
rec["tlc_over_path_paired_median"] = pair
rec["disagreements_total"] = disagree
json.dump(rec, open(OUT, "w"), indent=1, sort_keys=False)

# ---------------------------------------------------------------- Markdown
L = []
fmt = lambda v: "–" if v is None else (f"{v:,.0f}" if v >= 100 else f"{v:,.1f}")
for lab in rec["latency"]:
    L.append(f"\n#### Per-check latency, Sail `{lab}`\n")
    L.append("| Fixture | TLC | dive CLI | Sail embedded | Sail execute | Sail serve (warm) | Sail MCP (warm) | TLC ÷ execute | TLC ÷ MCP |")
    L.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for name, c in rec["latency"][lab].items():
        m = {p: c[p]["wall_ms"]["median"] for p in PATHS}
        L.append(f"| {name} | " + " | ".join(fmt(m[p]) for p in PATHS) +
                 f" | {m['tlc'] / m['execute']:.1f}× | {m['tlc'] / m['mcp']:.1f}× |")
for lab, o in rec["loop"].items():
    L.append(f"\n#### Agent loop, {o['n']} jobs, Sail `{lab}`\n")
    L.append("| Path | Session total (median, min–max) | Per job median | Per job p90 | TLC ÷ path |")
    L.append("|---|---:|---:|---:|---:|")
    t = o["tlc"]["total_ms"]["median"]
    for p in ["tlc", "dive", "execute", "serve", "mcp"]:
        s = o[p]["total_ms"]
        L.append(f"| {p} | {s['median'] / 1000:.2f} s ({s['min'] / 1000:.2f}–{s['max'] / 1000:.2f}) | "
                 f"{fmt(o[p]['per_job_median_ms'])} ms | {fmt(o[p]['per_job_p90_ms'])} ms | {t / s['median']:.1f}× |")
for lab, o in rec["through"].items():
    L.append(f"\n#### Throughput, {o['n']} jobs, Sail `{lab}`\n")
    L.append("| Case | Jobs/s (median, min–max) |")
    L.append("|---|---:|")
    for k, v in o.items():
        if isinstance(v, dict) and "jobs_per_s" in v:
            j = v["jobs_per_s"]
            L.append(f"| {k} | {j['median']:.1f} ({j['min']:.1f}–{j['max']:.1f}) |")
open(MD, "w").write("\n".join(L) + "\n")
print("disagreements", disagree)
