#!/usr/bin/env python3
"""Compare symbolic successors and initial states with the complete small TLC graph."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re

from ewd998 import Ring


def state(ring, label):
    text = json.loads(label)
    bindings = {}
    for prefix, fields in (("active", ring.active), ("color", ring.color), ("counter", ring.counter), ("pending", ring.pending)):
        body = re.search(rf"\b{prefix} = \(([^)]*)\)", text)[1]
        items = re.findall(r'(\d+) :> ("(?:black|white)"|FALSE|TRUE|-?\d+)', body)
        assert len(items) == len(fields)
        for index, raw in items:
            if prefix in ("color", "active"):
                value = int(raw in ('"black"', "TRUE"))
            else:
                value = int(raw)
            bindings[fields[int(index)]] = value
    record = re.search(r"token = \[([^\]]+)\]", text)[1]
    bindings["pos"] = int(re.search(r"pos \|-> (\d+)", record)[1])
    bindings["q"] = int(re.search(r"q \|-> (-?\d+)", record)[1])
    bindings["black"] = int(re.search(r'color \|-> "(white|black)"', record)[1] == "black")
    assert bindings.keys() == ring.values.keys()
    return ring.conjunction(ring.eq(key, value) for key, value in bindings.items())


def compare(path):
    ring = Ring(2)
    states, initial, edges = {}, set(), defaultdict(set)
    with path.open() as graph:
        for line in graph:
            arrow = re.match(r"(-?\d+) -> (-?\d+)\s", line)
            if arrow:
                edges[arrow[1]].add(arrow[2])
                continue
            match = re.match(r'(-?\d+) \[label=("(?:\\.|[^"\\])*")', line)
            if match:
                key = match[1]
                if key in states:
                    raise ValueError(f"duplicate TLC state declaration: {key}")
                states[key] = state(ring, match[2])
                if "style = filled" in line:
                    initial.add(key)
    if any(src not in states or not targets <= states.keys() for src, targets in edges.items()):
        raise ValueError("undeclared TLC edge endpoint")
    reachable = set(initial)
    pending = list(initial)
    while pending:
        new = edges[pending.pop()] - reachable
        reachable.update(new)
        pending.extend(new)
    if reachable != states.keys():
        raise ValueError("unreachable TLC state declaration")
    actual_initial = ring.either(states[key] for key in initial)
    if ring.initial != actual_initial or len(set(states.values())) != len(states) or ring.b.false in states.values():
        raise ValueError("state decoding or initial-state mismatch")
    for key, cube in states.items():
        actual = ring.either(states[dest] for dest in edges[key])
        predicted = ring.either(ring.image(cube, action) for action in ring.actions)
        if actual != predicted:
            raise ValueError(f"successors differ at TLC state {key}")
    return {"states": len(states), "initial": len(initial), "distinct_edges": sum(map(len, edges.values()))}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dot", type=Path)
    print(json.dumps(compare(parser.parse_args().dot)))
