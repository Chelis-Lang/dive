#!/usr/bin/env python3
"""Measure installed whole-value checks across separate Dive and Sail processes."""

import argparse
from hashlib import sha256
import json
from math import ceil
import os
from pathlib import Path
from statistics import median
from tempfile import TemporaryDirectory

from recomp_property_panel import measured


def measure(command, directory, group, seconds, key=None):
    environment = ["env", "-u", "DIVE_SYMBOLIC_PROOF_KEY"]
    if key is not None:
        environment.append(f"DIVE_SYMBOLIC_PROOF_KEY={key}")
    code, wall, output, memory = measured([*environment, *command], directory, seconds, group)
    return {"exit": code, "wall_s": wall, "peak_cgroup_bytes": memory}, json.loads(output)


def benchmark(args, work):
    model = args.model.resolve()
    module = model / "EmergencyCall_Model_auto.tla"
    if args.f3:
        init, goals = "Init3", "TypeInvariant SetInvariant F3"
    else:
        init, goals = "Init", "TypeInvariant SetInvariant"
    configs = {}
    for name, invariants in {"full": goals, "reduced": "TypeInvariant", "added": "TypeInvariant F1"}.items():
        path = work / f"{name}.cfg"
        path.write_text(f"INIT {init}\nNEXT Next\nINVARIANT {invariants}\n")
        configs[name] = path
    session = work / "session"
    key = work / "key"
    with open(key, "xb", opener=lambda path, flags: os.open(path, flags, 0o600)) as stream:
        stream.write(os.urandom(32))
    candidate = session / "sail-symbolic-predicate-v1.json"

    def dive_run(config, *options):
        return [args.dive, "check", module, "--config", config, "--strategy", "symbolic",
                "--time-limit", f"{args.seconds}s", "--output", "json", *options]

    def sail_run(config):
        return [args.sail, "runtime", "symbolic-check", "--dive", args.dive, "--session", session,
                "--module", module, "--config", config, "--time-limit", str(args.seconds)]

    def run(command, key=None):
        with TemporaryDirectory(dir=work) as tmp:
            return measure(command, Path(tmp), args.cgroup_parent, args.seconds + 1, key)

    establishment, first = run(sail_run(configs["full"]), key)
    assert establishment["exit"] == 0 and first["encoding"] == "whole-values" and first["verdict"] == "success"
    if not args.f3:
        assert (first["distinct"], first["generated"], first["depth"]) == (49_911_672_768, 328_063_574_184, 44)
    counters = {name: first[name] for name in ("initial", "distinct", "generated", "depth")}
    result = {"source_sha256": sha256(module.read_bytes()).hexdigest(), "f3": args.f3, "artifact_bytes": candidate.stat().st_size,
              "establishment": {**establishment, **counters}, "config_text": {name: path.read_text() for name, path in configs.items()},
              "holding": {}}
    print(json.dumps({"progress": "established", **establishment, "artifact_bytes": candidate.stat().st_size}), flush=True)
    for name in ("reduced", "full"):
        config = configs[name]
        commands = {
            "cold": (dive_run(config), None),
            "unsigned": (dive_run(config, "--verify-predicate", candidate), None),
            "sail_authorized": (sail_run(config), key),
        }
        samples = {tool: [] for tool in commands}
        for trial in range(args.rounds):
            for tool in list(commands)[::1 if trial % 2 == 0 else -1]:
                timing, row = run(*commands[tool])
                assert timing["exit"] == 0, (tool, timing, row)
                if tool == "cold":
                    assert row["verdict"] == "success" and all(row[k] == v for k, v in counters.items())
                else:
                    assert row["proof"] == "inductive-safety"
                    assert row["authorized_closure"] is (tool == "sail_authorized")
                    assert all(field not in row for field in ("initial", "distinct", "generated", "depth"))
                samples[tool].append(timing)
                print(json.dumps({"progress": name, "mode": tool, **timing}), flush=True)
        medians = {tool: median(row["wall_s"] for row in rows) for tool, rows in samples.items()}
        saving = medians["cold"] - medians["sail_authorized"]
        result["holding"][name] = {
            "samples": samples, "median_wall_s": medians,
            "authorized_speedup": medians["cold"] / medians["sail_authorized"],
            "break_even_edits_including_establishment": ceil(establishment["wall_s"] / saving) if saving > 0 else None,
            "break_even_edits_if_cold_establishment_already_required":
                max(0, ceil((establishment["wall_s"] - medians["cold"]) / saving)) if saving > 0 else None,
        }
    if not args.f3:
        timings = {}
        for name, command, signing in (
            ("cold", dive_run(configs["added"]), None), ("sail", sail_run(configs["added"]), key),
        ):
            timing, row = run(command, signing)
            assert timing["exit"] == 1 and row["trace_validated"] and row["ending"]["name"] == "F1"
            trace = row["trace"]
            assert len(trace["states"]) == 7
            path = work / "witness.json"
            validate = [args.dive, "validate-trace", module, "--config", configs["added"],
                        "--compat=tlc", "--trace", path, "--output", "json"]
            path.write_text(json.dumps(trace))
            status, reply = run(validate)
            assert status["exit"] == 0 and reply["result"] == "valid"
            trace["states"].pop(1)
            path.write_text(json.dumps(trace))
            status, reply = run(validate)
            assert status["exit"] == 1 and reply["result"] == "invalid"
            timings[name] = {**timing, "witness_states": 7, "replay": "valid", "damaged_replay": "invalid"}
        result["added_F1"] = timings
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path, help="pinned EmergencyCall model directory")
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--sail", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=180)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument("--cgroup-parent", type=Path)
    parser.add_argument("--f3", action="store_true", help="use the smaller development Init")
    args = parser.parse_args()
    if args.seconds < 1 or args.rounds < 1:
        parser.error("expected positive seconds and rounds")
    args.dive = args.dive.resolve()
    args.sail = args.sail.resolve()
    with TemporaryDirectory() as tmp:
        print(json.dumps(benchmark(args, Path(tmp))))


if __name__ == "__main__":
    main()
