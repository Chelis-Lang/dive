"""Compare actor-labelled TLC graphs of the original and payload-projected poison queue."""

import argparse
from collections import defaultdict
import json
from pathlib import Path
import re


NODE = re.compile(r'^(-?\d+) \[label="([^"]+)"')
EDGE = re.compile(r'^(-?\d+) -> (-?\d+) \[label="(Put\((p\d+),p\d+\)|Get\((c\d+)\))"')
BUFFER = re.compile(r"buffer = <<([^>]*)>>")


def read_graph(path):
    states = {}
    initial = []
    edges = defaultdict(set)
    with path.open() as graph:
        for line in graph:
            if match := EDGE.match(line):
                start, end, _, producer, consumer = match.groups()
                edges[start].add((producer or consumer, end))
            elif match := NODE.match(line):
                key, label = match.groups()
                if key in states or len(BUFFER.findall(label)) != 1:
                    raise ValueError("unexpected state")
                states[key] = label
                if "style = filled" in line:
                    initial.append(key)
            elif re.match(r"^-?\d+", line):
                raise ValueError(f"unrecognized graph entry: {line[:100]}")
    if len(initial) != 1 or not edges.keys() <= states.keys():
        raise ValueError("expected one initial state and complete source labels")
    return states, edges, initial[0]


def project(label):
    def buffer(match):
        entries = match[1].split(", ") if match[1] else []
        if any(not re.fullmatch(r"Poison|p\d+", item) for item in entries):
            raise ValueError("unrecognized payload")
        return "buffer = <<" + ", ".join("ordinary" if item != "Poison" else item
                                         for item in entries) + ">>"
    return BUFFER.sub(buffer, label)


def compare(original, projected):
    states, edges, initial = read_graph(original)
    quotient, steps, start = read_graph(projected)
    target = {project(label): key for key, label in quotient.items()}
    if len(target) != len(quotient) or set(target) != {project(s) for s in states.values()}:
        raise ValueError("the projected graph differs from the image of the original graph")
    image = {key: target[project(label)] for key, label in states.items()}
    if image[initial] != start:
        raise ValueError("initial states differ")
    for key, mapped in image.items():
        actual = {(actor, image[end]) for actor, end in edges[key]}
        if actual != steps[mapped]:
            raise ValueError(f"labelled successors differ at {key}")
    return {"original_states": len(states), "projected_states": len(quotient),
            "original_actor_edges": sum(map(len, edges.values())),
            "projected_actor_edges": sum(map(len, steps.values()))}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("original", type=Path)
    parser.add_argument("projected", type=Path)
    args = parser.parse_args()
    print(json.dumps(compare(args.original, args.projected)))
