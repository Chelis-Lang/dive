#!/usr/bin/env python3
"""Exact source-pinned EWD998 benchmark with finite binary decision diagrams."""

import argparse
import hashlib
import json
import operator
import sys
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path

from dd import cudd

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "symbolic"))
from finite_relation import FiniteRelation

SOURCES = {
    "EWD998.cfg": "4e47f078297118a6e31953eddf6a8a6d469d32464ec1d22b902c041c587e5d59",
    "EWD998.tla": "b51ba3587e0d1b88a38cc15cd667b7a50ccb72e7f5f79f7260c97e473f9392e8",
    "AsyncTerminationDetection.tla": "c1b15be68a73c6e5f69c5c09f2736870182a9b0765ad9860452240119911b030",
    "Folds.tla": "2f8c0a7de9a64a5276a78cb7b048e18038d37787439bb9e60df126dc7608a35a",
}
FUNCTIONS = "b54ff63b7c76c327525c17c188d5f9f5e53d92f3fd701f5e2ba54f0f54391063"


def admit(source, modules):
    for name, expected in SOURCES.items():
        if hashlib.sha256((source / name).read_bytes()).hexdigest() != expected:
            raise ValueError(f"source differs from the admitted model: {name}")
    local = source / "Functions.tla"
    resolved = local if local.exists() else modules / "Functions.tla"
    if hashlib.sha256(resolved.read_bytes()).hexdigest() != FUNCTIONS:
        raise ValueError("the bound Functions module differs")
    for directory in (source, modules):
        for name in ("Naturals", "Integers", "FiniteSets", "Sequences"):
            if (directory / f"{name}.tla").exists():
                raise ValueError(f"shadowed standard module: {name}")


@dataclass
class Action:
    name: str
    witness: object
    relation: object
    to_next: dict
    to_current: dict
    updates: dict


