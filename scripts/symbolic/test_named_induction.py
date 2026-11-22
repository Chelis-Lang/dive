#!/usr/bin/env python3
"""Run focused named-induction regressions using the bound-IR exporter."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import sys
from tempfile import TemporaryDirectory

CHECK = Path(__file__).with_name("check_named_induction.py")
PANEL = Path(__file__).resolve().parents[1] / "perf/scale"
MODULE = r"""---- MODULE Bounds ----
EXTENDS Naturals
VARIABLES x,y
Init == /\ x \in {0,1} /\ y=2
Next == UNCHANGED <<x,y>>
Admit == x=0
Safety == x=x
Layout == /\ x \in {0,1} /\ y \in {2}
All == y=2
Subset == x=0
====
"""
CFG = "INIT Init\nNEXT Next\nCONSTRAINT Admit\nINVARIANT Safety\nCHECK_DEADLOCK FALSE\n"


def check(args, expected):
    r = subprocess.run(args, capture_output=True, text=True)
    if r.returncode != expected:
        raise ValueError((args, r.returncode, r.stdout, r.stderr))
    return json.loads(r.stdout)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exporter", type=Path, required=True)
    exporter = parser.parse_args().exporter.resolve()
    with TemporaryDirectory() as temp:
        base = Path(temp)
        source = base / "one"
        source.mkdir()
        (source / "Bounds.tla").write_text(MODULE)
        (source / "Bounds.cfg").write_text(CFG)
        other = base / "two"
        shutil.copytree(source, other)
        for optimize in ([], ["-O"]):
            def command(directory, candidate="All"):
                return [sys.executable, *optimize, CHECK, directory / "Bounds.tla", directory / "Bounds.cfg",
                        "--exporter", exporter, "--layout", "Layout", "--property", "Safety",
                        "--candidate", candidate]

            (other / "Bounds.cfg").write_text(CFG)
            failure = check(command(source, "Subset"), 2)
            if "undischarged initiation" not in failure["reason"]:
                raise ValueError(failure)
            receipt = base / "receipt.json"
            genuine = check([*command(source), "--save", receipt], 0)
            if check([*command(other), "--verify", receipt], 0) != genuine:
                raise ValueError("identical input moved to another directory")
            modified = json.loads(receipt.read_bytes())
            if modified["clauses"] != 1:
                raise ValueError("unexpected number of synthetic clauses")
            modified["clauses"] = True
            mistyped = base / "boolean.json"
            mistyped.write_text(json.dumps(modified))
            if "differs from independently checked" not in check([*command(other), "--verify", mistyped], 2)["reason"]:
                raise ValueError("boolean accepted in place of an integer")
            (other / "Bounds.cfg").write_text(CFG + "\n")
            if "different source or configuration" not in check([*command(other), "--verify", receipt], 2)["reason"]:
                raise ValueError("changed CFG bytes escaped the binding")
            # A validator that rejects the genuine word must prevent a positive
            # evidence record even if Python disables assert statements.
            fake = base / "refuse"
            fake.write_text('#!/bin/sh\necho \'{"result":"invalid"}\'\n')
            fake.chmod(0o700)
            probe = ("import pathlib, sys;sys.path.insert(0,sys.argv[1]);"
                     "from certify_reconfiguration import validate_pair;"
                     "validate_pair(sys.argv[2],'Unused.tla','Unused.cfg',{'states':[]},pathlib.Path(sys.argv[3]))")
            r = subprocess.run([sys.executable, *optimize, "-c", probe, PANEL, fake, base], capture_output=True)
            if r.returncode == 0 or b"refused genuine trace" not in r.stderr:
                raise ValueError("replay failure was skipped")
    print("full initiation, relocated input, changed CFG and mandatory replay passed in both Python modes")


if __name__ == "__main__":
    main()
