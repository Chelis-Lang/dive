"""Analytic counts of reachable states and transition occurrences for the pinned KVS relation."""

import argparse
import json
from fractions import Fraction
from hashlib import sha256
from itertools import combinations_with_replacement, groupby
from math import comb, factorial, prod
from pathlib import Path


def transaction_history_counts(keys, values, tx, symmetry):
    """Independent count using nested missed-key sets ordered by time of opening.

    For one key, an unwritten, unmissed snapshot equals the current store.
    A written snapshot can be any value including NoVal. A missed key can
    also have any unwritten snapshot. When all transactions are open the
    youngest cannot have missed a commit: a committer would have closed.

    Nested missed sets of cardinalities i <= j admit C(K-i, j-i) choices
    for the larger set once the smaller one is fixed. The remaining factors
    depend only on those cardinalities and the lengths of equal-size runs.
    Grouping chains this way keeps the original multiplicity for each key.
    """
    if min(keys, values, tx) < 1:
        raise ValueError("the KVS count needs nonempty key, value and transaction domains")
    distinct = 0
    generated = Fraction(1)  # Init
    alphabet = values + 1
    for opened in range(tx + 1):
        for sizes in combinations_with_replacement(range(keys + 1), opened):
            if opened == tx and sizes[0] != 0:
                continue
            # Equal sizes in a nested chain imply identical masks. To grow a
            # mask from i to j elements choose its j-i new keys among K-i.
            groups = [(size, len(list(group))) for size, group in groupby(sizes)]
            configurations = alphabet**keys
            previous = 0
            for size, _ in groups:
                configurations *= comb(keys - previous, size - previous)
                previous = size
            if not symmetry:
                configurations *= (
                    comb(tx, opened) * factorial(opened) // prod(factorial(n) for _, n in groups)
                )
            closable = Fraction()
            for missed, n in groups:
                kinds = (2 * alphabet) ** missed * (alphabet + 1) ** (keys - missed)
                configurations *= comb(kinds + n - 1, n) if symmetry else kinds**n
                # Among local states with the same mask, each missed key is
                # written half the time. Equal-mask multisets treat all local
                # states symmetrically, so this also gives their mean count.
                closable += Fraction(n, 1 << missed)
            distinct += configurations
            # Open or Rollback once per id, plus writes and conflict-free Close.
            generated += configurations * (tx + opened * keys * values + closable)
    if generated.denominator != 1:
        raise ArithmeticError("nonintegral transition count")
    return distinct, int(generated)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("kvs", type=Path, help="pinned Examples specifications/KeyValueStore directory")
    parser.add_argument("--keys", required=True, type=int)
    parser.add_argument("--values", required=True, type=int)
    parser.add_argument("--transactions", required=True, type=int)
    args = parser.parse_args()
    if min(args.keys, args.values, args.transactions) < 1:
        parser.error("all domains must have at least one member")
    for name, expected in {
        "KeyValueStore.tla": "0306e7659212ef214fdb40012f1f24a78b25669292a80127c815da5eec466b07",
        "MCKVS.tla": "4d66a40abeb29558b473770b736dc1bd16c841d9ac847b4644b36cd47d8c8962",
    }.items():
        if sha256((args.kvs / name).read_bytes()).hexdigest() != expected:
            parser.error(f"unexpected source for {name}")
    counts = {}
    for mode, symmetry in (("transaction_orbits", True), ("labeled", False)):
        distinct, generated = transaction_history_counts(args.keys, args.values, args.transactions, symmetry)
        counts[mode] = {"distinct_decimal": str(distinct), "generated_decimal": str(generated)}
    print(json.dumps(counts))


if __name__ == "__main__":
    main()
