"""Strips a dump_models.sh output for committing: drops TLC's console output,
the constant-definition values and the intern table (large, and reproducible
with the tooling), keeping the binding itself.

    python3 -I strip_dump.py [--keep-interned] < full.jsonl > stripped.jsonl

With --keep-interned, the intern table stays (for the repository's own
models, whose tables the tests compare).
"""
import json
import sys


def main():
    keep = "--keep-interned" in sys.argv[1:]
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        d = json.loads(line)
        interned = d.get("interned") if keep else d.pop("interned", None)
        d.pop("out", None)
        d.pop("constantDefns", None)
        if not keep and isinstance(interned, dict):
            d["internFirst"] = interned.get("first")
        print(json.dumps(d, ensure_ascii=False, sort_keys=False))


if __name__ == "__main__":
    main()
