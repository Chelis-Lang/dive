"""Differential checks for product reachability, path lengths and generated multiplicity."""

from collections import deque
from itertools import product
from random import Random
import unittest

from phase_product import explore


def explicit(n, local, global_edges):
    start = (0, (0,) * n)
    queue = deque([(start, 1)])
    seen = {start}
    generated, deadlocked, depth = 1, 0, 0
    while queue:
        (phase, vector), depth = queue.popleft()
        successors = [(phase, vector[:i] + (target,) + vector[i+1:])
                      for i, value in enumerate(vector) for target in local(phase, value)]
        successors.extend((child, vector) for child, guard in global_edges(phase) if all(map(guard, vector)))
        generated += len(successors)
        deadlocked += not successors
        for state in successors:
            if state not in seen:
                seen.add(state)
                queue.append((state, depth + 1))
    return len(seen), generated, depth, deadlocked


class ProductTests(unittest.TestCase):
    def test_against_labeled_bfs(self):
        rng = Random(775)
        for _ in range(40):
            moves = {(phase, value): [rng.randrange(4) for _ in range(rng.randrange(4))]
                     for phase, value in product(range(5), range(4))}
            edges = {phase: [(child, frozenset(v for v in range(4) if rng.randrange(2)).__contains__)
                             for child in (phase, 2*phase+1, 2*phase+2) if child < 5]
                     for phase in range(5)}

            def local(phase, value):
                return moves[phase, value]

            for n in range(1, 5):
                result = explore(n, 0, 0, local, edges.__getitem__)
                self.assertEqual(tuple(result[k] for k in ("distinct", "generated", "depth", "deadlocked")),
                                 explicit(n, local, edges.__getitem__))

    def test_overlapping_phase_histories_refuse(self):
        for graph in ({0: [1], 1: [0]}, {0: [1, 2], 1: [3], 2: [3], 3: []}, {0: [1, 1], 1: []}):
            def edges(p):
                return [(q, lambda _: True) for q in graph[p]]
            with self.assertRaisesRegex(ValueError, "multiple incoming edges or cycle"):
                explore(2, 0, 0, lambda p, v: (), edges)


if __name__ == "__main__":
    unittest.main()
