#!/usr/bin/env python3
"""Check original Elevator fairness and remove one SF obligation as a witness control."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import time

SPEC = "c44366dbf4f4bdef30edade0c8a2faf75fd37b2b7aeb1c04b24e80eb40ed2203"
CFG = "3e6d67656512a5d2e4795c7de9c13d510096608e506c56f31f0fa4eb618d9290"
OMIT = r"    /\ \A e \in Elevator : SF_Vars(MoveElevator(e))"


def sha(data):
    return hashlib.sha256(data).hexdigest()


def invocation(args, cwd):
    rss = cwd / "rss"
    start = time.monotonic()
    proc = subprocess.run(["/usr/bin/time", "-qf", "%M", "-o", rss, "timeout", "-k", "3s", "30s", *args],
                          cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, check=False)
    return proc.stdout, {"wall_s": round(time.monotonic() - start, 5), "exit": proc.returncode,
                         "peak_rss_kib": int(rss.read_text().strip())}


def replay(dive, work, trace):
    path = work / "witness.json"
    path.write_text(json.dumps(trace))
    output, metrics = invocation([dive, "validate-trace", "Elevator.tla", "--config", "check.cfg",
                                  "--trace", path, "--seed", "0", "--fp", "0", "--output", "json"], work)
    replayed = json.loads(output)
    return {"result": replayed["result"], "exit": metrics["exit"]}


def panel(source, dive, manyan):
    original = (source / "Elevator.tla").read_bytes()
    cfg = (source / "ElevatorLivenessMedium.cfg").read_bytes()
    if sha(original) != SPEC or sha(cfg) != CFG:
        raise ValueError("Elevator inputs differ from the published pin")
    weakened = original.decode().replace(OMIT + "\n", "").encode()
    if original == weakened:
        raise ValueError("SF edit did not apply")
    small = cfg.replace(b"{p1, p2}", b"{p1}").replace(b"{e1, e2}", b"{e1}")
    rows = []
    for name, spec, config in [("small-fair", original, small), ("small-no-move-SF", weakened, small),
                               ("medium-fair", original, cfg)]:
        with tempfile.TemporaryDirectory() as td:
            work = Path(td)
            (work / "Elevator.tla").write_bytes(spec)
            (work / "check.cfg").write_bytes(config)
            row = {"case": name, "module_sha256": sha(spec), "cfg_sha256": sha(config)}
            out, many = invocation([manyan, "-workers", "1", "-fp", "0", "-seed", "0", "-config", "check.cfg",
                                    "Elevator.tla"], work)
            ok = many["exit"] == 0 and "Model checking completed. No error has been found." in out
            violated = many["exit"] == 13 and "Temporal property TemporalInvariant was violated." in out
            many["status"] = "success" if ok else "violation" if violated else "unclassified"
            if counts := re.search(r"(\d+) states generated, (\d+) distinct states found", out):
                many.update(generated=int(counts[1]), distinct=int(counts[2]))
            if not (ok or violated):
                many["diagnostic"] = next((line for line in out.splitlines() if "Error:" in line), None)
            out, core = invocation([dive, "check", "Elevator.tla", "--workers", "1", "--seed", "0", "--fp", "0",
                                    "--config", "check.cfg", "--output", "json"], work)
            result = json.loads(out)
            stats = result.get("statistics") or {}
            core.update(status=result["verdict"], property=result["property"],
                        distinct=stats.get("distinct"), generated=stats.get("generated"), depth=stats.get("depth"))
            if cex := result.get("counterexample"):
                trace = cex["trace"]
                core.update(witness_states=len(trace["states"]), ending=trace["ending"])
                row["replay"] = replay(dive, work, trace)
                # Keep the original prefix but replace the cycle by stuttering at the initial state.
                tampered = copy.deepcopy(trace)
                tampered["states"] = tampered["states"][:1]
                tampered.update(length=1, ending={"kind": "lasso", "back_to": 0})
                row["initial_stutter_replay"] = replay(dive, work, tampered)
                (work / "Elevator.tla").write_bytes(original)
                row["original_fairness_replay"] = replay(dive, work, trace)
            row.update(manyan=many, dive=core)
            rows.append(row)
    return rows


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="pinned Examples specifications/MultiCarElevator")
    parser.add_argument("dive", type=Path)
    parser.add_argument("manyan", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    report = {"model": "MultiCarElevator", "examples": "6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f",
              "property": "TemporalInvariant", "safety": ["TypeInvariant", "SafetyInvariant"],
              "workers": 1, "wall_limit_seconds": 30, "removed_fairness_conjunct": OMIT.strip(),
              "dive_sha256": sha(args.dive.read_bytes()), "manyan_sha256": sha(args.manyan.read_bytes()),
              "rows": panel(args.source, args.dive.resolve(), args.manyan.resolve())}
    with args.output.open("x") as file:
        json.dump(report, file, indent=2)
        file.write("\n")
