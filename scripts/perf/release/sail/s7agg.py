#!/usr/bin/env python3
"""s7agg.py <raw-dir>: quick console summary of s7-*.jsonl rows while a chain runs (not the record)."""
import json, sys, statistics as st, glob, os
D = sys.argv[1]
def med(xs): xs=[x for x in xs if x is not None]; return round(st.median(xs),1) if xs else None
for f in sorted(glob.glob(os.path.join(D, "s7-latency-*.jsonl"))):
    lab = f.rsplit("-",1)[1][:-6]; rows=[json.loads(l) for l in open(f)]
    print("==", lab)
    for name in dict.fromkeys(r["fixture"] for r in rows):
        rs=[r for r in rows if r["fixture"]==name]
        cells=[]
        for p in ["tlc","dive","embedded","execute","serve","mcp"]:
            w=[r[p]["wall_ms"] for r in rs]; ok=all(r[p].get("agrees") for r in rs)
            cells.append(f"{p}={med(w)}[{min(w):.0f}-{max(w):.0f}]{'' if ok else '!'}")
        t=med([r["tlc"]["wall_ms"] for r in rs])
        print(f"{name:28s}", " ".join(cells), "| x(tlc/exec)=%.1f x(tlc/mcp)=%.1f" % (t/med([r["execute"]["wall_ms"] for r in rs]), t/med([r["mcp"]["wall_ms"] for r in rs])))
for f in sorted(glob.glob(os.path.join(D, "s7-loop-*.jsonl"))):
    lab = f.rsplit("-",1)[1][:-6]; rows=[json.loads(l) for l in open(f)]
    print("== loop", lab, len(rows))
    for p in ["tlc","dive","execute","serve","mcp"]:
        print(f"  {p:8s} total={med([r[p]['total_ms'] for r in rows])} med={med([r[p]['median_ms'] for r in rows])} p90={med([r[p]['p90_ms'] for r in rows])} bad={sum(r[p]['disagreements'] for r in rows)}")
for f in sorted(glob.glob(os.path.join(D, "s7-through-*.jsonl"))):
    lab = f.rsplit("-",1)[1][:-6]; rows=[json.loads(l) for l in open(f)]
    print("== through", lab, len(rows))
    for k in rows[0]:
        if isinstance(rows[0][k], dict): print(f"  {k} jobs/s={med([r[k]['jobs_per_s'] for r in rows])} bad={sum(r[k]['disagreements'] for r in rows)}")
