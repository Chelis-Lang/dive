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


def check_i4(dive, case, root):
    original = case / "learning_switch_i4.tla"
    cfg = original.with_suffix(".cfg")
    symbolic = ("--strategy", "symbolic", "--type-invariant", "TypeOK", "--property-proof", "Safety",
                "--time-limit", "30s")
    exact = ("--store", "exact", "--workers", "1", "--seed", "0", "--fp", "0", "--exit-codes", "native")
    proof = run(dive, "check", original, cfg, 0, *symbolic)
    assert proof["proof"] == "inductive-safety" and proof["transition_obligations"] > 3

    data = original.read_text()
    diagonal = r"{<<n,x,y>> \in Node \X Node \X Node : x = y}"
    preserve = r"\/ <<pa,x,y>> \in route_tc"
    assert data.count(diagonal) == 1 and data.count(preserve) == 2
    edit = root / original.name
    trace = root / "word.json"
    cases = {}
    for label, edited, depth in (
        ("initial", data.replace(diagonal, "{}"), 1),
        ("flood", data.replace(preserve, r"\/ (x # y /\ <<pa,x,y>> \in route_tc)", 1), 4),
    ):
        edit.write_text(edited)
        row = run(dive, "check", edit, cfg, 1, *symbolic, "--trace", trace)
        assert row["trace_validated"] and row["depth"] == depth and row["ending"]["name"] == "Safety", row
        assert run(dive, "validate-trace", edit, cfg, 0, "--trace", trace)["result"] == "valid"
        assert run(dive, "validate-trace", original, cfg, 1, "--trace", trace)["result"] == "invalid"
        cases[label] = {"witness_states": depth, "replay_on_edit": "valid", "replay_on_original": "invalid"}

    # Restore the original four-node program, then compare the same relation
    # in a finite explicit control with two nodes.
    edit.write_text(data)
    restored = run(dive, "check", edit, cfg, 0, *symbolic)
    assert restored["proof"] == "inductive-safety"
    carrier = '{"n1","n2","n3","n4"}'
    assert data.count(carrier) == 1
    small = data.replace(carrier, '{"n1","n2"}')
    edit.write_text(small)
    exact_report = run(dive, "check", edit, cfg, 0, *exact)
    assert exact_report["verdict"] == "success"
    assert run(dive, "check", edit, cfg, 0, *symbolic)["proof"] == "inductive-safety"
    edit.write_text(small.replace(preserve, r"\/ (x # y /\ <<pa,x,y>> \in route_tc)", 1))
    bad = run(dive, "check", edit, cfg, 10, *exact)
    witness = bad["counterexample"]["trace"]
    assert bad["property"] == "Safety" and len(witness["states"]) == cases["flood"]["witness_states"]
    trace.write_text(json.dumps(witness))
    assert run(dive, "validate-trace", edit, cfg, 0, "--trace", trace)["result"] == "valid"
    return {"proof": proof["proof"], "obligations": proof["transition_obligations"],
            "closure_sweeps": proof["closure_sweeps"], "restored": restored["proof"],
            "edits": cases, "two_node_exact": exact_report["statistics"]}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("case", type=Path, help="pinned learning-switch configuration directory")
    parser.add_argument("--i4", action="store_true", help="check the four-node i4 variant")
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        accept = check_i4 if args.i4 else check
        print(json.dumps(accept(args.dive.resolve(), args.case.resolve(), Path(directory))))
