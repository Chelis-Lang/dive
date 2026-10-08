#!/usr/bin/env python3
"""Confusion matrix of dive-sema against SANY (`-error-codes`).

Usage: compare.py SANY.jsonl DIVE.jsonl [--codes]

SANY.jsonl comes from run_sany.py (or harness goldens converted by
goldens.py); DIVE.jsonl from `cargo run --release -p dive-sema --example
acceptance`. Prints the matrix (both accept, both reject, dive-only accept,
SANY-only accept) and lists every disagreement. With --codes, for files
named like the error corpus (`E4204_..._Test.tla`), also checks that dive's
diagnostics cross-reference the expected SANY code.
"""
import json
import os
import re
import sys


def load(p):
    out = {}
    for line in open(p):
        line = line.strip()
        if line:
            r = json.loads(line)
            out[os.path.realpath(r["path"])] = r
    return out


def main():
    sany = load(sys.argv[1])
    dive = load(sys.argv[2])
    codes = "--codes" in sys.argv
    m = {"both accept": 0, "both reject": 0, "dive-only accept": 0, "SANY-only accept": 0}
    excluded = []
    dis = []
    code_miss = []
    code_hit = 0
    for path, s in sorted(sany.items()):
        if s["verdict"] not in ("accept", "reject"):
            excluded.append((path, s["verdict"]))
            continue
        d = dive.get(path)
        if d is None or d["verdict"] not in ("accept", "reject"):
            excluded.append((path, "dive " + (d["verdict"] if d else "missing")))
            continue
        sa, da = s["verdict"] == "accept", d["verdict"] == "accept"
        if sa and da:
            m["both accept"] += 1
        elif not sa and not da:
            m["both reject"] += 1
        elif da:
            m["dive-only accept"] += 1
            dis.append((path, s, d))
        else:
            m["SANY-only accept"] += 1
            dis.append((path, s, d))
        mm = re.match(r"E(\d{4})(_\S+)?_Test\.tla$", os.path.basename(path))
        if codes and mm and not sa and not da:
            want = int(mm.group(1))
            if want in d.get("sany", []):
                code_hit += 1
            else:
                code_miss.append((path, want, d.get("sany", []), d.get("codes", [])))
    total = sum(m.values())
    print("files: %d (excluded %d)" % (total, len(excluded)))
    for k, v in m.items():
        print("  %-18s %d" % (k, v))
    for p, why in excluded:
        print("EXCLUDED %s: %s" % (p, why))
    for p, s, d in dis:
        print("DISAGREE %s\n  sany: %s %s\n  dive: %s %s %s" % (
            p, s["verdict"], (s.get("error") or "")[:300].replace("\n", " | "),
            d["verdict"], d.get("codes"), d.get("message", "")[:300]))
    if codes:
        print("error-corpus code cross-reference: %d match, %d miss" % (code_hit, len(code_miss)))
        for p, want, got, dc in code_miss:
            print("  CODE %s: want %d, dive cross-refs %s (%s)" % (os.path.basename(p), want, got, dc))


if __name__ == "__main__":
    main()
