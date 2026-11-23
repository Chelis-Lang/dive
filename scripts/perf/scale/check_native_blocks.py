#!/usr/bin/env python3
"""Exercise the optional source-built blocks against a stock CLI on small models."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile

PREPARE = Path(__file__).resolve().parent / "native-successors/prepare.py"

SOURCE = r"""---- MODULE Probe ----
EXTENDS Naturals, TLC
CONSTANTS Actors, Paths, Nil, Initial, Missing
VARIABLES w, hist
vars == <<w,hist>>
Init == /\ w=[s \in Actors |-> [pc |-> Initial, data |-> [p \in Paths |-> Nil]]]
        /\ hist=[s \in Actors |-> [p \in Paths |-> {}]]
Arm(s,label,following,z) ==
    /\ w[s].pc=label
    /\ w'=[w EXCEPT ![s].pc=following, ![s].data=[p \in Paths |-> z]]
History ==
    hist'=[s \in Actors |-> [p \in Paths |->
        hist[s][p] \cup (IF w'[s].data[p] # Nil /\ w'[s].data[p] # w[s].data[p]
                         THEN {w'[s].data[p][2]} ELSE {})]]
Next == \E s \in Actors :
    /\ \/ Arm(s,"a","b",<<s, 1>>)
       \/ Arm(s,"a","b",<<s, 1>>)
       \/ Arm(s,"b","c",<<s, 2>>)
       \/ Arm(s,"c","a",Nil)
    /\ History
====
"""
CONFIG = 'INIT Init\nNEXT Next\nCHECK_DEADLOCK FALSE\nCONSTANTS Actors={A,B} Paths={P} Nil=Nil Initial="a" Missing=MISSING\n'


def cases(directory):
    return [
        ("duplicate-and-changed-cells", SOURCE, CONFIG, []),
        ("reclaim", SOURCE, CONFIG, ["--store-compact-bytes", "0"]),
        ("bounds", SOURCE, CONFIG, ["--max-depth", "2"]),
        (
            "spill",
            SOURCE,
            CONFIG,
            ["--candidate-memory-bytes", "0", "--candidate-directory", str(directory / "spill")],
        ),
        ("nonstring-label", SOURCE, CONFIG.replace('Initial="a"', "Initial=1"), []),
        ("empty-actor", SOURCE, CONFIG.replace("Actors={A,B}", "Actors={}"), []),
        ("empty-inner", SOURCE, CONFIG.replace("Paths={P}", "Paths={}"), []),
        ("changed-singleton-index", SOURCE.replace("data[p][2]", "data[p][1]"), CONFIG, []),
        ("changed-payload-error", SOURCE.replace("data[p][2]", "data[p][19]"), CONFIG, []),
        ("alternate-key-type", SOURCE, CONFIG.replace("Paths={P}", "Paths={1,2}"), []),
        (
            "earliest-failing-source",
            SOURCE.replace('Arm(s,"a","b",<<s, 1>>)', 'Arm(s,"a","b",<<s, 1 \\div 0>>)', 1),
            CONFIG,
            [],
        ),
        (
            "short-circuit-error",
            SOURCE.replace('Arm(s,"b","c",<<s, 2>>)', 'Arm(s,"z","c",<<s, 1 \\div 0>>)'),
            CONFIG,
            [],
        ),
    ]


def invoke(command, directory, environment=None):
    run = subprocess.run(
        list(map(str, command)),
        cwd=directory,
        capture_output=True,
        env={**os.environ, **(environment or {})},
        timeout=180,
    )
    return run.returncode, json.loads(run.stdout)


def observables(status, report):
    stats = report.get("statistics")
    keys = ("distinct", "generated", "depth")
    return {
        "exit": status,
        "verdict": report["verdict"],
        "property": report.get("property"),
        "diagnostic": report.get("message"),
        "warnings": report.get("warnings"),
        "stats": {k: stats.get(k) for k in keys} if stats else None,
    }


def run(args, directory):
    (directory / "spill").mkdir()
    module = directory / "Probe.tla"
    config = directory / "Probe.cfg"
    for profile in ("dive", "tlc"):
        module.write_text(SOURCE)
        config.write_text(CONFIG)
        status, build = invoke(
            [
                "python3",
                PREPARE,
                module,
                "--config",
                config,
                "--cache",
                args.cache.resolve(),
                "--jobs",
                args.jobs,
                "--profile",
                profile,
            ],
            directory,
        )
        if status:
            raise RuntimeError(build)
        for name, source, cfg, options in cases(directory):
            module.write_text(source)
            config.write_text(cfg)
            command = [
                "check",
                module,
                "--config",
                config,
                "--workers",
                "4" if name == "spill" else "1",
                "--seed",
                "0",
                "--fp",
                "0",
                "--store",
                "exact",
                "--output",
                "json",
                *options,
            ]
            if profile == "tlc":
                command.append("--compat=tlc")
            control = observables(*invoke([args.stock.resolve(), *command], directory))
            disabled = observables(
                *invoke([build["binary"], *command], directory, {"DIVE_DISABLE_NATIVE_BLOCKS": "1"})
            )
            meter = directory / "block-meter"
            actions = directory / "action-meter"
            for path in (meter, actions):
                path.unlink(missing_ok=True)
            status, checked = invoke(
                [build["binary"], *command],
                directory,
                {
                    "DIVE_NATIVE_BLOCK_METER": str(meter),
                    "DIVE_NATIVE_SUCCESSOR_METER": str(actions),
                },
            )
            actual = observables(status, checked)
            if actual != control or disabled != control:
                raise ValueError((name, profile, actual, control, disabled))
            expected = {
                "bounds": "stopped",
                "nonstring-label": "evaluation-error",
                "earliest-failing-source": "evaluation-error",
                "changed-payload-error": "unsupported",
            }.get(name, "success")
            if actual["verdict"] != expected:
                raise ValueError((name, profile, expected, actual))
            hits = {}
            for line in meter.read_text().splitlines() if meter.exists() else []:
                event, lower_bound = line.split()
                hits[event] = max(hits.get(event, 0), int(lower_bound))
            metrics = (checked.get("core") or {}).get("metrics") or {}
            if name in ("duplicate-and-changed-cells", "reclaim"):
                if not actions.exists() or hits.get("HistoryChanged", 0) == 0:
                    raise ValueError((name, profile, "expected a genuine sparse update", hits))
            if name == "reclaim" and not (metrics.get("store_reclamation") or {}).get(
                "collections"
            ):
                raise ValueError("expected a Store collection")
            if name == "spill" and not (metrics.get("candidate_storage") or {}).get(
                "committed_spill_bytes"
            ):
                raise ValueError("expected a committed speculative spill")
            print(
                json.dumps(
                    {
                        "case": name,
                        "profile": profile,
                        **actual,
                        "events_lower_bound": hits,
                        "native": actions.exists(),
                        "collections": metrics.get("store_reclamation", {}).get("collections"),
                        "spill_bytes": metrics.get("candidate_storage", {}).get(
                            "committed_spill_bytes"
                        ),
                    }
                ),
                flush=True,
            )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock", type=Path, required=True)
    parser.add_argument("--cache", type=Path, required=True)
    parser.add_argument("--jobs", type=int, default=4)
    with tempfile.TemporaryDirectory() as temporary:
        run(parser.parse_args(), Path(temporary))
