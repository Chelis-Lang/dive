#!/usr/bin/env python3
"""Markdown table for the D4 record. d4table.py <record.json>"""
import json, sys
d = json.load(open(sys.argv[1]))
def s(c):
    if c.get("unsupported"):
        return "refused"
    v = f"{c['wall_ms_median']/1000:,.1f}"
    return v + (" (stopped)" if c["stopped"] else "")
print("| Model | 1 worker s | 4 workers s | 8 workers s | Speed-up 4 | Speed-up 8 | Note |\n|---|---:|---:|---:|---:|---:|---|")
rows = sorted(d["rows"].items(), key=lambda kv: -(kv[1]["w1"]["wall_ms_median"] or 0))
for m, r in rows:
    notes = []
    if r["w1"].get("registered_divergence"):
        notes.append("1 worker: registered divergence")
    if r["w4"].get("unsupported"):
        notes.append("refused at 4 and 8 workers (#1020)")
    if r["w1"]["stopped"]:
        notes.append("1 worker stopped at the 3570 s limit; speed-ups are lower bounds")
    sp = lambda w: "" if r.get(f"speedup{w}") is None else (("≥" if r.get(f"speedup{w}_is_lower_bound") else "") + f"{r[f'speedup{w}']:.2f}")
    print(f"| `{m.replace('specifications/', '').replace('.cfg', '')}` | {s(r['w1'])} | {s(r['w4'])} | {s(r['w8'])} | {sp(4)} | {sp(8)} | {'; '.join(notes)} |")
