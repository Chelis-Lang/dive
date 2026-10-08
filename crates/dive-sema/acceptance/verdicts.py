#!/usr/bin/env python3
"""Writes a verdict file from run_sany.py output.

Usage: verdicts.py SANY.jsonl OUT.txt [ROOT]

Each line of OUT.txt is `<path relative to ROOT> accept|reject`, sorted.
ROOT defaults to the directory of the first file.
"""
import json
import os
import sys


def main():
    rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
    root = sys.argv[3] if len(sys.argv) > 3 else os.path.dirname(rows[0]["path"])
    out = []
    for r in rows:
        if r["verdict"] not in ("accept", "reject"):
            sys.exit("no verdict for %s: %s" % (r["path"], r["verdict"]))
        out.append("%s %s" % (os.path.relpath(r["path"], root), r["verdict"]))
    with open(sys.argv[2], "w") as fh:
        fh.write("# SANY (tla2tools 1.8.0 at tlaplus 94d0c50, -error-codes) verdicts.\n")
        fh.write("# Regenerate with crates/dive-sema/acceptance (see README.md).\n")
        for line in sorted(out):
            fh.write(line + "\n")


if __name__ == "__main__":
    main()
