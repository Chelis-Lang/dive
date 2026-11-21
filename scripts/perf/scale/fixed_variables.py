#!/usr/bin/env python3
"""Identify state variables whose value every action in a bound Next preserves."""

import argparse
import json
from pathlib import Path
import subprocess


def fixed_variables(doc):
    if doc.get("format") != "dive-symbolic-input-v2" or doc["symmetry"]:
        raise ValueError("expected a bound safety model without symmetry")
    ir = doc["ir"]
    memo = {}

    def visit(index, frame=False):
        key = (index, frame)
        if key in memo:
            return memo[key]
        # A cyclic or unrecognized expression provides no evidence.
        memo[key] = frozenset()
        expr = ir["exprs"][index]["expr"]
        if isinstance(expr, str):
            return memo[key]
        tag, data = next(iter(expr.items()))
        found = frozenset()
        if tag == "Def":
            d = ir["defs"][data]
            if (str(data) not in doc["overrides"] and d["scope"] == "TopLevel"
                    and d["source"] is None and not d["std"] and not d["recursive"] and not d["params"]):
                found = visit(d["body"], frame)
        elif frame:
            if tag == "Var" and not data["primed"]:
                found = frozenset([data["var"]])
            elif tag == "Tuple":
                found = frozenset().union(*(visit(item, True) for item in data))
        elif tag == "Unchanged":
            found = visit(data, True)
        elif tag == "Junction" and data["items"]:
            children = [visit(i) for i in data["items"]]
            if data["kind"] == "And":
                found = frozenset.union(*children)
            elif data["kind"] == "Or":
                found = frozenset.intersection(*children)
        elif tag == "If":
            found = visit(data["then"]) & visit(data["other"])
        elif tag == "Quant" and data["kind"] == "Exists":
            found = visit(data["body"])
        elif tag == "Builtin" and data["op"] == "Eq":
            lhs, rhs = (ir["exprs"][i]["expr"] for i in data["args"])
            if isinstance(lhs, dict) and isinstance(rhs, dict) and "Var" in lhs and "Var" in rhs:
                a, b = lhs["Var"], rhs["Var"]
                if a["var"] == b["var"] and a["primed"] != b["primed"]:
                    found = frozenset([a["var"]])
        memo[key] = found
        return found

    indices = visit(doc["next"])
    return [ir["decls"][decl]["name"] for n, decl in enumerate(ir["vars"]) if n in indices]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binder", type=Path, help="path to the matching symbolic_input exporter")
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    args = parser.parse_args()
    try:
        bound = subprocess.run([args.binder.resolve(), args.module, args.config],
                               stdout=subprocess.PIPE, check=True)
        names = fixed_variables(json.loads(bound.stdout))
    except (OSError, subprocess.CalledProcessError, ValueError, RecursionError) as error:
        parser.exit(2, f"{error}\n")
    print(json.dumps({"fixed_variables": names}))


if __name__ == "__main__":
    main()
