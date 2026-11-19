#!/usr/bin/env python3
"""Print the module closure of a TLA+ model and any imports outside it.

Usage: closure.py <corpus-root> <module.tla> [<module.tla>...]
"""
import json
import os
import re
import sys

STD = {"Naturals", "Integers", "Sequences", "FiniteSets", "Bags"}
IMPORT = re.compile(r"\b(?:EXTENDS|INSTANCE)\s+([A-Za-z0-9_]+(?:\s*,\s*[A-Za-z0-9_]+)*)")


def strip_comments(text):
    text = re.sub(r"\(\*.*?\*\)", " ", text, flags=re.S)
    return re.sub(r"\\\*[^\n]*", " ", text)


def closure(root, module):
    base = os.path.dirname(module)
    files, outside, todo = [], set(), [os.path.basename(module)[:-4]]
    seen = set()
    while todo:
        name = todo.pop()
        if name in seen:
            continue
        seen.add(name)
        path = os.path.join(root, base, name + ".tla")
        if not os.path.isfile(path):
            outside.add(name)
            continue
        files.append(os.path.join(base, name + ".tla"))
        with open(path, encoding="utf-8") as f:
            body = strip_comments(f.read())
        # only the module itself, not text after the closing ====
        for m in IMPORT.finditer(body):
            for n in re.split(r"\s*,\s*", m.group(1)):
                if n in STD:
                    continue
                todo.append(n)
    return sorted(files), sorted(outside)


if __name__ == "__main__":
    root = sys.argv[1]
    for mod in sys.argv[2:]:
        files, outside = closure(root, mod)
        print(json.dumps({"module": mod, "files": files, "outside": outside}))
