#!/usr/bin/env python3
"""Differential controls for the installed selector on unrelated finite layouts."""

import argparse
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

MODULE = r"""---- MODULE Cells ----
CONSTANT IDs
VARIABLES got, flag
Layout == got \subseteq IDs /\ flag \in {0,1}
Init == got = {} /\ flag = 0
Single == \E i \in IDs : got' = got \cup {i} /\ UNCHANGED flag
Pair == \E i \in IDs : got = {} /\ got' = {i} /\ flag' = 1
Next == Pair \/ UNCHANGED <<got,flag>>
Empty == got = {}
AlwaysEmpty == []Empty
====
"""


def check(dive, module, *opts):
    result = subprocess.run([dive, "check", module, "--output", "json", "--time-limit", "30s", *opts],
                            text=True, capture_output=True, timeout=45)
    return result.returncode, json.loads(result.stdout)


def run(dive, root):
    module = root / "Cells.tla"
    module.write_text(MODULE)
    ids = ", ".join(f"a{i}" for i in range(22))
    base = f"CONSTANT IDs = {{{ids}}}\nINIT Init\nNEXT Next\nINVARIANT Layout\n"
    cfg = module.with_suffix(".cfg")
    cfg.write_text(base)
    auto = ["--strategy", "auto", "--workers", "2"]
    rc, row = check(dive, module, *auto)
    core_rc, core = check(dive, module, "--workers", "1", "--store", "exact")
    assert rc == core_rc == 0 and row["strategy"] == "compiled", row
    keys = ["distinct", "generated", "depth", "left_on_queue"]
    assert [row[k] for k in keys] == [core["statistics"][k] for k in keys] == [23, 46, 2, 0]
    cfg.write_text(base + "INVARIANT Empty\n")
    rc, row = check(dive, module, *auto)
    assert rc == 1 and row["trace_validated"] and row["ending"]["name"] == "Empty", row
    cfg.write_text(base.replace("NEXT Next", "NEXT Single"))
    rc, row = check(dive, module, *auto)
    assert rc == 0 and row["strategy"] == "symbolic", row
    assert [row[k] for k in keys] == [2**22, 1 + 22 * 2**22, 23, 0], row
    # A temporal obligation must pass through the unmodified Core request.
    cfg.write_text(base + "PROPERTY AlwaysEmpty\n")
    rc, row = check(dive, module, *auto)
    core_rc, core = check(dive, module, "--workers", "2", "--store", "exact")
    assert rc == core_rc != 0 and row["verdict"] == core["verdict"], row
    assert row["counterexample"]["validation"] == "valid", row


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    args = parser.parse_args()
    with TemporaryDirectory() as folder:
        run(args.dive.resolve(), Path(folder))
