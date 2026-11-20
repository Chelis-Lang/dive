#!/usr/bin/env python3
"""Exact receipt-set closure for the all-DONE region of the pinned c1cs model."""

import argparse
from collections import Counter
from copy import deepcopy
from hashlib import sha256
from itertools import product
import json
from math import comb
from pathlib import Path
import subprocess

PINS = {
    "c1cs.tla": "ee222e8a5a7defc2d449ec30ee24053036560b6b7f1107ab2ab115427fb53af0",
    "c1cs.cfg": "0ef50a8cc706e60f6285efe627f2b34ff9a52d8dfdf9287bed2b358157c4d233",
}


def symbolic_layers(free):
    """Cardinality of each BFS layer and its outgoing deliveries."""
    return [(comb(free, k), comb(free, k) * (free - k)) for k in range(free + 1)]


def explicit_layers(bits):
    frontier = {0}
    layers = []
    while frontier:
        successors = set()
        edges = 0
        for mask in frontier:
            for bit in range(bits):
                if mask & (1 << bit) == 0:
                    successors.add(mask | (1 << bit))
                    edges += 1
        layers.append((len(frontier), edges))
        frontier = successors
    return layers


def path(v, reverse):
    common = max(set(v), key=v.count)
    quorum = [i for i, x in enumerate(v) if x == common][:3]
    leader = quorum[0]
    pc = ["PROPOSE"] * 4
    decisions = [{"model": "Bottom"}] * 4
    broadcast = []
    received = [[] for _ in pc]
    states = []

    def emit():
        row = dict(
            pc={"seq": pc},
            v={"seq": [{"model": x} for x in v]},
            dValue={"seq": decisions},
            bcastMsg={"set": broadcast},
            rcvdMsg={"seq": [{"set": r} for r in received]},
        )
        states.append({"values": deepcopy(row)})

    def send(i, kind, value):
        broadcast.append({"record": {"sndr": i + 1, "type": kind, "value": {"model": value}}})

    def decide(i):
        pc[i] = "DONE"
        decisions[i] = {"model": common}
        send(i, "Decision", common)
        emit()

    emit()
    for i, value in enumerate(v):
        pc[i] = "DECIDE"
        send(i, "Proposed", value)
        emit()
    for i in quorum:
        received[leader].append(broadcast[i])
        emit()
    decide(leader)
    for i in range(4):
        if i != leader:
            received[i].append(broadcast[4])
            emit()
            decide(i)

    # Receive guards only test membership, nonmembership and a non-crashed pc.
    # Every unset bit is independently addable, and no step here clears a bit.
    free = [(i, msg) for i in range(4) for msg in broadcast if msg not in received[i]]
    for i, msg in reversed(free) if reverse else free:
        received[i].append(msg)
        emit()
    return len(free), dict(
        format="dive-trace",
        version=1,
        variables=list(states[0]["values"]),
        states=states,
        ending={"kind": "behavior"},
    )


def validate(dive, folder, trace):
    proc = subprocess.run(
        [
            dive,
            "validate-trace",
            folder / "c1cs.tla",
            "--trace",
            "/dev/stdin",
            "--config",
            folder / "c1cs.cfg",
            "--compat=tlc",
            "--seed",
            "0",
            "--fp",
            "0",
            "--output",
            "json",
        ],
        input=json.dumps(trace),
        text=True,
        capture_output=True,
    )
    return json.loads(proc.stdout)["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("model_dir", type=Path)
    args = parser.parse_args()
    for name, digest in PINS.items():
        if sha256((args.model_dir / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"unexpected {name} revision")
    for n in range(13):
        if symbolic_layers(n) != explicit_layers(n):
            raise ValueError(f"layer mismatch for {n} free receipts")
    counts = Counter()
    for v in product(("v1", "v2"), repeat=4):
        if v.count("v1") == 2:
            continue
        for reverse in (False, True):
            free, trace = path(v, reverse)
            if validate(args.dive.resolve(), args.model_dir, trace) != "valid":
                raise ValueError("independent validator rejected reachable path")
            counts["validated_paths"] += 1
        layers = symbolic_layers(free)
        counts["families"] += 1
        counts["states"] += sum(n for n, _ in layers)
        counts["deliveries"] += sum(e for _, e in layers)
    del trace["states"][-2]
    if validate(args.dive.resolve(), args.model_dir, trace) != "invalid":
        raise ValueError("independent validator accepted two simultaneous receipts")
    counts["enumeration_witnesses"] = counts["deliveries"] * 4
    print(json.dumps(dict(region="all-DONE receipt families", **counts), sort_keys=True))


if __name__ == "__main__":
    main()
