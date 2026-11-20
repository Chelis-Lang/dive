#!/usr/bin/env python3
"""Finite relational model of the three pinned KVS configs. Requires dd==0.6.0 (CUDD)."""

import argparse
import hashlib
import json
from fractions import Fraction
from itertools import combinations_with_replacement, groupby
from math import comb, factorial, prod
from pathlib import Path
from dd import cudd
from finite_relation import FiniteRelation, equal

CONFIGS = {
    "Small": (2, 2, 3, False, "2360ddff6cac82d84a7077e35cd5e77e7acf81a5d37fc37bc10f335082854103"),
    "Medium": (3, 3, 2, True, "d3272277738dce3a0f0c16480e96870a1630c9ea1fb7e64fdca0f0763fc7a629"),
    "Large": (3, 2, 3, True, "c1f414a1caf84df31089ecb569de0dd6b835fad640f7545b2b7b239b7aad7964"),
}


def check_pin(path, expected):
    if hashlib.sha256(path.read_bytes()).hexdigest() != expected:
        raise ValueError(f"unexpected source for {path.name}")


class Model(FiniteRelation):
    def __init__(self, keys, values, tx):
        self.b = b = cudd.BDD(memory_estimate=512 << 20)
        b.configure(reordering=False)
        self.variables = []
        self.roles = [[] for _ in range(tx)]

        def field(name, role=None):
            b.declare(name, name + "_next")
            self.variables.append(name)
            if role is not None:
                self.roles[role].append(name)
            return b.var(name)

        active = [field(f"active{t}", t) for t in range(tx)]
        groups = []
        self.type_ok = self.lifecycle = b.true
        self.actions = []
        for k in range(keys):
            store = [field(f"s{k}_{i}") for i in range(values.bit_length())]
            local = []
            for t in range(tx):
                snap = [field(f"x{k}_{t}_{i}", t) for i in range(len(store))]
                w = field(f"w{k}_{t}", t)
                m = field(f"m{k}_{t}", t)
                local.append((snap, w, m))
                same = self.conjunction(equal(a, c) for a, c in zip(snap, store))
                self.lifecycle &= active[t] | (self.encoding(snap, 0) & ~w & ~m)
                self.lifecycle &= ~active[t] | same | w | m
                self.type_ok &= self.domain(snap, values)
            groups.append((store, local))
            self.type_ok &= self.domain(store, values)
        for t in range(tx):
            opened, rollback, close = [], [], []
            clean = active[t]
            for store, local in groups:
                snap, w, m = local[t]
                clean &= ~(w & m)
                opened.append(self.assignment(dict(zip(snap, store))))
                reset = {x: b.false for x in [*snap, w, m]}
                rollback.append(self.assignment(reset))
                commit = dict(reset)
                commit.update({s: b.ite(w, x, s) for s, x in zip(store, snap)})
                for other, (_, _, missed) in enumerate(local):
                    if other != t:
                        commit[missed] = b.ite(active[other], missed | w, b.false)
                close.append(self.assignment(commit))
                # Exactly one enabled write per new value different from the snapshot.
                # Zero denotes NoVal (Remove), the other targets are Add/Update.
                for v in range(values + 1):
                    updates = {x: b.true if v & (1 << i) else b.false for i, x in enumerate(snap)}
                    updates[w] = b.true
                    self.actions.append((active[t] & ~self.encoding(snap, v), [self.assignment(updates)]))
            on, off = (self.assignment({active[t]: v}) for v in (b.true, b.false))
            self.actions.extend(
                [(~active[t], opened + [on]), (active[t], rollback + [off]), (clean, close + [off])]
            )
        self.initial = self.conjunction(~b.var(x) for x in self.variables)

    def explore(self):
        reached = frontier = self.initial
        depth = 1
        while True:
            if frontier & ~(self.type_ok & self.lifecycle) != self.b.false:
                raise ValueError("reachable invariant violation")
            successors = self.b.false
            enabled = self.b.false
            for guard, updates in self.actions:
                enabled |= guard
                image = frontier & guard
                for relation, old, rename in updates:
                    image = self.b.let(rename, cudd.and_exists(image, relation, old))
                successors |= image
            if frontier & ~enabled != self.b.false:
                raise ValueError("reachable deadlock")
            frontier = successors & ~reached
            if frontier == self.b.false:
                return reached, depth
            depth += 1
            reached |= frontier


def transaction_history_counts(keys, values, tx, symmetry):
    """Independent count using nested missed-key sets ordered by time of opening.

    For one key, an unwritten, unmissed snapshot equals the current store.
    A written snapshot can be any value including NoVal. A missed key can
    also have any unwritten snapshot. When all transactions are open the
    youngest cannot have missed a commit: a committer would have closed.
    """
    distinct = 0
    generated = Fraction(1)  # Init
    alphabet = values + 1
    for opened in range(tx + 1):
        for chain in combinations_with_replacement(range(1 << keys), opened):
            if opened == tx and chain[0] != 0:
                continue
            if any(a & b != a for a, b in zip(chain, chain[1:])):
                continue
            groups = [(mask.bit_count(), len(list(group))) for mask, group in groupby(chain)]
            configurations = alphabet**keys  # contents of the global store
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
    parser.add_argument("model_dir", type=Path)
    parser.add_argument("size", choices=CONFIGS)
    args = parser.parse_args()
    keys, values, tx, symmetry, cfg = CONFIGS[args.size]
    check_pin(
        args.model_dir / "MCKVS.tla", "4d66a40abeb29558b473770b736dc1bd16c841d9ac847b4644b36cd47d8c8962"
    )
    check_pin(
        args.model_dir / "KeyValueStore.tla",
        "0306e7659212ef214fdb40012f1f24a78b25669292a80127c815da5eec466b07",
    )
    check_pin(args.model_dir / f"MCKVSSafety{args.size}.cfg", cfg)
    model = Model(keys, values, tx)
    reached, depth = model.explore()
    distinct, generated = model.counts(reached, symmetry)
    if (distinct, generated) != transaction_history_counts(keys, values, tx, symmetry):
        raise ArithmeticError("symbolic and transaction-history counts disagree")
    if args.size != "Large":
        golden_file = (
            Path(__file__).resolve().parents[2]
            / "conformance/golden/tlc-2026.10.06.014338"
            / f"examples/specifications/KeyValueStore/MCKVSSafety{args.size}.cfg.json"
        )
        result = json.loads(golden_file.read_text())["result"]
        counts = result["counts"]
        if (result["verdict"], counts["distinct_states"], counts["states_generated"], counts["depth"]) != (
            "success",
            distinct,
            generated,
            depth,
        ):
            raise ArithmeticError("pinned oracle disagreement")
    print(
        json.dumps(
            dict(
                model=args.size,
                distinct=distinct,
                generated=generated,
                depth=depth,
                reachable_bdd_nodes=reached.dag_size,
                invariants=["TypeInvariant", "TxLifecycle"],
                backend="hand-translated KVS transition relation / CUDD",
            )
        )
    )


if __name__ == "__main__":
    main()
