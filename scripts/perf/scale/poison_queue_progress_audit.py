#!/usr/bin/env python3
"""Compare the progress proof's original actor actions with a complete pinned TLC graph."""

import argparse
from dataclasses import dataclass, replace
from pathlib import Path
import re
import json

from poison_queue_graph import read_graph
from poison_queue_progress import configuration


@dataclass(frozen=True)
class State:
    prod: tuple
    cons: tuple
    buffer: tuple
    waiting: frozenset


def value(label):
    fields = dict(re.findall(r"\b(prod|cons|buffer|waitSet) = (.*?)(?=\\n|$)", label))
    if len(fields) != 4:
        raise ValueError(f"incomplete TLC state: {label}")

    def quotas(text, role):
        matches = re.findall(rf"({role}[1-9]\d*) :> (\d+)", text)
        if not matches or len(matches) != len({name for name, _ in matches}):
            raise ValueError("invalid actor quota function")
        return tuple(sorted((name, int(value)) for name, value in matches))

    def sequence(text, delimiters):
        if not text.startswith(delimiters[0]) or not text.endswith(delimiters[1]):
            raise ValueError("invalid finite sequence or set")
        inner = text.removeprefix(delimiters[0]).removesuffix(delimiters[1])
        return tuple(inner.split(", ")) if inner else ()

    return State(
        quotas(fields["prod"], "p"),
        quotas(fields["cons"], "c"),
        sequence(fields["buffer"], ("<<", ">>")),
        frozenset(sequence(fields["waitSet"], ("{", "}"))),
    )


def wake(waiting, others):
    notified = waiting & others
    return [waiting - {actor} for actor in sorted(notified)] if notified else [waiting]


def change(quotas, actor):
    return tuple((name, count - (actor == name)) for name, count in quotas)


def successors(source, producers, consumers, capacity):
    for producer, quota in source.prod:
        if quota <= 0 or producer in source.waiting:
            continue
        if len(source.buffer) == capacity:
            yield producer, replace(source, waiting=source.waiting | {producer})
        else:
            for waiting in wake(source.waiting, consumers):
                yield (
                    producer,
                    replace(source, waiting=waiting, buffer=(*source.buffer, producer)),
                )
                yield (
                    producer,
                    replace(
                        source,
                        waiting=waiting,
                        buffer=(*source.buffer, "Poison"),
                        prod=change(source.prod, producer),
                    ),
                )
    for consumer, quota in source.cons:
        if quota <= 0 or consumer in source.waiting:
            continue
        if not source.buffer:
            yield consumer, replace(source, waiting=source.waiting | {consumer})
        else:
            for waiting in wake(source.waiting, producers):
                remaining = (
                    change(source.cons, consumer)
                    if source.buffer[0] == "Poison"
                    else source.cons
                )
                yield (
                    consumer,
                    replace(
                        source,
                        waiting=waiting,
                        buffer=source.buffer[1:],
                        cons=remaining,
                    ),
                )


def fairness_labels(source, edges):
    # Every source action in this graph changes at least one original state
    # variable. The producer's strong fairness counts only quota decreases.
    return {
        (actor, target)
        for actor, target in edges
        if actor.startswith("c") or dict(target.prod)[actor] < dict(source.prod)[actor]
    }


def audit(graph, cfg):
    p, c, capacity, deadlock_disabled = configuration(cfg.read_text())
    if not deadlock_disabled:
        raise ValueError("the audit requires a complete deadlock-disabled oracle graph")
    labels, occurrence, root = read_graph(graph)
    states = {key: value(label) for key, label in labels.items()}
    initial = State(
        tuple((actor, len(c)) for actor in sorted(p)),
        tuple((actor, len(p)) for actor in sorted(c)),
        (),
        frozenset(),
    )
    if states[root] != initial or len(set(states.values())) != len(states):
        raise ValueError("initial values or exact state identities differ")
    fair_edges = enabled_labels = 0
    for key, source in states.items():
        expected = set(successors(source, set(p), set(c), capacity))
        actual = {(actor, states[target]) for actor, target in occurrence[key]}
        if expected != actual:
            raise ValueError(f"actor-labelled successors differ at source {key}")
        actual_labels = fairness_labels(source, actual)
        enabled = {
            actor
            for actor, quota in source.cons
            if quota > 0 and actor not in source.waiting
        }
        enabled |= {
            actor
            for actor, quota in source.prod
            if quota > 0
            and actor not in source.waiting
            and len(source.buffer) < capacity
        }
        if enabled != {actor for actor, _ in actual_labels}:
            raise ValueError(
                f"unrestricted fairness enablement differs at source {key}"
            )
        fair_edges += len(actual_labels)
        enabled_labels += len({actor for actor, _ in actual_labels})
    return {
        "states": len(states),
        "edges": sum(map(len, occurrence.values())),
        "taken_fairness_labels": fair_edges,
        "enabled_actor_labels": enabled_labels,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("graph", type=Path)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    print(json.dumps(audit(args.graph, args.config)))


if __name__ == "__main__":
    main()
