#!/usr/bin/env python3
"""Print Markdown tables from the release records (for the results doc).

mktables.py <records-dir> <source8>
"""
import json
import os
import sys

R, S8 = sys.argv[1:3]
DATE = "2026-11-18"


def load(name):
    p = os.path.join(R, name)
    return json.load(open(p)) if os.path.exists(p) else None


def mb(x):
    return "" if x is None else f"{x / 2**20:,.0f}"


def s(x):
    return "" if x is None else f"{x / 1000:,.2f}"


def short(m):
    return m.replace("specifications/", "").replace(".cfg", "")


d1 = load(f"{DATE}-r2-procedure-{S8}.json")
if d1:
    print("### D1 models above 0.9x and the largest models\n")
    print("| Model | Class | Core / TLC | TLC s | Core s |\n|---|---|---:|---:|---:|")
    rows = sorted(d1["models"].items(), key=lambda kv: -(kv[1].get("ratio") or 0))
    for m, r in rows:
        if r["cat"] != "agreeing":
            continue
        if (r["ratio"] or 0) > 0.9 or (r.get("tlc_ms") or 0) > 600000:
            print(f"| `{short(m)}` | {r['cls']} | {r['ratio']:.3f} | {s(r.get('tlc_ms'))} | {s(r.get('core_ms'))} |")
    print()
d2 = load(f"{DATE}-release-d2-memory-{S8}.json")
if d2:
    print("### D2 benchmark set peak RSS (MiB, median of 5)\n")
    print("| Model | Profile | Workers | dive RSS | dive range | TLC RSS | dive s | TLC s |\n|---|---|---:|---:|---|---:|---:|---:|")
    for r in d2["rows"]["benchmark_set"]:
        print(f"| {r['model']} | {r['profile']} | {r['workers']} | {mb(r['dive_rss_median'])} | {mb(r['dive_rss_min'])}–{mb(r['dive_rss_max'])} | {mb(r.get('tlc_rss_median'))} | {s(r['dive_wall_ms_median'])} | {s(r.get('tlc_wall_ms_median'))} |")
    print()
    print("### D2 liveness capture RSS (median of 3)\n")
    print("| Model | Distinct | Peak RSS MiB | Observed max MiB | Store lower bound MiB | Bytes/state | Wall s | Agrees |\n|---|---:|---:|---:|---:|---:|---:|---|")
    for m, r in d2["rows"]["liveness"].items():
        print(f"| `{short(m)}` | {r['distinct'] or ''} | {mb(r['peak_rss_bytes']['median'])} | {mb(r['observed_max_bytes'])} | {mb(r['known_retained_lb'])} | {r['bytes_per_state'] or ''} | {s(r['wall_ms'])} | {'yes' if r['agrees_with_golden'] else (('registered ' + r['registered_divergence'].split('(')[1].split(')')[0]) if r.get('registered_divergence') else ('stopped' if r['stopped'] else 'NO'))} |")
    print()
d3 = load(f"{DATE}-release-d3-store-{S8}.json")
if d3:
    print("### D3 exact versus fingerprint store (median of 3)\n")
    print("| Model | fp s | exact s | time exact/fp | fp RSS MiB | exact RSS MiB | RSS exact/fp | Agrees |\n|---|---:|---:|---:|---:|---:|---:|---|")
    for m, r in d3["rows"].items():
        e = r.get("exact_over_fp") or {}
        ok = r.get("fp", {}).get("agrees_with_golden") and r.get("exact", {}).get("agrees_with_golden")
        print(f"| `{short(m)}` | {s(r['fp']['wall_ms'])} | {s(r['exact']['wall_ms'])} | {e.get('wall', '')} | {mb(r['fp']['peak_rss_bytes'])} | {mb(r['exact']['peak_rss_bytes'])} | {e.get('rss', '')} | {'yes' if ok else 'NO'} |")
    print()
d4 = load(f"{DATE}-release-d4-scaling-{S8}.json")
if d4:
    print("### D4 worker scaling (median of 3)\n")
    print("| Model | 1 worker s | 4 workers s | 8 workers s | speed-up 4 | speed-up 8 | used 8 | note |\n|---|---:|---:|---:|---:|---:|---:|---|")
    for m, r in d4["rows"].items():
        print(f"| `{short(m)}` | {s(r.get('w1_ms'))} | {s(r.get('w4_ms'))} | {s(r.get('w8_ms'))} | {r.get('speedup4') or ''} | {r.get('speedup8') or ''} | {r.get('used8') or ''} | {r.get('note') or ''} |")
    print()
