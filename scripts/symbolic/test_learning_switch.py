#!/usr/bin/env python3
"""Check the published Learning Switch and distinguish a failed lemma from a reachable bug."""

import argparse
from itertools import product
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def run(dive, command, source, cfg, status, *options):
    result = subprocess.run(
        [dive, command, source, "--config", cfg, "--output", "json", *options],
        capture_output=True, text=True, timeout=45,
    )
    if result.returncode != status:
        raise RuntimeError((result.returncode, result.stdout, result.stderr))
    return json.loads(result.stdout)


def literal_set(rows):
    return "{" + ", ".join("<<" + ", ".join(json.dumps(x) for x in row) + ">>" for row in sorted(rows)) + "}"


NODES = ("n1", "n2", "n3")
PAIRS = tuple(product(NODES, repeat=2))
DIAGONAL = frozenset((node, node) for node in NODES)


def slice_cases(source):
    # Candidate only. Merge destinations in the pending relation for this
    # source and allow packets for other sources to erase the entire slice.
    initial = (DIAGONAL, frozenset())
    pending = [initial]
    seen = {initial}
    for table, hops in pending:
        successors = [(table, frozenset()), (table, hops | {(source, source)})]
        for old, current in sorted(hops):
            updated = table
            if source != current:
                updated |= {(x, y) for x, y in PAIRS if (x, current) in table and (old, y) in table}
            outgoing = frozenset((current, node) for node in NODES)
            successors.extend(((updated, outgoing), (updated, hops | outgoing)))
        for step in successors:
            if step not in seen:
                seen.add(step)
                pending.append(step)
    return pending


def strengthened_source(source):
    pieces = [source.rsplit("====", 1)[0], r"""
TableOf(s) == {<<x,y>> \in Node \X Node : <<s,x,y>> \in table}
HopsOf(s) == {<<x,y>> \in Node \X Node : \E d \in Node : <<s,d,x,y>> \in pending}
SliceInvariant == /\ Safety
"""]
    for node in NODES:
        for index, (table, hops) in enumerate(slice_cases(node)):
            prefix = "    /\\ (" if index == 0 else "         "
            pieces.append(
                f'{prefix}\\/ /\\ TableOf("{node}") = {literal_set(table)}\n'
                f'            /\\ HopsOf("{node}") = {literal_set(hops)}\n'
            )
        pieces.append("    )\n")
    return "".join(pieces) + "====\n"


def check(dive, case, root):
    original = case / "learning_switch.tla"
    original_cfg = original.with_suffix(".cfg")
    predicate = root / "safety.json"
    symbolic = ("--strategy", "symbolic", "--type-invariant", "TypeOK", "--time-limit", "30s")
    proof = run(dive, "check", original, original_cfg, 0, *symbolic, "--property-proof", "Safety",
                "--write-predicate", predicate)
    assert proof["proof"] == "inductive-safety" and proof["closure_sweeps"] == 0
    assert proof["transition_obligations"] == 3**2 + 3**5

    data = original.read_text()
    before = r'( (ps # sw1) /\ (\A w \in Node : w # sw1 => <<ps,sw1,w>> \notin table) )'
    # Keep the non-root guard. The only removed condition is the sink check.
    assert data.count(before) == 1
    edited = data.replace(before, "(ps # sw1)")
    mutant = root / original.name
    cfg = mutant.with_suffix(".cfg")
    mutant.write_text(edited)
    cfg.write_bytes(original_cfg.read_bytes())
    failed = run(dive, "check", mutant, cfg, 2, *symbolic, "--property-proof", "Safety",
                 "--verify-predicate", predicate)
    assert failed["proof"] == "inconclusive" and failed["failed_obligation"] == "Next closure"

    table = {(node, x, y) for node in NODES for x, y in DIAGONAL} | {("n1", "n3", "n2")}
    seed = edited.rsplit("====", 1)[0] + (
        f'AssumedInit == /\\ table = {literal_set(table)}\n'
        '               /\\ pending = {<<"n1","n1","n1","n3">>}\n'
        '               /\\ Safety\n====\n'
    )
    assumed = root / "assumed.cfg"
    assumed.write_text("INIT AssumedInit\nNEXT Next\nINVARIANT Safety\nCHECK_DEADLOCK FALSE\n")
    mutant.write_text(seed)
    cti = run(dive, "check", mutant, assumed, 10, "--store", "exact", "--workers", "1",
              "--exit-codes", "native", "--seed", "0", "--fp", "0", "--max-depth", "2")
    trace = root / "cti.json"
    trace.write_text(json.dumps(cti["counterexample"]["trace"]))
    assert len(cti["counterexample"]["trace"]["states"]) == 2
    assert run(dive, "validate-trace", mutant, assumed, 0, "--trace", trace)["result"] == "valid"
    assert run(dive, "validate-trace", mutant, cfg, 1, "--trace", trace)["result"] == "invalid"

    # Still using the released Init and edited Next, check the strengthened
    # predicate. The named proof independently checks every concrete action.
    mutant.write_text(strengthened_source(edited))
    cfg.write_text(cfg.read_text().replace("INVARIANT Safety", "INVARIANT SliceInvariant"))
    reachability = run(dive, "check", mutant, cfg, 0, *symbolic, "--property-proof", "SliceInvariant")
    assert reachability["proof"] == "inductive-safety" and reachability["closure_sweeps"] == 0
    return {
        "original_obligations": proof["transition_obligations"],
        "deleted_condition": "destination switch has no outgoing edge to another switch",
        "failed_obligation": failed["failed_obligation"],
        "hypothetical_witness_states": 2,
        "hypothetical_witness_replay": "valid under AssumedInit and invalid under original Init",
        "mutant_reachable_safety": "proved with SliceInvariant",
        "mutant_obligations": reachability["transition_obligations"],
        "projected_cases_per_source": [len(slice_cases(node)) for node in NODES],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("case", type=Path, help="pinned learning_switch_trans/3 directory")
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        print(json.dumps(check(args.dive.resolve(), args.case.resolve(), Path(directory))))
