#!/usr/bin/env python3
"""Run cold fair-word discovery and independent replay under one wall budget."""

import argparse
from hashlib import sha256
import json
import os
from pathlib import Path
import subprocess
import tempfile
from time import monotonic


def run(command, deadline, allowance, environment):
    started = monotonic()
    seconds = min(allowance, deadline - started)
    if seconds < 0.001:
        return {"result": "stopped", "reason": "shared wall limit"}, None
    try:
        result = subprocess.run(command, env=environment, capture_output=True,
                                check=False, timeout=seconds)
    except subprocess.TimeoutExpired:
        return {"result": "stopped", "reason": "shared wall limit", "wall_s": monotonic() - started}, None
    elapsed = monotonic() - started
    try:
        document = json.loads(result.stdout)
    except (TypeError, ValueError):
        return {"result": "unsupported", "reason": result.stderr.decode(errors="replace").strip(),
                "exit": result.returncode, "wall_s": elapsed}, None
    statistics = document.get("statistics") or {}
    facts = {"result": document.get("verdict", document.get("result")), "exit": result.returncode,
             "wall_s": elapsed, "seed": (document.get("options") or {}).get("seed"),
             "traces": statistics.get("traces_started"), "statistics": statistics,
             "reason": document.get("message")}
    return facts, document


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("--config", type=Path, required=True)
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--library-path", type=Path, action="append", default=[])
    parser.add_argument("--seconds", type=int, default=360)
    parser.add_argument("--memory", default="8g")
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--min-states", default=65_536, type=int)
    parser.add_argument("--probe-seconds", default=2, type=int)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args(argv)
    if args.seconds < 2 or not 0 <= args.probe_seconds < args.seconds or args.min_states <= 0:
        parser.error("positive resources and a probe shorter than the shared budget are required")

    with tempfile.TemporaryDirectory(prefix="dive-cold-fair-") as path:
        scratch = Path(path)
        env = {**os.environ, "DIVE_CACHE_DIR": str(scratch / "empty-cache"), "DIVE_COMPAT": "dive"}
        Path(env["DIVE_CACHE_DIR"]).mkdir()
        library = [part for directory in args.library_path for part in ("--library-path", str(directory))]
        common = [str(args.module), "--config", str(args.config), *library, "--output", "json"]
        policy = ["--workers", "1", "--seed", str(args.seed), "--max-memory", args.memory]
        dive = str(args.dive.resolve())
        deadline = monotonic() + args.seconds
        attempts = []
        stages = []
        if args.probe_seconds:
            stages.append(("simulation", float(args.probe_seconds), [dive, "simulate", *common, *policy,
                           "--depth", "120", "--traces", "2048", "--keep-traces", "0",
                           "--sample-unbounded", "off", "--time-limit", f"{args.probe_seconds}s"]))
        stages.append(("first-admitted-init", float(args.seconds), [dive, "check", *common, *policy,
                       "--lncheck", "cold-fair", "--fp", "0", "--periodic-min-states", str(args.min_states)]))
        final = "bounded-inconclusive"
        for name, allowance, command in stages:
            if name == "first-admitted-init":
                milliseconds = int(1000 * (deadline - monotonic()) - 1000)
                if milliseconds <= 0:
                    break
                command.extend(["--time-limit", f"{milliseconds}ms"])
            facts, response = run(command, deadline, allowance + 0.25, env)
            attempts.append({"stage": name, **facts})
            if not response:
                final = facts["result"]
                continue
            trace = (response.get("counterexample") or {}).get("trace")
            if name == "simulation" and response.get("violation_validated") and response.get("traces"):
                trace = response["traces"][0].get("document")
            if isinstance(trace, dict) and (trace.get("ending") or {}).get("kind") in {"lasso", "stuttering", "stutter"}:
                witness = scratch / "word.json"
                witness.write_text(json.dumps(trace))
                replay, _ = run([dive, "validate-trace", *common, "--trace", str(witness)], deadline, args.seconds, env)
                if replay["result"] == "valid":
                    with args.out.open("x") as output:
                        output.write(json.dumps(trace) + "\n")
                    attempts[-1]["replay"] = replay
                    final = "validated-fair-counterexample"
                    break
                attempts[-1]["replay"] = replay
            if name == "first-admitted-init" and facts["result"] in {"unsupported", "stopped", "rejected", "internal-error"}:
                final = facts["result"]
        print(json.dumps({"result": final, "seed": args.seed, "seconds": args.seconds, "memory": args.memory,
                          "module_sha256": sha256(args.module.read_bytes()).hexdigest(),
                          "configuration_sha256": sha256(args.config.read_bytes()).hexdigest(),
                          "attempts": attempts}))
        return {"validated-fair-counterexample": 0, "bounded-inconclusive": 2}.get(final, 3)


if __name__ == "__main__":
    raise SystemExit(main())
