#!/usr/bin/env python3
"""D6 spec for mkrecord.py.  d6spec.py <rows.jsonl> <raw-sha256> <out-spec.json>"""
import json
import statistics
import sys

rows_p, raw, out = sys.argv[1:4]
rows = [json.loads(l) for l in open(rows_p)]


def med(xs):
    xs = [x for x in xs if isinstance(x, (int, float))]
    return round(statistics.median(xs), 2) if xs else None


def short(cfg):
    return cfg.replace("specifications/", "").replace(".cfg", "")


load, time_rows, ceil_rows, temp_rows = {}, {}, {}, {}
for r in rows:
    p = r["id"].split("|")
    if p[0] == "load":
        load.setdefault((p[1], p[2]), []).append(r["wall_ms"])
for r in rows:
    p = r["id"].split("|")
    st = r.get("stopped") or {}
    if p[0] == "time":
        cfg, w, lim = p[1], p[2], p[3]
        lim_ms = int(lim[:-1]) * 1000
        ld = med(load[(cfg, w)])
        time_rows.setdefault(f"{short(cfg)}|workers={w}|--time-limit {lim}", []).append({
            "wall_ms": r["wall_ms"], "exit_code": r["rc"], "phase": st.get("phase"), "resource": st.get("resource"),
            "stopped_elapsed_ms": st.get("elapsed_ms"), "latency_ms": round(r["wall_ms"] - ld - lim_ms, 1), "wall_minus_limit_ms": round(r["wall_ms"] - lim_ms, 1),
            "stopped_elapsed_over_limit_ms": (st.get("elapsed_ms") - lim_ms) if st.get("elapsed_ms") is not None else None,
            "distinct": r.get("distinct"), "load_ms": ld})
    elif p[0] in ("ceiling", "temporal-ceiling"):
        cfg = p[1]
        if p[0] == "ceiling":
            w, fl = p[2], p[3]
        else:
            w, fl = "1", p[2]
        flag, val = fl.split("=")
        val = int(val)
        got = r.get("distinct") if flag == "--max-states" else r.get("depth")
        tgt = ceil_rows if p[0] == "ceiling" else temp_rows
        tgt.setdefault(f"{short(cfg)}|workers={w}|{flag} {val}" + ("|--store exact" if p[0] != "ceiling" else ""), []).append({
            "wall_ms": r["wall_ms"], "exit_code": r["rc"], "verdict": r.get("verdict"), "phase": st.get("phase"),
            "resource": st.get("resource"), "distinct": r.get("distinct"), "depth": r.get("depth"),
            "overshoot": (got - val) if isinstance(got, int) else None, "check_kind": r.get("check_kind")})


def summarise(d):
    out = {}
    for k, reps in d.items():
        s = {"reps": len(reps), "wall_ms_median": med(x["wall_ms"] for x in reps),
             "exit_codes": sorted({x["exit_code"] for x in reps}), "phases": sorted({str(x["phase"]) for x in reps})}
        if "latency_ms" in reps[0]:
            s.update({"latency_ms_median": med(x["latency_ms"] for x in reps),
                      "latency_ms_max": max(x["latency_ms"] for x in reps),
                      "wall_minus_limit_ms_median": med(x["wall_minus_limit_ms"] for x in reps),
                      "wall_minus_limit_ms_max": max(x["wall_minus_limit_ms"] for x in reps),
                      "stopped_elapsed_over_limit_ms_max": max((x["stopped_elapsed_over_limit_ms"] or 0) for x in reps),
                      "load_ms": reps[0]["load_ms"]})
        else:
            s.update({"overshoot": sorted({x["overshoot"] for x in reps}, key=str),
                      "distinct": sorted({x["distinct"] for x in reps}, key=str),
                      "depth": sorted({x["depth"] for x in reps}, key=str),
                      "verdict": sorted({str(x["verdict"]) for x in reps})})
        s["runs"] = reps
        out[k] = s
    return out


spec = {
    "measurement": "D6", "anchor": "d6-stop-latency",
    "command": "dive check <module.tla> --config <model.cfg> --workers 1|8 --seed 0 --fp 0 with --time-limit 5s|30s|120s "
               "--max-memory 64g (5 repetitions), --max-states 1 for load time (3 repetitions), and the state/depth ceilings "
               "with --time-limit 3600s (3 repetitions); --output json",
    "latency": "wall time minus the load time (median of the --max-states 1 runs at the same worker count) minus the limit",
    "latency_note": "For ElevatorLivenessLarge the --max-states 1 run stops in graph capture and then runs the temporal "
                    "stop path (about 68 ms against about 13 ms for the safety models), so subtracting it overstates load "
                    "and the computed latency is negative. wall_minus_limit_ms, which includes load, is the upper bound "
                    "for that model.",
    "not_measured": ["Time from the last progress report to exit: the runs used --output json without progress output."],
    "raw": {"location": "dive evidence archive bucket (us-east-1), prefix release-perf/2026-11-18/small-node-raw.tar.gz",
            "sha256": raw},
    "deviations": ["Release binary built on one bench node and copied to the others (independent builds were not "
                   "byte-identical); dive_binary_sha256 names the copied binary."],
    "rows": {"time_budget": summarise(time_rows), "state_depth_ceilings": summarise(ceil_rows),
             "temporal_ceilings": summarise(temp_rows)},
}
json.dump(spec, open(out, "w"), indent=1)
