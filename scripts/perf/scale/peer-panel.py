#!/usr/bin/env python3
"""Run one checker at a time. Results and native caches are out of tree."""
import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

MODELS = {
    "kvs": ("MCKVS.tla", "MCKVSSafetyMedium.cfg"),
    "lamport": ("MCLamportMutex.tla", "MCLamportMutex.cfg"),
    "counters": ("counters.tla", "counters.cfg"),
    "liveness": ("liveness.tla", "liveness.cfg"),
    "emergency": ("EmergencyCall_Model_auto.tla", "f3.cfg"),
}


def command(root, arm, module, config, workers, directory):
    flags = ["-config", config, "-workers", str(workers)]
    exe = root / "bin" / arm
    if arm in ("core", "release", "spike", "spike-off", "spike-stats"):
        exe = root / "bin" / f"dive-{arm.split('-', 1)[0]}"
        return [
            str(exe), "check", module, "--config", config, "--workers", str(workers),
            "--compat=tlc", "--fp", "0", "--seed", "0", "--time-limit", "595s",
            "--max-memory", "24g", "--output", "json",
        ]
    if arm in ("tlc-pinned", "tlc-1.8.0"):
        cp = ":".join(str(root / "inputs" / f) for f in
                      (arm + ".jar", "community.jar", "community-deps.jar"))
        return [
            str(root / "inputs/jre/bin/java"), "-Xmx16g", "-XX:+UseParallelGC",
            "-XX:ActiveProcessorCount=8", "-cp", cp, "tlc2.TLC", "-fp", "0", "-seed", "0",
            "-metadir", str(directory / "tlc-states"), *flags, module,
        ]
    if arm == "manyan":
        return [str(exe), "-fp", "0", *flags, module]
    peer, tier = arm.split("_", 1)
    exe = root / "bin" / peer
    if peer == "tlx":
        tier_flag = "-interp" if tier == "interp" else "-compile"
        return [str(exe), "check", module, "-tlc", tier_flag, *flags]
    return [str(exe), *([] if tier == "interp" else ["-compile"]), *flags, module]


def parse_result(arm, stdout, stderr, code):
    if arm in ("core", "release", "spike", "spike-off", "spike-stats"):
        try:
            data = json.loads(stdout)
        except json.JSONDecodeError:
            return {"status": "timeout" if code == 124 else "unclassified"}
        stats = data.get("statistics") or {}
        workers = data.get("options", {}).get("workers", {})
        metrics = data.get("core", {}).get("metrics", {}) or {}
        return {
            "status": data.get("verdict"), "distinct": stats.get("distinct"),
            "generated": stats.get("generated"), "depth": stats.get("depth"),
            "workers_used": workers.get("used"), "worker_reason": workers.get("reason"),
            "property": data.get("property"), "collision": data.get("collision"),
            "explore_ms": metrics.get("explore_ms"), "compile_ms": metrics.get("compile_ms"),
            "error": data.get("error"),
        }
    text = stdout + "\n" + stderr
    states = re.findall(r"(\d+) states generated,\s*(\d+) distinct states found", text)
    depths = re.findall(r"(?:depth of the complete state graph search is\s*|Depth\s+)(\d+)", text, re.I)
    workers = re.search(r"with\s+(\d+)\s+workers?\b", text, re.I)
    status = "completed" if code == 0 and re.search(r"No error has been found", text, re.I) else (
        "timeout" if code == 124 else "tool-error" if code else "unclassified"
    )
    return {
        "status": status, "distinct": int(states[-1][1]) if states else None,
        "generated": int(states[-1][0]) if states else None,
        "depth": int(depths[-1]) if depths else None,
        "workers_used": int(workers[1]) if workers else None,
        "tier_lines": [ln for ln in text.splitlines() if re.match(r"(?:compile:|Compiled|Optimizations:)", ln)],
        **({"diagnostic": text[-1600:]} if status in ("tool-error", "unclassified") else {}),
    }


