#!/usr/bin/env python3
"""Compare named proofs for finite tuple relations with concrete Core witnesses."""

import argparse
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

SOURCE = r"""---- MODULE Families ----
EXTENDS FiniteSets, Naturals
CONSTANT IDs
VARIABLES chosen, seen
Pairs == IDs \X IDs
TypeOK == chosen \in SUBSET IDs /\ seen \in SUBSET Pairs
Init == /\ chosen \in SUBSET IDs
        /\ seen = {}
Next == \/ \E q \in SUBSET chosen : \E i \in q :
             /\ seen' = seen \cup {<<i,i>>}
             /\ UNCHANGED chosen
        \/ UNCHANGED <<chosen,seen>>
Diagonal == \A <<i,j>> \in seen : i = j /\ i \in chosen
Empty == seen = {}
InitiallyEmpty == chosen = {}
BadPattern == \A <<i,j,k>> \in seen : i = j
====
"""


def check(cmd, expected):
    run = subprocess.run(cmd, text=True, capture_output=True, timeout=20)
    assert run.returncode == expected, (run.returncode, run.stdout[-500:], run.stderr)
    return json.loads(run.stdout)


def run(dive, root):
    module = root / "Families.tla"
    module.write_text(SOURCE)
    cfg = module.with_suffix(".cfg")
    options = [module, "--config", cfg, "--output", "json"]
    named = [dive, "check", *options, "--strategy", "symbolic", "--time-limit", "15s",
             "--type-invariant", "TypeOK", "--property-proof"]
    core = [dive, "check", *options, "--store", "exact", "--workers", "1", "--seed", "0",
            "--fp", "0", "--exit-codes", "native"]
    for prop, depth in (("Diagonal", 0), ("InitiallyEmpty", 1), ("Empty", 2)):
        cfg.write_text(f"CONSTANT IDs = {{a,b,c}}\nINIT Init NEXT Next INVARIANT {prop}\n")
        checked = check(named + [prop], int(depth > 0))
        explicit = check(core, 10 if depth else 0)
        if depth:
            assert checked["verdict"] == "invariant-violation" and checked["trace_validated"], checked
            assert checked["trace"]["ending"] == explicit["counterexample"]["trace"]["ending"]
            assert len(checked["trace"]["states"]) == len(explicit["counterexample"]["trace"]["states"]) == depth
        else:
            assert checked["proof"] == "inductive-safety" and explicit["verdict"] == "success"
    cfg.write_text("CONSTANT IDs = {a,b}\nINIT Init NEXT Next INVARIANT BadPattern\n")
    refused = check(named + ["BadPattern"], 2)
    assert refused["verdict"] == "unsupported" and "arity" in refused["reason"], refused
    # More factors than the bounded subset enumerator accepts must refuse.
    cfg.write_text("CONSTANT IDs = {" + ",".join(f"p{i}" for i in range(13)) +
                   "}\nINIT Init NEXT Next INVARIANT Diagonal\n")
    assert "4096" in check(named + ["Diagonal"], 2)["reason"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    args = parser.parse_args()
    with TemporaryDirectory() as folder:
        run(args.dive.resolve(), Path(folder))
