#!/usr/bin/env python3
"""Prove local scheduling bounds for the released failure-detector configuration."""

import argparse
import hashlib
import itertools
import json
from pathlib import Path

PINS = {
    "EnvironmentController.cfg": "1cb9f21665fbe58dcc90cf0d05d99f69e8c87a01258d58c5cb952168ff2a3388",
    "EnvironmentController.tla": "27d4468fd8dae883b066b2e9aba0b3bae1402260e373362ed7b79935c0480372",
    "EPFailureDetector.tla": "34407972eef86a8353374086558ede7a1ae611794ff399ca528112b431414b25",
    "Age_Channel.tla": "2235e13cbf2e930caeda1ae3a592c1f6e3c166a9903bddb8bef4e931cb413d47",
}


def action(clock):
    if clock % 2 == 0 and clock % 3 != 0:
        return "send"
    if clock % 3 == 0 and clock % 2 != 0:
        return "predict"
    return "receive"


def advance(clock):
    # 0..3 remain exact. Values >=4 retain their residue modulo six.
    # Allow every reset past 3, including those forbidden by a large delta.
    inc = 4 if clock == 9 else clock + 1
    return (inc, 0) if clock >= 4 else (inc,)


def longest(graph):
    """Maximum number of edges before the obligation, from any initial node."""
    unresolved = set(graph)
    distance = {}
    while unresolved:
        ready = {state for state in unresolved if all(n in distance for n in graph[state])}
        if not ready:
            raise ValueError("unbounded obligation: the avoidance graph has a cycle")
        for state in ready:
            distance[state] = max((1 + distance[n] for n in graph[state]), default=0)
        unresolved -= ready
    return max(distance.values())


def silent_epochs(delivery_guard=True):
    # Bound a suffix after the last crash, from an arbitrary complete epoch.
    # Age is that of the oldest waiting box for this directed live pair,
    # saturated at 2. A healthy receiver cannot skip this box when it is old.
    graph = {}
    for sender, receiver, age in itertools.product(range(10), range(10), (-1, 0, 1, 2)):
        required = delivery_guard and age == 2 and action(receiver) == "receive"
        if required:
            successors = ()
        else:
            has_box = age >= 0 or action(sender) == "send"
            after_tick = min(2, max(0, age) + 1) if has_box else -1
            successors = [(s, r, after_tick) for s, r in itertools.product(advance(sender), advance(receiver))]
        graph[sender, receiver, age] = successors
    return longest(graph)


def bounds():
    gaps = {a: longest({c: () if action(c) == a else advance(c) for c in range(10)})
            for a in ("send", "predict", "receive")}
    return {"clock_classes": 10, "maximum_consecutive_local_steps_without_action": gaps,
            "directed_pair_monitor_states": 400, "maximum_consecutive_complete_epochs_without_hearing": silent_epochs()}


if __name__ == "__main__":
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("source", type=Path)
    source = cli.parse_args().source
    for name, expected in PINS.items():
        if hashlib.sha256((source / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"source differs: {name}")
    for name in ("Naturals", "Integers", "FiniteSets", "Sequences", "TLC", "TLCExt", "_TLCTrace"):
        if (source / f"{name}.tla").exists():
            raise ValueError(f"shadowed standard module: {name}")
    print(json.dumps(bounds()))
