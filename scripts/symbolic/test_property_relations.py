#!/usr/bin/env python3
"""Compare named proofs for finite tuple relations with concrete Core witnesses."""

import argparse
import hashlib
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
Canonical == chosen = chosen /\ <<{"z","a"}, {i \in IDs : i # i}>> = <<{"a","z"}, {}>>
Conditional == \A <<i,j>> \in seen : IF i \in chosen THEN i # j ELSE TRUE
StateTuple == <<chosen>> = <<IDs>>
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
    for prop, depth in (("Diagonal", 0), ("Canonical", 0), ("InitiallyEmpty", 1), ("Empty", 2), ("Conditional", 2)):
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
    cfg.write_text("CONSTANT IDs = {a,b}\nINIT Init NEXT Next INVARIANT StateTuple\n")
    refused = check(named + ["StateTuple"], 2)
    assert refused["verdict"] == "unsupported" and "nonconstant domain" in refused["reason"], refused
    # More factors than the bounded subset enumerator accepts must refuse.
    cfg.write_text("CONSTANT IDs = {" + ",".join(f"p{i}" for i in range(13)) +
                   "}\nINIT Init NEXT Next INVARIANT Diagonal\n")
    assert "4096" in check(named + ["Diagonal"], 2)["reason"]


def check_client_server(dive, source, root):
    """Remove a real response-matching guard and replay the shortest violation."""
    data = source.read_bytes()
    assert hashlib.sha256(data).hexdigest() == "91d0a91cf1c83255c486cc5d50fd9f5ca5966dc96e293bba4599bfeee41eeb83"
    guard = r"    /\ <<r,p>> \in match"
    original = data.decode()
    assert original.count(guard) == 1
    module = root / source.name
    module.write_text(original.replace(guard, ""))
    cfg = module.with_suffix(".cfg")
    cfg.write_bytes(source.with_suffix(".cfg").read_bytes())
    row = check([dive, "check", module, "--config", cfg, "--strategy", "symbolic",
                 "--type-invariant", "TypeOK", "--property-proof", "Safety", "--output", "json"], 1)
    assert row["trace_validated"] and row["verdict"] == "invariant-violation" and row["depth"] == 4


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("--client-server", type=Path, help="original Recomp client_server_ae/2-4-2 module")
    args = parser.parse_args()
    with TemporaryDirectory() as folder:
        run(args.dive.resolve(), Path(folder))
        if args.client_server:
            check_client_server(args.dive.resolve(), args.client_server, Path(folder))
