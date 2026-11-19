#!/usr/bin/env python3
"""Run measurement jobs one at a time and append one compact JSON row per run.

  runjobs.py <jobs.json> <out-dir> [<prefix-argv-json>]

jobs.json: a list of {"id": str, "argv": [...], "cwd": str?, "stdout_json": bool?}.
Each run is wrapped in GNU `/usr/bin/time -v` (Linux) or `/usr/bin/time -l`
(macOS); wall time is perf_counter around subprocess.run. Raw stdout goes to
<out-dir>/raw/<n>.out. Rows go to <out-dir>/rows.jsonl.
"""
import json
import os
import platform
import re
import subprocess
import sys
import time

HOME = os.path.expanduser("~")


def get(d, path):
    for k in path.split("/"):
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d


def main():
    run_jobs(json.load(open(sys.argv[1])), sys.argv[2], json.loads(sys.argv[3]) if len(sys.argv) > 3 else [])


def run_jobs(jobs, out, prefix=()):
    prefix = list(prefix)
    os.makedirs(os.path.join(out, "raw"), exist_ok=True)
    done = set()
    rows_path = os.path.join(out, "rows.jsonl")
    if os.path.exists(rows_path):
        for line in open(rows_path):
            done.add(json.loads(line)["n"])
    mac = platform.system() == "Darwin"
    timer = ["/usr/bin/time", "-l"] if mac else ["/usr/bin/time", "-v"]
    for n, job in enumerate(jobs):
        if n in done:
            continue
        argv = [a.replace("~", HOME, 1) if a.startswith("~") else a for a in job["argv"]]
        raw = os.path.join(out, "raw", f"{n}.out")
        err = os.path.join(out, "raw", f"{n}.err")
        t0 = time.perf_counter()
        start = time.time()
        with open(raw, "wb") as fo, open(err, "wb") as fe:
            rc = subprocess.call(prefix + (timer if job.get("time", True) else []) + argv, cwd=job.get("cwd", HOME), stdout=fo, stderr=fe)
        wall = (time.perf_counter() - t0) * 1000
        text = open(err, errors="replace").read()
        if mac:
            m = re.search(r"(\d+)\s+maximum resident set size", text)
            rss = int(m.group(1)) if m else None  # bytes on macOS
        else:
            m = re.search(r"Maximum resident set size \(kbytes\): (\d+)", text)
            rss = int(m.group(1)) * 1024 if m else None
        row = {"n": n, "id": job["id"], "rc": rc, "wall_ms": round(wall, 2), "start": start,
               "peak_rss_bytes": rss}
        if job.get("stdout_json", True):
            try:
                rep = json.load(open(raw))
            except (ValueError, OSError):
                rep = None
            if isinstance(rep, dict):
                for key, path in [("verdict", "verdict"), ("exit_code", "exit_code"),
                                  ("distinct", "statistics/distinct"), ("generated", "statistics/generated"),
                                  ("depth", "statistics/depth"), ("store", "options/store"),
                                  ("check_kind", "check_kind"), ("workers_used", "options/workers/used"),
                                  ("workers_requested", "options/workers/requested"),
                                  ("sequential", "options/workers/sequential"),
                                  ("sequential_reason", "options/workers/reason"),
                                  ("observed_max_bytes", "resources/memory/observed_max_bytes"),
                                  ("known_retained_lb", "core/metrics/store/memory/known_retained_bytes_lower_bound"),
                                  ("stopped", "stopped"), ("elapsed_ms", "elapsed_ms"),
                                  ("validation", "validation"), ("result", "result")]:
                    v = get(rep, path)
                    if v is not None:
                        row[key] = v
                for k in ("statistics", "stats", "summary"):
                    if isinstance(rep.get(k), dict) and k != "statistics":
                        row[k] = rep[k]
        with open(rows_path, "a") as f:
            f.write(json.dumps(row) + "\n")
    open(os.path.join(out, "DONE"), "w").close()


if __name__ == "__main__":
    main()
