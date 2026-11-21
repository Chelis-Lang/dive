#!/usr/bin/env python3
"""Check the append-only DAG obligations for the pinned TLCSailfish2 source."""

import argparse
import hashlib
import itertools
import json
from functools import cache
from pathlib import Path

import z3

PINS = {
    "TLCSailfish2.cfg": "c2e2517d52b97e7dba6bf2296c70bfc4116c1cbfc88f68f48969f8c109236d0f",
    "TLCSailfish2.tla": "8263711a83ee9afe4a26ead6219ab7235775b084c9fea87e3934b34f24f4bb86",
    "Utils.tla": "b38f43403c4d5cfd4b733613935e9ac92149c26d055d44b77f0b9c81c6644a1e",
    "Digraph.tla": "d89e6b7ffb1f25e288bdfef9b80c2259493ae5381929a10b8bb4a949b0db6be5",
    "BlockDag.tla": "f80956f2d02bd547ce310eaf693c434daf6785166e863859276cce0377051d62",
    "Sailfish.tla": "74e165a98af083509b5dc6b86e526588721fadd572f7c6b47e5474ce509bc401",
}
NODES = range(4)
CORRECT = range(1, 4)
GST = 6


def any_of(*clauses):
    return z3.Or(*clauses) if clauses else z3.BoolVal(False)


def all_of(*clauses):
    return z3.And(*clauses) if clauses else z3.BoolVal(True)


def leader(round_number):
    return (round_number - 1) % 4


def quorum(predicates):
    """At least three of the four nodes. Emit portable Boolean SMT-LIB."""
    return any_of(*(all_of(*triple) for triple in itertools.combinations(predicates, 3)))


