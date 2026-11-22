#!/usr/bin/env python3
"""Compare a fresh lockserv predicate's Boolean model count with the protocol's token count."""

import argparse
import json
from pathlib import Path


def count(path, bits):
    doc = json.loads(path.read_text())
    if doc["format"] != "finite-state-predicate-v1":
        raise ValueError("unexpected predicate")
    # Entries are child-first. Complemented edges count the other assignments
    # at that bit. Coordinates skipped between parent and child are free.
    nodes = [(bits, 0), (bits, 1)]

    def below(edge, start):
        if type(edge) is not int or not 0 < abs(edge) < len(nodes):
            raise ValueError("invalid predicate edge")
        bit, n = nodes[abs(edge)]
        if bit < start:
            raise ValueError("predicate does not have the original coordinate order")
        if edge < 0:
            n = (1 << (bits - bit)) - n
        return n << (bit - start)

    for bit, low, high in doc["nodes"]:
        if type(bit) is not int or not 0 <= bit < bits:
            raise ValueError("unexpected coordinate")
        nodes.append((bit, below(low, bit + 1) + below(high, bit + 1)))
    return below(doc["root"], 0)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("clients", type=int)
    parser.add_argument("predicate", type=Path)
    args = parser.parse_args()
    if not 1 <= args.clients <= 127:
        parser.error("expected 1 to 127 clients in the finite layout")
    total = count(args.predicate, 4 * args.clients + 1)
    expected = (3 * args.clients + 1) << args.clients
    if total != expected:
        parser.exit(1, f"predicate has {total} assignments, expected {expected}\n")
    print(json.dumps({"clients": args.clients, "predicate_assignments_decimal": str(total)}))
