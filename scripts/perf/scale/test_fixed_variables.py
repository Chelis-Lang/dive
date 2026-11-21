#!/usr/bin/env python3
"""Regression control for incoming shortcuts and fixed tuple components."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile

from fixed_variables import fixed_variables


def main(binder):
    model = r"""---- MODULE Projection ----
EXTENDS Naturals
VARIABLES loc, tag, tick
Init == loc = 0 /\ tag \in {0, 1} /\ tick = 0
Frame == <<tag, tick>>
Step == /\ loc < 4 /\ loc' = loc + 1 /\ UNCHANGED Frame
Flip == /\ loc = 0 /\ loc' = 4 /\ tag' = 1 - tag /\ tick = tick'
Filtered == IF tag = 0 THEN Step ELSE Flip
Open == Step \/ Flip
====
"""
    with tempfile.TemporaryDirectory() as folder:
        path = Path(folder) / "Projection.tla"
        path.write_text(model)
        cfg = path.with_suffix(".cfg")
        for action, extra, expected in [
            ("Step", "", ["tag", "tick"]),
            ("Open", "", ["tick"]),
            ("Filtered", "", ["tick"]),
            ("Step", "CONSTANT Frame = 0\n", []),
        ]:
            cfg.write_text(f"INIT Init\nNEXT {action}\n{extra}")
            bound = subprocess.check_output([binder, path, cfg])
            actual = fixed_variables(json.loads(bound))
            if actual != expected:
                raise AssertionError((action, extra, expected, actual))


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve())