class Dag:
    """A superset of the reachable immutable DAGs at this round bound."""

    def __init__(self, last):
        self.last = last
        self.rounds = range(1, last + 1)
        self.commits = range(1, last - 1)
        self.present = {(r, i): z3.Bool(f"v_{r}_{i}") for r in self.rounds for i in NODES}
        self.edges = {
            (r, i, j): z3.Bool(f"e_{r}_{i}_{j}")
            for r in range(2, last + 1) for i in NODES for j in NODES
        }

    def edge(self, r, i, j):
        # At round one, the source always attaches to genesis.
        return self.present[r, i] if r == 1 else self.edges[r, i, j]

    def correct_parents(self, r, i, parents):
        """The two voting guards, plus timely delivery after GST."""
        previous = leader(r - 1)
        older = leader(r - 2)
        votes = [all_of(parents[j], z3.Not(self.edge(r - 1, j, older))) for j in NODES]
        guards = [z3.Implies(parents[previous], any_of(self.edge(r - 1, previous, older), quorum(votes)))]
        if r >= GST:
            guards.extend(parents[j] for j in CORRECT)
        if i == leader(r):
            # The current leader counts itself. Other non-votes that existed
            # at insertion still exist in the final DAG with the same edges.
            no_vote = [True if j == i else all_of(self.present[r, j], z3.Not(self.edge(r, j, previous))) for j in NODES]
            guards.append(any_of(parents[previous], quorum(no_vote)))
        return all_of(*guards)

    def admissible(self):
        for (r, i, j), edge in self.edges.items():
            yield z3.Implies(edge, all_of(self.present[r, i], self.present[r - 1, j]))
        for r in range(2, self.last + 1):
            for i in NODES:
                parents = [self.edge(r, i, j) for j in NODES]
                yield z3.Implies(self.present[r, i], quorum(parents))
                if i in CORRECT:
                    yield z3.Implies(self.present[r, i], all_of(self.present[1, i], self.correct_parents(r, i, parents)))

    @cache
    def ancestor(self, r, i, earlier):
        """Does this vertex descend from the leader of an earlier or equal round?"""
        if r == earlier:
            return all_of(self.present[r, i], i == leader(r))
        return any_of(*(all_of(self.edge(r, i, j), self.ancestor(r - 1, j, earlier)) for j in NODES))

    @cache
    def chain(self, later, earlier):
        """Iterate the source's greatest earlier leader in the ancestry."""
        if later == earlier:
            return z3.BoolVal(True)
        paths = []
        for k in range(earlier, later):
            greater = [self.ancestor(later, leader(later), t) for t in range(k + 1, later)]
            paths.append(all_of(self.ancestor(later, leader(later), k), z3.Not(any_of(*greater)), self.chain(k, earlier)))
        return any_of(*paths)

    @cache
    def own_commit(self, r, i):
        votes = [all_of(self.edge(r + 2, i, j), self.edge(r + 1, j, leader(r))) for j in NODES]
        return all_of(self.present[r + 2, i], quorum(votes))

    def steps(self):
        """All individual legal insertions in the current DAG, including delivery choices."""
        for i in NODES:
            yield (1, i, ()), z3.Not(self.present[1, i])
        for r in range(2, self.last + 1):
            for i in NODES:
                ready = z3.Not(self.present[r, i])
                if i in CORRECT:
                    # round[i] is zero before the first insertion and otherwise
                    # the greatest present round of i.
                    ready = all_of(self.present[1, i], *(z3.Not(self.present[k, i]) for k in range(r, self.last + 1)))
                for count in (3, 4):
                    for choice in itertools.combinations(NODES, count):
                        guards = [ready, *(self.present[r - 1, j] for j in choice)]
                        if i in CORRECT:
                            guards.append(self.correct_parents(r, i, [z3.BoolVal(j in choice) for j in NODES]))
                        yield (r, i, choice), all_of(*guards)

    def obligations(self):
        committed = {r: any_of(*(self.own_commit(r, i) for i in CORRECT)) for r in self.commits}
        violations = [all_of(committed[s], committed[r], z3.Not(self.chain(r, s))) for s, r in itertools.combinations(self.commits, 2)]
        enabled = any_of(*(guard for _, guard in self.steps()))
        done = all_of(*(self.present[self.last, i] for i in CORRECT))
        yield "incompatible", any_of(*violations), z3.unsat
        yield "deadlock", all_of(z3.Not(done), z3.Not(enabled)), z3.unsat
        yield "all_commits", all_of(*committed.values()), z3.sat
        yield "terminal_without_terminate", all_of(done, z3.Not(enabled)), z3.sat
        late, missing = [], []
        for s in self.commits:
            if s < GST or leader(s) not in CORRECT:
                continue
            for i in CORRECT:
                advanced = any_of(*(self.present[r, i] for r in range(s + 2, self.last + 1)))
                has_leader = []
                for t in range(s, self.last - 1):
                    newer = [self.own_commit(u, i) for u in range(t + 1, self.last - 1)]
                    has_leader.append(all_of(self.own_commit(t, i), self.ancestor(t, leader(t), s), z3.Not(any_of(*newer))))
                late.append(advanced)
                missing.append(all_of(advanced, z3.Not(any_of(*has_leader))))
        if late:
            yield "missing_stable_leader", any_of(*missing), z3.unsat
            yield "stable_leader_due", any_of(*late), z3.sat


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="directory of the original modules and CFG")
    parser.add_argument("--rounds", type=int, default=5, help="change only the round bound; the original is 5")
    parser.add_argument("--out", type=Path, help="directory for standalone SMT-LIB queries")
    args = parser.parse_args()
    if not 1 <= args.rounds <= 16:
        parser.error("supported round bounds are 1 through 16")
    for name, expected in PINS.items():
        if hashlib.sha256((args.source / name).read_bytes()).hexdigest() != expected:
            parser.error(f"source differs from the admitted model: {name}")
    if args.out:
        args.out.mkdir(parents=True, exist_ok=True)
    dag = Dag(args.rounds)
    solver = z3.Solver()
    solver.set(timeout=120_000)
    solver.add(*dag.admissible())
    passed = True
    for name, clause, expected in dag.obligations():
        solver.push()
        solver.add(clause)
        if args.out:
            (args.out / f"{name}.smt2").write_text("(set-logic QF_UF)\n" + solver.to_smt2())
        result = solver.check()
        print(json.dumps({"rounds": args.rounds, "query": name, "result": str(result), "expected": str(expected)}), flush=True)
        passed = passed and result == expected
        solver.pop()
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
