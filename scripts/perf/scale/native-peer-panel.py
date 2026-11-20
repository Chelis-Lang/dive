#!/usr/bin/env python3
"""Run the pinned native peer panel on one dedicated Linux host."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
from time import perf_counter_ns

MODELS = {
    "kvs": ("MCKVS.tla", "MCKVSSafetyMedium.cfg", "tlx"),
    "lamport": ("MCLamportMutex.tla", "MCLamportMutex.cfg", "tlx"),
    "counters": ("counters.tla", "counters.cfg", "manyan"),
    "counters-wide": ("counters.tla", "counters.cfg", "manyan"),
    "liveness": ("liveness.tla", "liveness.cfg", "manyan"),
    "emergency": ("EmergencyCall_Model_auto.tla", "f3.cfg", None),
    "bp": ("BPConProof.tla", "BPConProof.cfg", None),
    "startup": ("SimpleRegular.tla", "SimpleRegular.cfg", None),
    "tiny": ("Counter.tla", "Counter.cfg", None),
}
STOPS = {"stopped", "timeout", "killed"}


def parse_dive(stdout, code):
    try:
        report = json.loads(stdout)
    except json.JSONDecodeError:
        return {"status": {124: "timeout", 137: "killed"}.get(code, "unclassified")}
    stats = report.get("statistics") or {}
    options = report.get("options") or {}
    metrics = (report.get("core") or {}).get("metrics") or {}
    return {
        "status": report.get("verdict"),
        "distinct": stats.get("distinct"),
        "generated": stats.get("generated"),
        "depth": stats.get("depth"),
        "property": report.get("property"),
        "workers_used": options.get("workers", {}).get("used"),
        "explore_ms": metrics.get("explore_ms"),
        "compile_ms": metrics.get("compile_ms"),
        "collision": report.get("collision"),
        "elapsed_ms": report.get("elapsed_ms"),
        "error_site": report.get("error_site"),
    }


def parse_peer(stdout, stderr, code):
    output = stdout + "\n" + stderr
    if code == 0 and "No error has been found" in output:
        counts = re.search(r"(\d+) states generated,\s*(\d+) distinct states found", stdout)
        depth = re.search(r"(?:depth of the complete state graph search is|Depth)\s+(\d+)", stdout, re.I)
        return {
            "status": "success" if counts and depth else "unclassified",
            "generated": int(counts[1]) if counts else None,
            "distinct": int(counts[2]) if counts else None,
            "depth": int(depth[1]) if depth else None,
        }
    status = {124: "timeout", 137: "killed"}.get(code, "peer-error" if code else "unclassified")
    result = {"status": status}
    if status == "timeout":
        progress = re.findall(
            r"progress: depth (\d+), ([\d,]+) generated, ([\d,]+) distinct[^\n]*?, ([\d.]+)s",
            output, re.I,
        )
        if progress:
            depth, generated, distinct, elapsed = progress[-1]
            result["last_progress"] = {
                "depth": int(depth),
                "generated": int(generated.replace(",", "")),
                "distinct": int(distinct.replace(",", "")),
                "seconds_since_start": float(elapsed),
            }
    return result


def measure(args, model, workers, rep, arm):
    module, config, _ = MODELS[model]
    cell = args.output / f"{model}-{workers}-{rep}-{arm}-{args.profile}"
    cell.mkdir()
    cwd = args.stage / "models" / model
    env = dict(os.environ, LC_ALL="C")
    for key in tuple(env):
        if key.startswith(("DIVE_", "TLX_", "TLAMC_", "MANYAN_")):
            env.pop(key)
    cache = args.output / "cache" / f"{model}-{workers}-{rep}-tlx-{args.profile}"
    env["TLX_CACHE"] = str(cache)
    arm_binary = args.stage / "bin" / ("tlx" if arm in ("tlx-cold", "tlx-warm") else arm)
    if arm in ("base", "candidate"):
        command = [
            str(arm_binary), "check", module, "--config", config, "--workers", str(workers),
            "--fp", "0", "--seed", "0", "--time-limit", f"{args.seconds - 5}s",
            "--max-memory", "24g", "--output", "json",
        ]
        if args.profile == "tlc":
            command.append("--compat=tlc")
        command.extend(("--store", args.store))
        if arm == "candidate":
            env.update(args.candidate_env)
    elif arm.startswith("tlx-"):
        command = [str(arm_binary), "check", module, "-tlc", "-compile",
                   "-config", config, "-workers", str(workers)]
    else:
        command = [str(arm_binary), "-fp", "0", "-config", config, "-workers", str(workers), module]
    time_file = cell / "time"
    with (cell / "stdout").open("w") as stdout, (cell / "stderr").open("w") as stderr:
        start = perf_counter_ns()
        process = subprocess.run(
            ["/usr/bin/time", "-q", "-f", "%e %U %S %M", "-o", str(time_file),
             "timeout", "--kill-after=5s", f"{args.seconds}s", *command],
            cwd=cwd, env=env, stdout=stdout, stderr=stderr, check=False,
        )
        wall_s = (perf_counter_ns() - start) / 1e9
    stdout = (cell / "stdout").read_text(errors="replace")
    stderr = (cell / "stderr").read_text(errors="replace")
    timing = time_file.read_text().split()
    return {
        "model": model, "workers": workers, "repetition": rep, "arm": arm,
        "profile": args.profile, "exit": process.returncode, "argv": command,
        "wall_s": wall_s,
        "cpu_s": float(timing[1]) + float(timing[2]) if len(timing) == 4 else None,
        "peak_rss_kib": int(timing[3]) if len(timing) == 4 else None,
        **(parse_dive(stdout, process.returncode) if arm in ("base", "candidate")
           else parse_peer(stdout, stderr, process.returncode)),
    }


def blocks_for(model, with_candidate, with_peers):
    blocks = [("base",)]
    if with_candidate:
        blocks.append(("candidate",))
    peer = MODELS[model][2]
    if with_peers and peer:
        blocks.append(("tlx-cold", "tlx-warm") if peer == "tlx" else (peer,))
    return blocks


def digests(args):
    inputs = [
        path for model in args.models
        for path in sorted((args.stage / "models" / model).iterdir())
        if path.suffix in (".tla", ".cfg")
    ]
    binaries = {
        arm.split("-")[0]
        for model in args.models
        for block in blocks_for(model, args.candidate, not args.no_peers)
        for arm in block
    }
    inputs.extend(args.stage / "bin" / binary for binary in sorted(binaries))
    return {str(path.relative_to(args.stage)): hashlib.sha256(path.read_bytes()).hexdigest()
            for path in inputs}


def campaign(args):
    # Scratch output is unique per invocation so a cold cell cannot reuse a cache
    # from an earlier attempt.
    args.output.mkdir(parents=True)
    (args.output / "inputs.json").write_text(json.dumps(digests(args), indent=2) + "\n")
    stopped = set()
    with (args.output / "cells.jsonl").open("w", buffering=1) as output:
        for model in args.models:
            blocks = blocks_for(model, args.candidate, not args.no_peers)
            for rep in range(args.repetitions):
                order = slice(None, None, -1 if rep % 2 else 1)
                for workers in args.workers[order]:
                    for block in blocks[order]:
                        for arm in block:
                            cell = (model, workers, arm)
                            if cell in stopped:
                                row = {"model": model, "workers": workers, "arm": arm,
                                       "repetition": rep, "status": "omitted-after-stop"}
                            else:
                                row = measure(args, model, workers, rep, arm)
                                if args.stop_after_cap and row["status"] in STOPS:
                                    stopped.add(cell)
                            line = json.dumps(row, sort_keys=True)
                            print(line, file=output)
                            print(line, flush=True)
                        # A warm tlx cell always uses its own preceding cold sample.


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", type=lambda p: Path(p).resolve())
    parser.add_argument("output", type=lambda p: Path(p).resolve())
    parser.add_argument("--models", nargs="+", choices=MODELS, default=list(MODELS))
    parser.add_argument("--workers", nargs="+", type=int, choices=(1, 8), default=[1, 8])
    parser.add_argument("--profile", choices=("dive", "tlc"), default="tlc")
    parser.add_argument("--store", choices=("fp", "exact"), default="fp",
                        help="state identity for dive arms")
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--seconds", type=int, default=600)
    parser.add_argument("--candidate", action="store_true")
    parser.add_argument("--candidate-env", action="append", default=[], metavar="NAME=VALUE")
    parser.add_argument("--no-peers", action="store_true")
    parser.add_argument("--stop-after-cap", action="store_true")
    args = parser.parse_args()
    if args.seconds < 6 or args.repetitions < 1:
        parser.error("need at least six seconds and one repetition")
    candidate_env = {}
    for arg in args.candidate_env:
        key, sep, value = arg.partition("=")
        if not args.candidate or not sep or not key.startswith("DIVE_EXPERIMENT_"):
            parser.error("--candidate-env requires --candidate and DIVE_EXPERIMENT_NAME=VALUE")
        candidate_env[key] = value
    args.candidate_env = candidate_env
    campaign(args)


if __name__ == "__main__":
    main()
