#!/usr/bin/env python3
"""Compare the static Sailfish insertion predicates with every vertex of a small TLC graph."""

import argparse
import json
import re
from collections import defaultdict
from pathlib import Path

import z3

from sailfish_dag import CORRECT, Dag, all_of

VERTEX = r"<<n([1-4]), (\d+)>>"
ARC = re.compile(r"<<" + VERTEX + r", (?:<<>>|" + VERTEX + r")>>")


def snapshot(label):
    label = label.replace("\\n ", " ")
    variables = dict(re.findall(r"(log|vs|es|round) = (.*?)(?=\\n|$)", label))
    present = frozenset((int(r), int(i) - 1) for i, r in re.findall(VERTEX, variables["vs"]))
    edges = set()
    for node, r, parent, previous in ARC.findall(variables["es"]):
        r, node = int(r), int(node) - 1
        if not parent:
            assert r == 1 and (r, node) in present
        else:
            j = int(parent) - 1
            assert int(previous) == r - 1 and (r - 1, j) in present and (r, node) in present
            edges.add((r, node, j))
    assert len(ARC.findall(variables["es"])) == variables["es"].count("<<<<")
    observed = [(int(i) - 1, int(r)) for i, r in re.findall(r"n([2-4]) :> (\d+)", variables["round"])]
    assert len(observed) == len(CORRECT)
    assert all(r == max((k for k, j in present if j == i), default=0) for i, r in observed)
    return present, frozenset(edges)


def audit(dot, rounds):
    states, adjacent = {}, defaultdict(set)
    for line in dot.read_text().splitlines():
        edge = re.match(r"(-?\d+) -> (-?\d+)\s", line)
        if edge:
            adjacent[edge[1]].add(edge[2])
        else:
            state = re.match(r'(-?\d+) \[label="([^"]*)"', line)
            if state:
                states[state[1]] = snapshot(state[2])
    if not states:
        raise ValueError("no TLC states found")
    dag = Dag(rounds)
    admissible, steps = all_of(*dag.admissible()), list(dag.steps())
    ground = z3.Solver()
    for origin, (present, edges) in states.items():
        bindings = [atom if key in present else z3.Not(atom) for key, atom in dag.present.items()]
        bindings.extend(atom if key in edges else z3.Not(atom) for key, atom in dag.edges.items())
        assert ground.check(*bindings) == z3.sat
        model = ground.model()
        if not z3.is_true(model.eval(admissible)):
            raise ValueError(f"TLC state {origin} excluded by the static assumptions")
        actual = set()
        for target in adjacent[origin]:
            if target == origin:
                actual.add("Terminate")
                continue
            next_present, next_edges = states[target]
            (r, i), = next_present - present
            assert present <= next_present and edges <= next_edges
            added = next_edges - edges
            assert all((k, j) == (r, i) for k, j, _ in added)
            actual.add((r, i, tuple(sorted(j for _, _, j in added))))
        predicted = {key for key, guard in steps if z3.is_true(model.eval(guard))}
        if all((rounds, i) in present for i in CORRECT):
            predicted.add("Terminate")
        if predicted != actual:
            raise ValueError(f"TLC state {origin}: predicted {predicted - actual}, missed {actual - predicted}")
    return {"states": len(states), "edges": sum(map(len, adjacent.values()))}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dot", type=Path)
    parser.add_argument("--rounds", type=int, choices=(1, 2), required=True)
    args = parser.parse_args()
    print(json.dumps(audit(args.dot, args.rounds)))