class Ring(FiniteRelation):
    def __init__(self, n):
        self.b = cudd.BDD(memory_estimate=2**30)
        self.b.configure(reordering=False)
        self.n = n
        self.nodes = range(n)
        self.bounds = {"pos": (0, n - 1), "black": (0, 1), "q": (-3 * (n - 1)**2, min(9, 3 * (n - 1)))}
        self.active = self.vector("active", 0, 1)
        self.color = self.vector("color", 0, 1)
        self.counter = self.vector("counter", -3 * (n - 1), 3)
        self.pending = self.vector("pending", 0, 3)
        self.bits = {}
        self.variables = []
        for key in ["pos", "black", "q", *self.color, *self.active, *self.counter, *self.pending]:
            lo, hi = self.bounds[key]
            names = [f"{key}_{i}" for i in range(max(1, (hi - lo).bit_length()))]
            self.bits[key] = names
            self.variables.extend(names)
            for name in names:
                self.b.declare(name, name + "_next")
        self.values = {}
        for key, (lo, hi) in self.bounds.items():
            self.values[key] = {v: self.literal(key, v) for v in range(lo, hi + 1)}
        valid = self.conjunction(self.either(values.values()) for values in self.values.values())
        self.initial = valid & self.eq("q", 0) & self.eq("black", 1) & self.conjunction(
            self.eq(k, 0) for k in self.counter + self.pending
        )
        self.terminated, self.detected = self.termination_predicates(self.values)
        self.actions = list(self.transitions())

    def vector(self, prefix, lo, hi):
        keys = [f"{prefix}{i}" for i in self.nodes]
        self.bounds.update((key, (lo, hi)) for key in keys)
        return keys

    def either(self, predicates):
        result = self.b.false
        for predicate in predicates:
            result |= predicate
        return result

    def literal(self, key, value, successor=False):
        lo, hi = self.bounds[key]
        if not lo <= value <= hi:
            return self.b.false
        names = self.bits[key]
        return self.encoding([self.b.var(k + "_next" if successor else k) for k in names], value - lo)

    def eq(self, key, value):
        return self.values[key].get(value, self.b.false)

    def constant(self, value):
        return {value: self.b.true}

    def assignment_of(self, key, terms):
        return self.either(condition & self.literal(key, value, True) for value, condition in terms.items())

    def shift(self, key, delta):
        return {value + delta: clause for value, clause in self.values[key].items()}

    def sum_of(self, keys, observations=None):
        observations = self.values if observations is None else observations
        totals = {0: self.b.true}
        for key in keys:
            new = defaultdict(lambda: self.b.false)
            for total, prior in totals.items():
                for value, clause in observations[key].items():
                    new[total + value] |= prior & clause
            totals = new
        return totals

    def compare(self, left, right, predicate):
        return self.either(x & y for i, x in left.items() for j, y in right.items() if predicate(i, j))

    def action(self, name, witness, assignments):
        changed = [bit for key in assignments for bit in self.bits[key]]
        rename = {bit: bit + "_next" for bit in changed}
        relations = (self.assignment_of(k, terms) for k, terms in assignments.items())
        return Action(name, witness, witness & self.conjunction(relations), rename, {v: k for k, v in rename.items()}, assignments)

    def transitions(self):
        # The three satisfied alternatives each generate a TLC witness, even
        # when they take the very same initial-probe transition.
        initiates = [
            self.eq("black", 1),
            self.eq(self.color[0], 1),
            self.either(c for x, c in self.sum_of([self.counter[0], "q"]).items() if x > 0),
        ]
        assignments = {"pos": self.constant(self.n - 1), "q": self.constant(0),
                       "black": self.constant(0), self.color[0]: self.constant(0)}
        for clause in initiates:
            yield self.action("initiate", self.eq("pos", 0) & clause, assignments)
        for i in self.nodes:
            active, color, counter, pending = self.active[i], self.color[i], self.counter[i], self.pending[i]
            if i:
                black = self.eq("black", 1) | self.eq(color, 1)
                yield self.action("pass", self.eq("pos", i) & self.eq(active, 0), {
                    "pos": self.constant(i - 1),
                    "q": self.sum_of(["q", counter]),
                    "black": {0: ~black, 1: black}, color: self.constant(0),
                })
            for j in self.nodes:
                if i != j:
                    receiver = self.pending[j]
                    yield self.action("send", self.eq(active, 1), {counter: self.shift(counter, 1), receiver: self.shift(receiver, 1)})
            yield self.action("receive", ~self.eq(pending, 0), {
                counter: self.shift(counter, -1), pending: self.shift(pending, -1),
                active: self.constant(1), color: self.constant(1),
            })
            yield self.action("deactivate", self.eq(active, 1), {active: self.constant(0)})

    def termination_predicates(self, observations):
        def match(key, value):
            return observations[key].get(value, self.b.false)

        terminated = self.conjunction(match(k, 0) for k in self.active + self.pending)
        detected = match("pos", 0) & match("black", 0) & match(self.color[0], 0) & match(self.active[0], 0)
        detected &= self.sum_of(["q", self.counter[0]], observations).get(0, self.b.false)
        return terminated, detected

    def invariant(self, observations):
        totals = self.sum_of(self.counter, observations)
        equal_total = self.compare(totals, self.sum_of(self.pending, observations), operator.eq)
        terminated, detected = self.termination_predicates(observations)
        clauses = [self.either(clause for v, clause in observations[k].items() if v >= 0) for k in self.pending]
        for i in self.nodes:
            later = range(i + 1, self.n)
            passive = self.conjunction(observations[self.active[j]].get(0, self.b.false) for j in later)
            counted = self.compare(observations["q"], self.sum_of(self.counter[i + 1:], observations), operator.eq)
            positive = self.either(c for s, c in self.sum_of(["q", *self.counter[:i + 1]], observations).items() if s > 0)
            black = self.either(observations[k].get(1, self.b.false) for k in ["black", *self.color[:i + 1]])
            clauses.append(~observations["pos"].get(i, self.b.false) | (passive & counted) | positive | black)
        return equal_total & self.conjunction(clauses) & (~detected | terminated)

    def boundary(self, reached):
        # TLC checks state invariants even on generated destinations that its
        # state constraint excludes from the queue. Keep mathematical outputs
        # of every assignment before clipping to the finite search encoding.
        occurrences = 0
        for a in self.actions:
            escaped = self.either(cond for k, terms in a.updates.items() for v, cond in terms.items()
                                  if v not in self.values[k])
            sources = reached & a.witness & escaped
            if sources == self.b.false:
                continue
            post = self.values | a.updates
            violates_constraint = self.either(cond for k in [*self.counter, *self.pending, "q"]
                                              for v, cond in post[k].items() if v > (9 if k == "q" else 3))
            if sources & ~violates_constraint != self.b.false:
                raise ValueError(f"an admitted {a.name} successor escaped the encoding")
            if sources & ~self.invariant(post) != self.b.false:
                raise ValueError(f"state invariant outside the constraint: {a.name}")
            occurrences += self.state_count(sources)
        return occurrences

    def image(self, states, action):
        result = cudd.and_exists(states, action.relation, list(action.to_next))
        return self.b.let(action.to_current, result)

    def preimage(self, states, action):
        next_states = self.b.let(action.to_next, states)
        return cudd.and_exists(next_states, action.relation, list(action.to_current))

    def temporal(self, reached):
        # Check TD!Init and every admitted concrete action against TD!Next or
        # stuttering on the three projected variables. For send/receive and
        # deactivate, active/pending already have their abstract updates.
        if self.initial & self.detected & ~self.terminated != self.b.false:
            raise ValueError("abstract Init")
        for action in self.actions:
            terminated_after, detected_after = self.termination_predicates(self.values | action.updates)
            allowed = self.detected.equiv(detected_after)
            if action.name == "deactivate":
                allowed |= terminated_after.equiv(detected_after)
            elif action.name in ("initiate", "pass"):
                allowed |= self.terminated & detected_after
            if reached & action.witness & ~allowed != self.b.false:
                raise ValueError(f"abstract Next: {action.name}")

        # Once all nodes terminate, every environment action is disabled. The
        # token position selects the unique nonstuttering System destination.
        # Weak fairness forces successive steps down the computed distance.
        universe = reached & self.terminated
        detected = universe & self.detected
        wins = detected
        steps = [a for a in self.actions if a.name in ("initiate", "pass")]
        rank = 0
        while True:
            previous = universe & self.either(self.preimage(wins, a) for a in steps) & ~wins
            if previous == self.b.false:
                break
            wins |= previous
            rank += 1
        if universe != wins:
            raise ValueError("unranked terminated state")
        return {"terminated": self.state_count(universe), "detected": self.state_count(detected), "max_system_steps": rank}

    def check(self):
        frontier = reached = self.initial
        depth = 1
        while True:
            source = min((frontier, reached), key=lambda root: root.dag_size)
            new = self.either(self.image(source, a) for a in self.actions) & ~reached
            if new == self.b.false:
                break
            frontier = new
            reached |= new
            depth += 1
        if reached & ~self.invariant(self.values) != self.b.false:
            raise ValueError("state invariant")
        initial = self.state_count(self.initial)
        generated = initial + sum(self.state_count(reached & action.witness) for action in self.actions)
        return {"nodes": self.n, "initial": initial, "distinct": self.state_count(reached), "generated": generated,
                "depth": depth, "left_on_queue": 0, "constraint_excluded_witnesses": self.boundary(reached), "final_bdd_nodes": reached.dag_size, **self.temporal(reached)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--modules", required=True, type=Path, help="pinned CommunityModules directory")
    parser.add_argument("--nodes", choices=(2, 3, 4, 5), type=int, default=4, help="change only ring size")
    options = parser.parse_args()
    admit(options.source, options.modules)
    print(json.dumps(Ring(options.nodes).check()))


if __name__ == "__main__":
    main()
