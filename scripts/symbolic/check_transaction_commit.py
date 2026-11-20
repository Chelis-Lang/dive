#!/usr/bin/env python3
"""Check Lamport's TCommit with the source checker and an independent finite-state count."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def expected(n):
    # No manager has committed: each is working, prepared, or aborted.
    # A commit requires all others prepared or committed. Both regions include
    # the all-prepared state. Every assignment in both regions is reachable.
    states = 3**n + 2**n - 1
    # Each manager can prepare from working, abort from working/prepared, and
    # commit from prepared after every other manager has prepared.
    generated = 1 + n * (3**n + 2 ** (n - 1))
    return states, generated, 2 * n + 1


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("model", type=Path, help="pinned Examples transaction_commit directory")
    p.add_argument("--managers", type=int, nargs="+", default=[3, 32, 64])
    p.add_argument("--dive", default="target/release/dive")
    p.add_argument("--exporter", default="target/release/examples/symbolic_input")
    args = p.parse_args()
    src = args.model / "TCommit.tla"
    cfg = args.model / "TCommit.cfg"
    old = "RM = {r1, r2, r3}"
    if cfg.read_text().count(old) != 1:
        p.error("unexpected original RM domain")
    checker = [sys.executable, Path(__file__).with_name("check.py"),
               "--type-invariant", "TCTypeOK", "--dive", args.dive, "--exporter", args.exporter]
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        for n in args.managers:
            if not 1 <= n <= 256:
                p.error("expected 1 to 256 resource managers")
            names = ", ".join(f"r{i+1}" for i in range(n))
            conf = work / f"TCommit{n}.cfg"
            conf.write_text(cfg.read_text().replace(old, f"RM = {{{names}}}"))
            row = json.loads(subprocess.check_output([*checker, src, conf]))
            if row["verdict"] != "success" or tuple(row[k] for k in ("distinct", "generated", "depth")) != expected(n):
                raise ValueError(f"incorrect reachability result for {n} managers: {row}")
            print(json.dumps({"managers": n, **row}), flush=True)

        # A commit is possible only after every manager has prepared. Check the
        # shortest bad path with the actual source-level string-valued states.
        if src.read_text().count("THEOREM TCSpec") != 1:
            p.error("unexpected TCommit theorem")
        bad = work / src.name
        bad.write_text(src.read_text().replace("THEOREM TCSpec", r'''NoCommit == \A r \in RM : rmState[r] # "committed"
THEOREM TCSpec'''))
        conf = work / cfg.name
        conf.write_text(cfg.read_text() + "\nINVARIANT NoCommit\n")
        proc = subprocess.run([*checker, bad, conf], capture_output=True, text=True)
        result = json.loads(proc.stdout)
        if (proc.returncode, result.get("trace_validated"), result.get("depth"),
                result.get("ending")) != (1, True, 5, {"kind": "invariant", "name": "NoCommit"}):
            raise ValueError("expected validated commit violation at depth 5")


if __name__ == "__main__":
    main()
