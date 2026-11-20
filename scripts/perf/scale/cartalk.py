#!/usr/bin/env python3
"""Evaluate the AllSolutions expression in the pinned five-piece Car Talk model."""
import argparse
from hashlib import sha256
from itertools import combinations_with_replacement, product
import json
from pathlib import Path

INPUTS = {
    "CarTalkPuzzle.tla": "e1210a1dc347966f53233095cb2be5b8d17e237342d9f7fb49fc38a14df10d6a",
    "MC.tla": "d49aa03533a0aee67efbfdbb6f47e3f79683a7751eb7a271ac7e2b239962ef87",
    "MC.cfg": "d488a15255157895ea6c6e620dd95501f511c5747485f48e18922afbd41bd008",
}


def solutions(total, pieces):
    """Generate all positive, nondecreasing breaks admitting every signed weight."""
    def extend(prefix, weight, left):
        if left == 0:
            if weight == total:
                yield prefix
            return
        # Even taking the largest next weight that preserves coverage on every
        # step cannot fill the required interval beyond this upper bound.
        if (2 * weight + 1) * 3**left < 2 * total + 1:
            return
        highest = min(2 * weight + 1, (total - weight) // left)
        for item in range(prefix[-1] if prefix else 1, highest + 1):
            yield from extend((*prefix, item), weight + item, left - 1)

    yield from extend((), 0, pieces)


def check_small():
    """Independent literal signed-sum enumeration on a finite rectangle."""
    for pieces in range(1, 5):
        signs = list(product((-1, 0, 1), repeat=pieces))
        for total in range(1, 17):
            expected = []
            for weights in combinations_with_replacement(range(1, total + 1), pieces):
                if sum(weights) != total:
                    continue
                values = {sum(w * s for w, s in zip(weights, vector)) for vector in signs}
                if all(i in values for i in range(1, total + 1)):
                    expected.append(weights)
            if list(solutions(total, pieces)) != expected:
                raise ValueError(f"disagreement on {total} pounds and {pieces} pieces")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path, help="pinned CarTalkPuzzle.toolbox/Model_3 directory")
    parser.add_argument("--check-small", action="store_true")
    args = parser.parse_args()
    for name, expected in INPUTS.items():
        if sha256((args.model / name).read_bytes()).hexdigest() != expected:
            parser.error(f"input differs from the qualified model: {name}")
    if args.check_small:
        check_small()
    print(json.dumps({"expression": "AllSolutions", "total": 121, "pieces": 5,
                      "solutions": list(solutions(121, 5)), "input_sha256": INPUTS}))


if __name__ == "__main__":
    main()
