"""Exact labeled BFS statistics for a tree of homogeneous participant products."""

from collections import Counter
from heapq import heapify, heappop, heappush
from itertools import combinations, count


def local_closure(entry, moves):
    """Minimize the whole path to each local value, including work before phase entry."""
    distance = dict(entry)
    serial = count()
    queue = [(cost, next(serial), value) for value, cost in entry.items()]
    heapify(queue)
    while queue:
        cost, _, value = heappop(queue)
        if cost != distance[value]:
            continue
        for target in moves(value):
            if target not in distance or cost + 1 < distance[target]:
                distance[target] = cost + 1
                heappush(queue, (cost + 1, next(serial), target))
    return distance


def uncovered(alphabet, guards, owners):
    """Count tuples outside every global guard rectangle by inclusion-exclusion."""
    total = len(alphabet) ** owners
    for size in range(1, len(guards) + 1):
        sign = (-1) ** size
        for group in combinations(guards, size):
            total += sign * len(alphabet.intersection(*group)) ** owners
    return total


def explore(owners, initial_phase, initial_value, local_moves, global_moves):
    """Local edges update one owner. A global edge preserves every tuple in guard**N.

    Each callback includes one entry per enabled action witness, including self-loops.
    A phase may have only one incoming edge other than stuttering. A second route
    can give a union of overlapping products and different joint shortest paths.
    """
    if owners < 1:
        raise ValueError("expected at least one participant")
    phases = {initial_phase}
    pending = [(initial_phase, 0, {initial_value: 0})]
    parts = []
    while pending:
        phase, hops, entry = pending.pop()
        distance = local_closure(entry, lambda value: local_moves(phase, value))
        alphabet = set(distance)
        degrees = {value: len(local_moves(phase, value)) for value in alphabet}
        all_guards = []
        for target, guard in global_moves(phase):
            allowed = {value for value in alphabet if guard(value)}
            if not allowed:
                continue
            all_guards.append(allowed)
            if target == phase:
                continue
            if target in phases:
                raise ValueError("multiple incoming edges or cycle in reachable coordinator phases")
            phases.add(target)
            pending.append((target, hops + 1, {v: distance[v] for v in allowed}))
        inactive = {value for value in alphabet if not degrees[value]}
        summed_degree = sum(degrees.values())
        cardinality = len(alphabet)
        parts.append({
            "phase": phase, "local_values": sorted(alphabet), "local_degree_sum": summed_degree,
            "global_guard_sizes": sorted(map(len, all_guards)), "phase_changes": hops,
            "max_local_distance": max(distance.values()), "distinct": cardinality ** owners,
            "generated_edges": owners * summed_degree * cardinality ** (owners - 1)
                               + sum(len(guard) ** owners for guard in all_guards),
            "depth": 1 + hops + owners * max(distance.values()),
            "deadlocked": uncovered(inactive, all_guards, owners),
        })
    return {
        "distinct": sum(p["distinct"] for p in parts),
        "generated": 1 + sum(p["generated_edges"] for p in parts),
        "depth": max(p["depth"] for p in parts),
        "deadlocked": sum(p["deadlocked"] for p in parts), "phases": parts,
    }


def mixed_states(values, owners, first, second):
    """Count labeled tuples containing both disjoint participant status categories."""
    labels = Counter(value[0] for value in values)
    total = len(values)
    return (total ** owners - (total - labels[first]) ** owners - (total - labels[second]) ** owners
            + (total - labels[first] - labels[second]) ** owners)