def measure(root, output, model, workers, repetition, arm, abi_profile, profile_perf=False):
    directory = output / f"{model}-{workers}-{repetition}-{arm}-{abi_profile}"
    directory.mkdir()
    module, config = MODELS[model]
    cwd = root / "models" / model
    env = dict(os.environ, LC_ALL="C")
    for key in ("DIVE_ENGINE", "DIVE_CORE_OPT", "DIVE_COMPAT", "DIVE_CHAOS", "TLX_PARANOID", "DIVE_EXPERIMENT_GUARDS"):
        env.pop(key, None)
    cache = output / "cache" / f"{model}-{workers}-{repetition}-{arm.rsplit('_', 1)[0]}"
    cache.mkdir(parents=True, exist_ok=True)
    env.update(TLX_CACHE=str(cache), TLAMC_CACHE=str(cache), CARGO_BUILD_JOBS="8")
    if arm in ("spike", "spike-stats"):
        env["DIVE_EXPERIMENT_GUARDS"] = "stats" if arm == "spike-stats" else "1"
    argv = command(root, arm, module, config, workers, directory)
    if abi_profile == "dive":
        argv.remove("--compat=tlc")
    if profile_perf:
        argv = [
            "perf", "record", "-e", "cpu-clock:u", "-F", "197", "--call-graph", "dwarf,8192",
            "-o", str(directory / "perf.data"), "--", *argv,
        ]
    time_path = directory / "time.txt"
    with (directory / "stdout").open("w") as stdout, (directory / "stderr").open("w") as stderr:
        p = subprocess.run(
            ["/usr/bin/time", "-q", "-f", "%e %U %S %M", "-o", str(time_path),
             "timeout", "--kill-after=5s", "600s", "taskset", "-c", "0-7", *argv],
            cwd=cwd, env=env, stdout=stdout, stderr=stderr, check=False,
        )
    stdout = (directory / "stdout").read_text(errors="replace")
    stderr = (directory / "stderr").read_text(errors="replace")
    timing = time_path.read_text().split()
    data = {
        "model": model, "arm": arm, "repetition": repetition, "profile": abi_profile,
        "workers_requested": workers, "exit": p.returncode, "argv": argv,
        "wall_s": float(timing[0]) if len(timing) == 4 else None,
        "cpu_s": float(timing[1]) + float(timing[2]) if len(timing) == 4 else None,
        "peak_rss_kib": int(timing[3]) if len(timing) == 4 else None,
        **parse_result(arm, stdout, stderr, p.returncode),
    }
    counters = re.findall(r"guard-spike (.*)", stderr)
    if counters:
        data["guard_spike"] = counters
    with (output / "cells.jsonl").open("a") as out:
        print(json.dumps(data, sort_keys=True), file=out)
    print(json.dumps({k: data.get(k) for k in ("model", "arm", "workers_requested", "status", "wall_s", "distinct", "generated", "depth")}), flush=True)
    if profile_perf and (directory / "perf.data").is_file() and (directory / "perf.data").stat().st_size:
        with (directory / "perf-report").open("w") as report:
            subprocess.run(
                ["perf", "report", "-i", str(directory / "perf.data"), "--stdio", "--percent-limit", "0.4",
                 "--no-children", "-g", "none", "--sort", "dso,symbol,srcline"],
                stdout=report, check=False,
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--part", choices=("profile", "panel", "spike", "spike-stats", "smoke"), required=True)
    parser.add_argument("--models", nargs="+", choices=MODELS, default=list(MODELS))
    parser.add_argument("--repetitions", type=int, default=3)
    args = parser.parse_args()
    root, output = args.root.resolve(), args.output.resolve()
    output.mkdir()
    digests = {str(f.relative_to(root)): hashlib.sha256(f.read_bytes()).hexdigest()
               for m in args.models for f in sorted((root / "models" / m).iterdir())
               if f.suffix in (".tla", ".cfg")}
    (output / "inputs.json").write_text(json.dumps(digests, sort_keys=True) + "\n")
    if args.part == "profile":
        for model in args.models:
            measure(root, output, model, 1, 0, "core", "tlc", profile_perf=True)
        measure(root, output, "lamport", 1, 0, "tlx_cold", "tlc", profile_perf=True)
        return
    if args.part == "spike-stats":
        for model in args.models:
            measure(root, output, model, 1, 0, "spike-stats", "tlc")
        return
    for model in args.models:
        for rep in range(args.repetitions if args.part != "smoke" else 1):
            for workers in (1, 8)[::(-1 if rep % 2 else 1)] if args.part != "smoke" else (1,):
                if args.part == "spike":
                    for profile in ("tlc", "dive"):
                        for arm in ("spike-off", "spike")[::(-1 if rep % 2 else 1)]:
                            measure(root, output, model, workers, rep, arm, profile)
                    continue
                blocks = [["core"], ["release"], ["tlc-pinned"], ["tlc-1.8.0"]]
                if model in ("counters", "liveness"):
                    blocks.append(["manyan"])
                if model in ("lamport", "kvs"):
                    for engine in ("tlx", "tla-mc"):
                        blocks.extend([[engine + "_interp"], [engine + "_cold", engine + "_warm"]])
                if args.part == "smoke":
                    blocks = [b for b in blocks if b[0] != "tlc-1.8.0"]
                for block in blocks[::(-1 if rep % 2 else 1)]:
                    for arm in block:
                        measure(root, output, model, workers, rep, arm, "tlc")


if __name__ == "__main__":
    main()
