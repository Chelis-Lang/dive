#!/usr/bin/env python3
"""Builds the compact S4 simulation throughput record from simbench.py's runs.

Usage: simrecord.py --runs runs.jsonl --hostinfo hostinfo.txt --source SHA \
    --out conformance/reports/perf/DATE-simulation-throughput-SHORT.json [--profile FILE]
"""
import argparse, json, statistics
from collections import defaultdict
from pathlib import Path


def med(values):
    return statistics.median(values) if values else None


def main():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--runs", type=Path, required=True)
    p.add_argument("--hostinfo", type=Path, required=True)
    p.add_argument("--source", required=True)
    p.add_argument("--date", required=True)
    p.add_argument("--profile", type=Path, help="compact JSON summary of the scaling profile")
    p.add_argument("--out", type=Path, required=True)
    a = p.parse_args()
    runs = [json.loads(line) for line in a.runs.read_text().splitlines() if line.strip()]
    host = a.hostinfo.read_text().splitlines()
    groups = defaultdict(list)
    for r in runs:
        groups[(r["bench"], r["kind"], r["tool"], r["workers"])].append(r)
    benches = list(dict.fromkeys(r["bench"] for r in runs))
    rows, problems = {}, []
    for b in benches:
        row = {"startup_ms": {}, "workers": {}}
        for tool in ("dive", "tlc"):
            s = groups.get((b, "startup", tool, 1), [])
            row["startup_ms"][tool] = round(med([r["wall_s"] for r in s]) * 1000, 1) if s else None
        traces = None
        for w in sorted({r["workers"] for r in runs if r["bench"] == b and r["kind"] == "throughput"}):
            cell = {}
            for tool in ("dive", "tlc"):
                g = groups.get((b, "throughput", tool, w), [])
                if not g:
                    continue
                traces = g[0]["requested_traces"]
                for r in g:
                    if r.get("result") != "sampled" or r.get("traces") != traces or r.get("exit") != 0:
                        problems.append(f"{b} {tool} w{w} rep{r['rep']}: {r.get('result')} {r.get('traces')} exit {r.get('exit')}")
                wall = med([r["wall_s"] for r in g])
                startup = row["startup_ms"][tool] / 1000 if row["startup_ms"][tool] else 0
                cell[tool] = {
                    "reps": len(g),
                    "wall_s": round(wall, 3),
                    "wall_range_s": [round(min(r["wall_s"] for r in g), 3), round(max(r["wall_s"] for r in g), 3)],
                    "traces_per_s": round(traces / wall, 1),
                    "traces_per_s_after_startup": round(traces / (wall - startup), 1) if wall > startup else None,
                    "peak_rss_mib": round(max(r["peak_rss_bytes"] or 0 for r in g) / 2**20, 1),
                    "generated_states": g[0].get("generated"),
                }
                if tool == "dive" and g[0].get("temporal"):
                    cell[tool]["temporal_checked_unchecked"] = g[0]["temporal"]
            if "dive" in cell and "tlc" in cell:
                cell["tlc_wall_over_dive_wall"] = round(cell["tlc"]["wall_s"] / cell["dive"]["wall_s"], 2)
            row["workers"][str(w)] = cell
        row["traces"] = traces
        outcomes = {(r.get("generated"), r.get("traces"), json.dumps(r.get("temporal")))
                    for r in runs if r["bench"] == b and r["kind"] == "throughput" and r["tool"] == "dive"}
        row["dive_identical_across_workers_and_reps"] = len(outcomes) == 1
        one = row["workers"].get("1", {})
        for w, cell in row["workers"].items():
            for tool in ("dive", "tlc"):
                if tool in cell and tool in one:
                    cell[tool]["speedup_over_1_worker"] = round(one[tool]["wall_s"] / cell[tool]["wall_s"], 2)
        rows[b] = row
    record = {
        "measurement": "S4 simulation throughput",
        "plan": "docs/developer/simulation-design.md#14-acceptance-metrics",
        "date": a.date,
        "source": a.source,
        "oracle": "tlc-2026.10.06.014338",
        "corpus": "examples@6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f",
        "host": host,
        "command": {
            "dive": "dive simulate <module> --config <cfg> --traces N --depth 100 --seed 0 --workers W --time-limit 1800s --max-memory 64g --output json",
            "tlc": "java -XX:+UseParallelGC -Xmx16g -Xss4m -DTLA-Library=<tlapm library> -cp tla2tools.jar:CommunityModules jars tlc2.TLC <module> -config <cfg> -simulate num=N/W -depth 100 -workers W -seed 0",
            "binding": "numactl --cpunodebind=0 --membind=0",
            "startup": "the same commands with N = 1 and W = 1, ten fresh runs each",
            "notemporal": "a -notemporal bench is the same configuration with its PROPERTY lines removed",
        },
        "method": "Each tool samples N traces of depth 100 at seed 0: dive's --traces is the total, TLC's num is per worker (D-003), so TLC runs num=N/W. Runs alternate dive and TLC; wall time is perf_counter around the process, including JVM start for TLC; peak RSS from GNU time. Every run must sample all N traces with exit 0. traces_per_s_after_startup subtracts the tool's median one-trace startup. dive_identical_across_workers_and_reps compares dive's generated count, trace count and per-property temporal coverage over every throughput run of the bench. A temporal bench generates more states than its -notemporal twin because --liveness-extend continues undecided traces.",
        "raw": "not kept: runs.jsonl is regenerated by scripts/perf/release/node/sim-chain.sh on a fresh node",
        "problems": problems,
        "rows": rows,
    }
    if a.profile:
        record["profile"] = json.loads(a.profile.read_text())
    a.out.write_text(json.dumps(record, indent=1) + "\n")
    print(f"{len(rows)} benches, {len(runs)} runs, {len(problems)} problems -> {a.out}")


if __name__ == "__main__":
    main()
