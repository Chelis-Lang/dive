#!/usr/bin/env python3
"""Confusion matrix of SANY parser acceptance vs dive-syntax acceptance.

Usage: compare.py SANY.jsonl DIVE.jsonl [--show N]

`name_mismatch` counts as SANY-accept: the parser accepted the module and
only the file-name check failed, which is not a parser property.
"""
import json
import sys


def load(p):
    out = {}
    for line in open(p):
        r = json.loads(line)
        out[r["path"]] = r
    return out


def main():
    sany = load(sys.argv[1])
    dive = load(sys.argv[2])
    show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 50
    m = {"both_accept": 0, "both_reject": 0, "dive_only_accept": 0, "sany_only_accept": 0}
    dis = []
    missing = 0
    for path, s in sorted(sany.items()):
        d = dive.get(path)
        if d is None:
            missing += 1
            continue
        if s["verdict"] == "crash":
            continue
        sa = s["verdict"] in ("accept", "name_mismatch")
        da = d["accept"]
        if sa and da:
            m["both_accept"] += 1
        elif not sa and not da:
            m["both_reject"] += 1
        elif da:
            m["dive_only_accept"] += 1
            dis.append(("dive-only-accept", path, s.get("message", ""), ""))
        else:
            m["sany_only_accept"] += 1
            dis.append(("sany-only-accept", path, "", d["error"]))
    print(json.dumps(m), "missing=%d" % missing)
    for kind, path, smsg, dmsg in dis[:show]:
        print(kind, path)
        if smsg:
            print("   sany:", smsg[:300])
        if dmsg:
            print("   dive:", dmsg[:300])


if __name__ == "__main__":
    main()
