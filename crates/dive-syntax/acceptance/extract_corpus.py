#!/usr/bin/env python3
"""Split tree-sitter corpus files into one Test.tla per test case.

Usage: extract_corpus.py CORPUS_DIR OUT_DIR
Writes OUT_DIR/NNNN/Test.tla and OUT_DIR/index.json.
Mirrors tlaplus test/util/SyntaxCorpusFileParser.java.
"""
import json
import os
import re
import sys

HEADER = re.compile(r"^===+\|\|\|\r?\n(?P<name>.*?(?=\r?\n===+\|\|\|))\r?\n===+\|\|\|\r?\n", re.M | re.S)
SEP = re.compile(r"^---+\|\|\|\r?\n", re.M)


def tests_in(path):
    content = open(path, encoding="utf-8").read()
    heads = list(HEADER.finditer(content))
    seps = list(SEP.finditer(content))
    out = []
    for i, h in enumerate(heads):
        sep = seps[i]
        header = h.group("name")
        name = "\n".join(l for l in header.splitlines() if not l.startswith(":")).strip()
        attrs = [l.strip()[1:].lower() for l in header.splitlines() if l.startswith(":")]
        src = content[h.end():sep.start()]
        end = heads[i + 1].start() if i + 1 < len(heads) else len(content)
        out.append({"name": name, "attrs": attrs, "input": src,
                    "expected": content[sep.end():end]})
    return out


def main():
    corpus, outdir = sys.argv[1:3]
    index = []
    n = 0
    for root, _, files in sorted(os.walk(corpus)):
        for f in sorted(files):
            if not f.endswith(".txt"):
                continue
            p = os.path.join(root, f)
            for t in tests_in(p):
                d = os.path.join(outdir, "%04d" % n)
                os.makedirs(d, exist_ok=True)
                with open(os.path.join(d, "Test.tla"), "w", encoding="utf-8") as fh:
                    fh.write(t["input"])
                index.append({"id": "%04d" % n, "file": os.path.relpath(p, corpus),
                              "name": t["name"], "attrs": t["attrs"]})
                n += 1
    with open(os.path.join(outdir, "index.json"), "w") as fh:
        json.dump(index, fh, indent=1)
    print(n)


if __name__ == "__main__":
    main()
