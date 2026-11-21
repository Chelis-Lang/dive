#!/usr/bin/env python3
"""Generate one independently checkable cell of an INIT/NEXT safety model."""

import argparse
from pathlib import Path
import re


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path, help="original TLA+ module")
    parser.add_argument("config", type=Path, help="simple INIT/NEXT safety configuration")
    parser.add_argument("output", type=Path, help="new directory for this cell")
    parser.add_argument("--when", action="append", required=True, metavar="PREDICATE",
                        help="state predicate, e.g. 'role = 1'; first predicate is the low bit")
    parser.add_argument("--cell", required=True, type=int, metavar="N")
    args = parser.parse_args()
    if args.cell < 0 or args.cell.bit_length() > len(args.when):
        parser.error("cell index is outside the partition")

    # Accept only the configuration grammar we preserve in full. A symmetry
    # quotient, VIEW or temporal claim would require a different composition proof.
    identifier = r"[A-Za-z_][A-Za-z0-9_]*"
    config = args.config.read_text()
    original = args.module.stem
    if not re.fullmatch(identifier, original) or not args.module.is_file():
        parser.error("expected an existing module with a TLA+ identifier as its filename")
    config = re.sub(r"\\\*[^\n]*", "", config)
    tokens = config.split()
    keywords = {"INIT", "NEXT", "INVARIANT", "INVARIANTS", "CHECK_DEADLOCK"}
    fields = []
    one_name = {"INIT", "NEXT", "CHECK_DEADLOCK"}
    for token in tokens:
        if token in keywords:
            fields.append((token, []))
        elif not fields or not re.fullmatch(identifier, token) or (re.fullmatch(r"[A-Z_]{2,}", token) and token not in {"TRUE", "FALSE"}):
            parser.error(f"unsupported configuration token {token!r}")
        else:
            fields[-1][1].append(token)
    if any(not values or key in one_name and len(values) != 1 for key, values in fields):
        parser.error("expected a simple INIT/NEXT safety configuration")
    for key in ("INIT", "NEXT"):
        if sum(kind == key for kind, _ in fields) != 1:
            parser.error(f"expected exactly one {key}")
    if any(key == "CHECK_DEADLOCK" and values[0] not in {"TRUE", "FALSE"} for key, values in fields):
        parser.error("CHECK_DEADLOCK must be TRUE or FALSE")

    name = f"DiveSafetyCell{args.cell}"
    init = next(values[0] for key, values in fields if key == "INIT")
    parts = [f"({p})" if args.cell >> i & 1 else f"~({p})" for i, p in enumerate(args.when)]
    source = (f"---- MODULE {name} ----\nEXTENDS {original}\n"
              + "DiveCellPredicate == " + " /\\ ".join(parts) + "\n"
              + f"DiveCellInit == {init} /\\ DiveCellPredicate\n====\n")
    lines = ["INIT DiveCellInit" if key == "INIT" else " ".join([key, *values]) for key, values in fields]
    lines.append("INVARIANT DiveCellPredicate")
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / (name + ".tla")).write_text(source)
    (args.output / (name + ".cfg")).write_text("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
