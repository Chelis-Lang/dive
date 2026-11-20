#!/usr/bin/env python3
"""Emit an isolated Rust BFS from the existing finite relation compiler."""

import argparse
from collections import defaultdict
import itertools
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "scripts/symbolic"))
from engine import SourceModel, Unsupported, require  # noqa: E402


class Rust:
    """Put symmetry lanes next to one another and lower the Boolean DAG."""

    def __init__(self, model):
        roles = set(itertools.chain.from_iterable(model.roles))
        fields = [field for field in model.variables if field not in roles]
        self.fixed = len(fields)
        fields.extend(itertools.chain.from_iterable(model.roles))
        self.bits = {name: index for index, name in enumerate(fields)}
        self.model = model
        self.total_nodes = 0

    def edge(self, edge, nodes, lines):
        manager = self.model.b
        if edge == manager.true:
            return "true"
        if edge == manager.false:
            return "false"
        if edge.negated:
            return "!" + self.edge(~edge, nodes, lines)
        if edge in nodes:
            return nodes[edge]
        mask = 1 << self.bits[edge.var]
        low, high = self.edge(edge.low, nodes, lines), self.edge(edge.high, nodes, lines)
        name = f"bdd_{len(nodes)}"
        nodes[edge] = name
        self.total_nodes += 1
        require(self.total_nodes <= 8192, "generated decision DAG exceeds 8192 nodes")
        expression = (f"row & {mask:#x} != 0" if (low, high) == ("false", "true")
                      else f"if row & {mask:#x} != 0 {{ {high} }} else {{ {low} }}")
        lines.append(f"let {name} = {expression};")
        return name

    def action(self, guard, updates):
        # Adjacent destination bits that copy a contiguous source field can
        # move together, even when other bits in the row use general BDDs.
        nodes = {}
        before_guard = []
        condition = self.edge(guard, nodes, before_guard)
        after_guard = []
        copies = defaultdict(int)
        set_bits = 0
        compute = []
        written = 0
        for target, root in updates:
            index = self.bits[target]
            written |= 1 << index
            if root == self.model.b.true:
                set_bits |= 1 << index
            elif root == self.model.b.false:
                pass
            elif root == self.model.b.var(root.var):
                src = self.bits[root.var]
                copies[(index - src) % 64] |= 1 << src
            else:
                compute.append(f"((({self.edge(root, nodes, after_guard)}) as u64) << {index})")
        terms = [f"row & {~written & 0xffffffffffffffff:#x}"]
        if set_bits:
            terms.append(f"{set_bits:#x}")
        terms.extend(f"(row & {mask:#x}).rotate_left({shift})" for shift, mask in sorted(copies.items()))
        terms.extend(compute)
        body = "\n                ".join([*after_guard, f"self.emit({' | '.join(terms)});"])
        guard = "\n            ".join(before_guard)
        return f"{{\n            {guard}\n            if {condition} {{\n                {body}\n            }}\n        }}"

    def rank(self, point):
        return sum(1 << bit for name, bit in self.bits.items() if point[name])

    def emit(self):
        model = self.model
        require(model.constraint == model.b.true, "state constraints in the explicit prototype")
        require(0 < len(self.bits) < 64, "expected one to 63 state bits")
        require(len(model.roles) <= 6, "too many symmetry lanes")
        root_words = [self.rank(point) for point in model.b.pick_iter(model.initial, self.bits)]
        require(model.effects.total() <= 512, "generated worker exceeds 512 action witnesses")
        expand = "\n        ".join(
            self.action(guard, sorted(updates))
            for (guard, updates), count in model.effects.items()
            for _ in range(count)
        )
        properties = []
        for _, root in model.properties:
            lines = []
            holds = self.edge(root, {}, lines)
            if holds == "true":
                continue
            properties.append("{ " + " ".join(lines) + f" {holds} }}")
        condition = " && ".join(f"({part})" for part in properties) or "true"
        values = dict(
            WIDTH=len(self.bits),
            FIXED=self.fixed,
            ROLES=len(model.roles),
            LANE=len(model.roles[0]) if model.roles else 0,
            CHECK_DEADLOCK=str(model.doc["check_deadlock"]).lower(),
            INITIAL_GENERATED=model.initial_generated,
            INITIALS=", ".join(map(str, root_words)),
            EXPAND=expand,
            PROPERTIES=condition,
        )
        worker = Path(__file__).with_name("finite_worker.rs.in").read_text()
        for key, value in values.items():
            worker = worker.replace(f"@{key}@", str(value))
        self.metadata = {
            "width": len(self.bits),
            "symmetry_lanes": len(model.roles),
            "decision_nodes": self.total_nodes,
            "actions": model.effects.total(),
        }
        return worker


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exporter", type=Path)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--type-invariant", default="TypeInvariant")
    args = parser.parse_args()
    bound = subprocess.run([args.exporter, args.module, args.config], check=False, capture_output=True)
    if bound.returncode != 0:
        parser.exit(2, f"Binder exited {bound.returncode}: {bound.stderr.decode(errors='replace')}")
    doc = json.loads(bound.stdout)
    try:
        rust = Rust(SourceModel(doc, args.type_invariant))
        worker = rust.emit()
    except Unsupported as error:
        parser.exit(2, f"Unsupported: {error}\n")
    with args.output.open("x") as output:
        output.write(worker)
    print(json.dumps(rust.metadata))


if __name__ == "__main__":
    main()
