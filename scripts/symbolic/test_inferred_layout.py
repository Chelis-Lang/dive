#!/usr/bin/env python3
"""Check inferred layouts against the original transition relation and exact Core."""

import argparse
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def run(dive, source, config, status, *options, command="check"):
    process = subprocess.run(
        [dive, command, source, "--config", config, "--output", "json", *options],
        capture_output=True, text=True, timeout=20,
    )
    if process.returncode != status:
        raise RuntimeError((process.returncode, process.stdout, process.stderr))
    return json.loads(process.stdout)


SYMBOLIC = ("--strategy", "symbolic", "--property-proof")
EXACT = ("--store", "exact", "--workers", "1", "--exit-codes", "native", "--seed", "0", "--fp", "0")


def compare(dive, source, config, prop, depth, directory):
    symbolic = run(dive, source, config, int(depth > 0), *SYMBOLIC, prop)
    explicit = run(dive, source, config, 10 if depth else 0, *EXACT)
    if not depth:
        assert symbolic["proof"] == "inductive-safety" and symbolic["layout"] == "inferred"
        assert explicit["verdict"] == "success"
        return
    assert symbolic["depth"] == depth and symbolic["trace_validated"]
    assert explicit["property"] == prop and len(explicit["counterexample"]["trace"]["states"]) == depth
    trace = directory / "witness.json"
    trace.write_text(json.dumps(symbolic["trace"]))
    assert run(dive, source, config, 0, "--trace", trace, command="validate-trace")["result"] == "valid"


def check_toy(dive, source, directory):
    config = source.with_suffix(".cfg")
    compare(dive, source, config, "Mutex", 0, directory)
    named = run(dive, source, config, 2, *SYMBOLIC, "Mutex", "--type-invariant", "TypeOK")
    assert named["verdict"] == "unsupported" and "operator named TypeOK" in named["reason"]
    original = source.read_text()
    mutant = directory / source.name
    unguarded = "    /\\ globalVar < Max\n"
    assert original.count(unguarded) == 1
    mutant.write_text(original.replace(unguarded, ""))
    failed = run(dive, mutant, config, 2, *SYMBOLIC, "Mutex")
    assert failed["verdict"] == "unsupported" and "assignment leaves bounds for globalVar" in failed["reason"]
    # Check the original inferred integer range directly on the mutant.
    body = mutant.read_text().rstrip().rsplit("\n", 1)[0]
    with_bound = body + "\n" + r"Within == globalVar \in 0..Max" + "\n====\n"
    mutant.write_text(with_bound)
    bound_config = directory / "Within.cfg"
    bound_config.write_text(config.read_text().replace("INVARIANT Mutex", "INVARIANT Within"))
    escaped = run(dive, mutant, bound_config, 10, *EXACT)
    trace = directory / "bound-escape.json"
    word = escaped["counterexample"]["trace"]
    assert escaped["property"] == "Within" and word["states"][-1]["values"]["globalVar"] == 2
    trace.write_text(json.dumps(word))
    assert run(dive, mutant, bound_config, 0, "--trace", trace, command="validate-trace")["result"] == "valid"
    enter = "EnterCS(proc) ==\n"
    protect = enter + "    /\\ isLocked = FALSE\n"
    assert original.count(protect) == 1
    mutant.write_text(original.replace(protect, enter))
    compare(dive, mutant, config, "Mutex", 3, directory)


def check_table(dive, directory):
    source = directory / "GrantTable.tla"
    original = r"""---- MODULE GrantTable ----
Peers == {"a","b"}
VARIABLE flags
Init == flags = [p \in Peers |-> FALSE]
Next == \E p \in Peers :
    \/ /\ \A q \in Peers : ~flags[q]
       /\ flags' = [flags EXCEPT ![p] = TRUE]
    \/ /\ flags[p]
       /\ flags' = [flags EXCEPT ![p] = FALSE]
OneGrant == \A p,q \in Peers : flags[p] /\ flags[q] => p=q
====
"""
    config = source.with_suffix(".cfg")
    config.write_text("INIT Init\nNEXT Next\nINVARIANT OneGrant\nCHECK_DEADLOCK FALSE\n")
    source.write_text(original)
    compare(dive, source, config, "OneGrant", 0, directory)
    source.write_text(original.replace(r"\A q \in Peers : ~flags[q]", "TRUE"))
    compare(dive, source, config, "OneGrant", 3, directory)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("--toy-mutex", type=Path, help="pinned Recomp-Verify ToyMutex.tla")
    args = parser.parse_args()
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        check_table(args.dive.resolve(), root)
        if args.toy_mutex:
            check_toy(args.dive.resolve(), args.toy_mutex.resolve(), root)
