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


def check_function_init(dive, root):
    module = root / "InitialTable.tla"
    module.write_text(r"""---- MODULE InitialTable ----
EXTENDS Integers
CONSTANT IDs
VARIABLE table, touched
Layout == table \in [IDs -> SUBSET IDs] /\ touched \in BOOLEAN
Init == /\ table \in [IDs -> SUBSET IDs]
        /\ \A i \in IDs : i \in table[i]
        /\ touched = FALSE
Next == /\ \E i \in IDs : (i \in table[i]) \/ TRUE
        /\ touched' = TRUE /\ UNCHANGED table
Reflexive == \A i \in IDs : i \in table[i]
Untouched == ~touched
Arithmetic == ((0-1) % 3 = 2) /\ ((0-1) \notin Nat) /\ (0 \in Int) /\ touched=touched
BooleanEqual == (touched = (touched /\ TRUE)) /\ ((~touched) = (touched = FALSE))
SetEqual == \A i \in IDs : (table[i] \cap {} = {}) /\ (table[i] # {})
BadEquality == touched = 0
BadMod == (1 % 0 = 0) /\ touched=touched
Single == \A i \in IDs : table[i] = {i}
====
""")
    cfg = module.with_suffix(".cfg")
    core = [dive, "check", module, "--config", cfg, "--output", "json", "--store", "exact",
            "--workers", "1", "--seed", "0", "--fp", "0", "--exit-codes", "native"]
    named = [dive, "check", module, "--config", cfg, "--output", "json", "--strategy",
             "symbolic", "--type-invariant", "Layout", "--property-proof"]
    for prop, depth in (("Reflexive", 0), ("Arithmetic", 0), ("BooleanEqual", 0),
                        ("SetEqual", 0), ("Single", 1), ("Untouched", 2)):
        cfg.write_text(f"CONSTANT IDs = {{a,b,c}}\nINIT Init NEXT Next INVARIANT {prop}\n")
        row = check(named + [prop], int(bool(depth)))
        explicit = check(core, 10 if depth else 0)
        if depth:
            assert row["trace_validated"] and row["depth"] == depth
            assert row["ending"]["name"] == explicit["counterexample"]["trace"]["ending"]["name"] == prop
        else:
            assert row["proof"] == "inductive-safety" and explicit["verdict"] == "success"
    cfg.write_text("CONSTANT IDs = {a,b}\nINIT Init NEXT Next INVARIANT BadMod\n")
    row = check(named + ["BadMod"], 2)
    assert row["verdict"] == "unsupported" and "positive" in row["reason"], row
    cfg.write_text("CONSTANT IDs = {a,b}\nINIT Init NEXT Next INVARIANT BadEquality\n")
    row = check(named + ["BadEquality"], 2)
    assert row["verdict"] == "unsupported" and "incomparable finite values" in row["reason"], row
    cfg.write_text("CONSTANT IDs = {a,b,c,d,e}\nINIT Init NEXT Next INVARIANT Reflexive\n")
    row = check(named + ["Reflexive"], 2)
    assert row["verdict"] == "unsupported" and "enumeration budget" in row["reason"], row


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


def check_lockserver(dive, source, root):
    cfg = source.with_suffix(".cfg")
    base = [dive, "check"]
    named = ["--strategy", "symbolic", "--type-invariant", "TypeOK", "--property-proof", "Inv",
             "--output", "json"]
    exact = ["--store", "exact", "--workers", "1", "--seed", "0", "--fp", "0",
             "--exit-codes", "native", "--output", "json"]
    proven = check([*base, source, "--config", cfg, *named], 0)
    assert proven["proof"] == "inductive-safety"
    mutant = root / source.name
    grant = "semaphore' = [semaphore EXCEPT ![s] = FALSE]"
    original = source.read_text()
    assert original.count(grant) == 1
    mutant.write_text(original.replace(grant, "UNCHANGED semaphore"))
    bad = check([*base, mutant, "--config", cfg, *named], 1)
    assert bad["trace_validated"] and bad["depth"] == 3 and bad["ending"]["name"] == "Inv"
    word = root / "double-grant.json"
    word.write_text(json.dumps(bad["trace"]))
    replay = [dive, "validate-trace", "--config", cfg, "--trace", word, "--output", "json"]
    assert check([*replay, mutant], 0)["result"] == "valid"
    assert check([*replay, source], 1)["result"] == "invalid"
    for profile in ("dive", "tlc"):
        original_row = check([*base, source, "--config", cfg, *exact, f"--compat={profile}"], 0)
        stats = original_row["statistics"]
        assert (stats["distinct"], stats["generated"], stats["depth"]) == (9, 25, 3)
        explicit = check([*base, mutant, "--config", cfg, *exact, f"--compat={profile}"], 10)
        path = explicit["counterexample"]["trace"]
        assert explicit["property"] == "Inv" and len(path["states"]) == 3
        word.write_text(json.dumps(path))
        assert check([*replay, mutant], 0)["result"] == "valid"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("--client-server", type=Path, help="original Recomp client_server_ae/2-4-2 module")
    parser.add_argument("--lockserver", type=Path, help="original Recomp lockserver/2-2 module")
    args = parser.parse_args()
    with TemporaryDirectory() as folder:
        run(args.dive.resolve(), Path(folder))
        check_function_init(args.dive.resolve(), Path(folder))
        if args.client_server:
            check_client_server(args.dive.resolve(), args.client_server, Path(folder))
        if args.lockserver:
            check_lockserver(args.dive.resolve(), args.lockserver, Path(folder))
