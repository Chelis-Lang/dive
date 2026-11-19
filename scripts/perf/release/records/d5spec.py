#!/usr/bin/env python3
"""D5 spec for mkrecord.py from drecords d5a/d5b output and the raw rows.

d5spec.py <a.jsonl> <b.jsonl> <golden-dir> <raw-sha256> <out-spec.json>
"""
import json
import subprocess
import sys

A, Bp, G, RAWSHA, OUT = sys.argv[1:6]
T = __file__.rsplit("/", 1)[0]
a = json.loads(subprocess.check_output(["python3", f"{T}/drecords.py", "d5a", A, G]))
b = json.loads(subprocess.check_output(["python3", f"{T}/drecords.py", "d5b", Bp, G]))
rows_a = [json.loads(l) for l in open(A)]
startup = {k: a[k] for k in ("version", "parse", "check")}
sim = {}
for cfg in ["specifications/lamport_mutex/MCLamportMutex.cfg", "specifications/bosco/bosco.cfg"]:
    rs = [r for r in rows_a if r["id"].startswith(f"simulate|{cfg}|")]
    reps = []
    for r in rs:
        st = r.get("stopped") or {}
        reps.append({"wall_ms": r["wall_ms"], "exit_code": r["rc"], "peak_rss_bytes": r["peak_rss_bytes"],
                     "generated": r.get("generated"), "stopped": st or None})
    sim[cfg] = {"reps": reps}
sim["specifications/lamport_mutex/MCLamportMutex.cfg"].update({
    "result": "violation (simulation deadlock on the first trace)", "traces_started": 1, "generated": 72,
    "traces_per_second": None, "states_per_second": None,
    "note": "Every successor of the trace's last state is pruned by the state constraint, which dive reports as a "
            "deadlock (docs/user/simulation.md). The run stops after one trace, so it is not a throughput figure."})
bos = sim["specifications/bosco/bosco.cfg"]
el = sorted(r["stopped"]["elapsed_ms"] for r in bos["reps"])[1]
bos.update({
    "result": "stopped at the 16 GiB memory budget in every repetition", "traces_started": 9901,
    "generated": 2049127, "stopped_elapsed_ms": el,
    "traces_per_second": round(9901 / (el / 1000), 2), "states_per_second": round(2049127 / (el / 1000), 0),
    "note": "Rates use stopped.elapsed_ms (the budget clock). The budget observed 17.18 GB at the stop, but peak RSS "
            "from time -v is 30.9 GB, about 1.8x the --max-memory limit, and the process exits about 17 s after the "
            "stop. Tracked in the D5 perf issue."})
lens = {"specifications/DieHard/DieHard.cfg": 7, "specifications/SlidingPuzzles/SlidingPuzzles.cfg": 117,
        "specifications/SpecifyingSystems/RealTime/MCRealTimeHourClock.cfg": 2, "specifications/bosco/bosco.cfg": 101}
val = {}
for k, v in b.items():
    kind, cfg = k.split("|")
    n = lens[cfg]
    val[k] = dict(v, trace_states=n, states_per_second=round(n / (v["median_ms"] / 1000), 1))
spec = {
    "measurement": "D5", "anchor": "d5-startup-simulation-and-trace-validation-throughput",
    "command": {
        "startup": "dive --version | dive parse docs/user/examples/Counter.tla | dive check docs/user/examples/Counter.tla "
                   "--config docs/user/examples/Counter.cfg --workers 1 --seed 0 --fp 0 --output json (50 fresh runs each)",
        "simulation": "dive simulate <module.tla> --config <model.cfg> --traces 10000 --depth 100 --seed 0 --stats basic "
                      "--time-limit 600s --max-memory 16g --output json (3 repetitions)",
        "validation": "dive validate-trace <module.tla> --config <model.cfg> --trace trace.json --seed 0 --fp 0 --output json "
                      "(10 each); --sample for 100 bosco simulation traces (simulate --traces 100 --depth 100 --seed 0)"},
    "timing": "perf_counter around subprocess.run; check, simulate and validate-trace under GNU /usr/bin/time -v for peak RSS",
    "raw": {"location": "dive evidence archive bucket (us-east-1), prefix release-perf/2026-11-18/small-node-raw.tar.gz",
            "sha256": RAWSHA},
    "deviations": [
        "The 100 --sample traces come from bosco's simulation, not MCLamportMutex's: MCLamportMutex simulation stops at "
        "its first trace with a reported deadlock, so it yields one trace.",
        "Release binary built on one bench node and copied to the others, because independent builds of the same "
        "source were not byte-identical; dive_binary_sha256 names the copied binary."],
    "rows": {"startup": startup, "simulation": sim, "trace_validation": val},
}
json.dump(spec, open(OUT, "w"), indent=1)
