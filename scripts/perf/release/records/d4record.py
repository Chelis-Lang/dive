#!/usr/bin/env python3
"""D4 worker-scaling record from several dive-bench runs.jsonl files.

d4record.py <out.json> <raw.json> <set-sha.json> <node>:<keep-reps|rep=N>:<runs.jsonl>...
  keep-reps "1,2" keeps rows whose dive-bench rep is listed; "rep=3" relabels every row of a --reps 1 file as rep 3.
raw.json: {"location": ..., "files": {name: sha256}}; set-sha.json: {set-file: sha256}
"""
import json
import statistics
import sys
import os
import tomllib

REPO = os.environ.get("DIVE_REPO", os.path.join(os.path.dirname(os.path.abspath(__file__)), *[".."] * 4))

DIV = {}
for d in tomllib.load(open(os.path.join(REPO, "conformance", "expected-divergences.toml"), "rb"))["divergence"]:
    if d.get("profile") == "dive":
        DIV.setdefault(d["pattern"], []).append(d)


UNSUP = {m: "refused at more than one worker: temporal properties: concrete pool capture with effects or runtime names (core); #1020"
         for m in ("specifications/KeyValueStore/MCKVsnap.cfg", "specifications/SpanningTree/SpanTreeRandom.cfg")}


def registered(model, r):
    """Name the registered divergence a 1-worker success run matches exactly, or None."""
    if r.get("verdict") != "success" or r.get("exit_code") != 0:
        return None
    for d in DIV.get(model, []):
        if "generated_count_pair" in d and r.get("generated") == d["generated_count_pair"]["actual"]:
            return d["id"]
        u = d.get("unreduced_symmetry_counts")
        if u and [r.get("distinct"), r.get("generated"), r.get("depth")] == u["actual"]:
            return d["id"]
    return None

out, raw_p, setsha_p = sys.argv[1:4]
cells = {}
for spec in sys.argv[4:]:
    node, sel, path = spec.split(":", 2)
    for line in open(path):
        r = json.loads(line)
        if sel.startswith("rep="):
            rep = int(sel[4:])
        else:
            rep = r["rep"]
            if rep not in {int(x) for x in sel.split(",")}:
                continue
        w = int(r["impl"].rsplit("-w", 1)[1])
        pp = r["run"]["physical_process"]
        key = (r["run"]["inputs"]["model"], w, rep)
        assert key not in cells, key
        stopped = r.get("verdict") == "stopped" or pp.get("timed_out")
        reg = registered(key[0], r) if (w == 1 and not r["correct"]) else None
        cells[key] = {"registered": reg, "rep": rep, "node": node, "wall_ms": round(r["wall_s"] * 1000, 1), "rss": r["max_rss_bytes"],
                      "correct": r["correct"], "stopped": bool(stopped), "verdict": r.get("verdict"),
                      "exit": r.get("exit_code"), "workers_used": r.get("workers_used"),
                      "sequential_reason": r.get("sequential_reason"), "note": r.get("note") or None}
models = sorted({k[0] for k in cells})
rows, missing = {}, []
for m in models:
    row = {}
    for w in (1, 4, 8):
        reps = [cells[(m, w, rep)] for rep in (1, 2, 3) if (m, w, rep) in cells]
        if len(reps) < 3:
            missing.append(f"{m} w{w}: {len(reps)} reps")
        walls = [c["wall_ms"] for c in reps]
        row[f"w{w}"] = {"reps": len(reps), "wall_ms_median": round(statistics.median(walls), 1) if walls else None,
                        "wall_ms_range": [min(walls), max(walls)] if walls else None,
                        "peak_rss_median": int(statistics.median([c["rss"] for c in reps])) if reps else None,
                        "stopped": sum(c["stopped"] for c in reps),
                        "all_correct": all(c["correct"] for c in reps),
                        "registered_divergence": sorted({c["registered"] for c in reps if c["registered"]}),
                        "all_correct_or_registered": all(c["correct"] or c["registered"] for c in reps),
                        "workers_used": sorted({c["workers_used"] for c in reps if c["workers_used"] is not None}),
                        "sequential_reason": sorted({str(c["sequential_reason"]) for c in reps if c["sequential_reason"]}),
                        "notes": sorted({c["note"] for c in reps if c["note"] and not c["stopped"] and not c["registered"]}),
                        "runs_rep_node_wall_ms": [[c["rep"], c["node"], c["wall_ms"]] for c in reps],
                        "verdicts": sorted({c["verdict"] for c in reps})}
        if row[f"w{w}"]["verdicts"] == ["unsupported"]:
            row[f"w{w}"]["unsupported"] = UNSUP.get(m)
            row[f"w{w}"]["notes"] = []
        for k in ("sequential_reason", "notes", "registered_divergence"):
            if not row[f"w{w}"][k]:
                del row[f"w{w}"][k]
    for w in (4, 8):
        sp = [cells[(m, 1, rep)]["wall_ms"] / cells[(m, w, rep)]["wall_ms"] for rep in (1, 2, 3)
              if (m, 1, rep) in cells and (m, w, rep) in cells and cells[(m, w, rep)]["verdict"] == "success"
              and cells[(m, 1, rep)]["verdict"] in ("success", "stopped")]
        row[f"speedup{w}"] = round(statistics.median(sp), 2) if sp else None
        if row["w1"]["stopped"]:
            row[f"speedup{w}_is_lower_bound"] = True
    rows[m] = row
rec_raw = json.load(open(raw_p))
summary = {"models": len(rows), "cells_missing_reps": missing,
           "speedup8_median": statistics.median([r["speedup8"] for r in rows.values() if r["speedup8"]]),
           "speedup4_median": statistics.median([r["speedup4"] for r in rows.values() if r["speedup4"]]),
           "stopped_cells": sorted(f"{m} w{w}" for m, r in rows.items() for w in (1, 4, 8) if r[f"w{w}"]["stopped"]),
           "not_correct_cells": sorted(f"{m} w{w}" for m, r in rows.items() for w in (1, 4, 8)
                                       if not r[f"w{w}"]["all_correct_or_registered"] and not r[f"w{w}"]["stopped"]
                                       and not r[f"w{w}"].get("unsupported")),
           "unsupported_cells": sorted(f"{m} w{w}" for m, r in rows.items() for w in (1, 4, 8) if r[f"w{w}"].get("unsupported")),
           "registered_divergence_cells": sorted(f"{m} w{w}: {', '.join(r[f'w{w}'].get('registered_divergence', []))}" for m, r in rows.items()
                                                  for w in (1, 4, 8) if r[f"w{w}"].get("registered_divergence"))}
json.dump({"summary": summary, "raw": rec_raw, "set_sha256": json.load(open(setsha_p)), "rows": rows},
          open(out, "w"), indent=1)
print(json.dumps(summary, indent=1))
