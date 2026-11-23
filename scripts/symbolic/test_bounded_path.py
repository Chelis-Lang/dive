#!/usr/bin/env python3
"""Exercise exact-depth images, k-induction and receipt binding in fresh processes."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile


SOURCE = r"""---- MODULE PathControls ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ /\ x = 0 /\ x' = 0
        \/ /\ x \in 1..2 /\ x' = x + 1
TypeOK == x \in 0..3
Safe == x # 3
Weaker == x \in 0..3
Admitted == x # 3
====
"""
CFG = "INIT Init\nNEXT Next\nINVARIANTS TypeOK Safe\nCHECK_DEADLOCK FALSE\n"


def controls(dive, exporter, directory):
    folder = Path(directory)
    module, config = folder / "PathControls.tla", folder / "PathControls.cfg"
    receipt, trace = folder / "path-receipt.json", folder / "word.json"
    module.write_text(SOURCE)
    config.write_text(CFG)
    worker = Path(__file__).with_name("check.py")

    def run(*options, code=0):
        result = subprocess.run([sys.executable, worker, module, config, "--dive", dive,
                                 "--exporter", exporter, "--type-invariant", "TypeOK",
                                 "--symbolic-path", *options], capture_output=True, text=True, check=False)
        assert result.returncode == code, (result.returncode, result.stdout, result.stderr)
        return json.loads(result.stdout)

    def replay(code):
        checked = subprocess.run([dive, "validate-trace", module, "--trace", trace, "--config",
                                  config, "--output", "json"], capture_output=True, text=True, check=False)
        assert (checked.returncode == 0) == code, (checked.stdout, checked.stderr)

    bounds = ("--path-bound", "0", "--path-bound", "2", "--path-bound", "5")
    row = run(*bounds)
    assert row["verdict"] == "bounded-safe" and not row["model_proved"]
    assert [item["admitted_states"] for item in row["completed_bounds"]] == [1, 1, 1]

    row = run(*bounds, "--k-induction", "2", "--write-path-receipt", receipt)
    assert row["verdict"] == "induction-inconclusive" and not row["model_proved"]
    assert row["induction"]["base"] == "unsat" and row["induction"]["step"] == "sat"
    assert "trace" not in row
    assert run(*bounds, "--k-induction", "2", "--verify-path-receipt", receipt)["receipt_rechecked"]
    forged = folder / "forged-receipt.json"
    changed = json.loads(receipt.read_text())
    changed["result"]["model_proved"] = True
    forged.write_text(json.dumps(changed))
    assert "freshly checked" in run(*bounds, "--k-induction", "2", "--verify-path-receipt", forged, code=2)["reason"]
    assert "bound query" in run(*bounds, "--k-induction", "1", "--verify-path-receipt", receipt, code=2)["reason"]
    config.write_text(CFG.replace("TypeOK Safe", "TypeOK Weaker"))
    assert "bound query" in run(*bounds, "--k-induction", "2", "--verify-path-receipt", receipt, code=2)["reason"]
    config.write_text(CFG)
    module.write_text(SOURCE.replace("x # 3", "x # 2", 1))
    assert "bound query" in run(*bounds, "--k-induction", "2", "--verify-path-receipt", receipt, code=2)["reason"]

    module.write_text(SOURCE.replace("x' = 0", "x' = 1"))
    config.write_text(CFG + "CONSTRAINT Admitted\n")
    row = run(*bounds, "--trace", trace, code=1)
    assert row["path_depth"] == 3 and row["ending"]["name"] == "Safe" and row["trace_validated"]
    replay(True)
    damaged = json.loads(trace.read_text())
    damaged["states"][-1]["values"]["x"] = 0
    trace.write_text(json.dumps(damaged))
    replay(False)

    module.write_text(SOURCE.replace("x' = 0", r"x' = 0 /\ FALSE"))
    config.write_text(CFG)
    row = run(*bounds)
    assert row["verdict"] == "executions-too-short"
    assert [item["executions"] for item in row["completed_bounds"]] == ["sat", "unsat", "unsat"]
    assert run(*bounds, "--time-limit", "0.000001", code=4)["verdict"] == "stopped"

    module.write_text(SOURCE.replace("x \\in 1..2", "x = 1"))
    row = run(*bounds, "--k-induction", "2")
    assert row["verdict"] == "inductive-safety" and row["model_proved"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dive", type=Path, default=Path("target/release/dive"))
    parser.add_argument("--exporter", type=Path, default=Path("target/release/examples/symbolic_input"))
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="dive-symbolic-path-") as directory:
        controls(args.dive.resolve(), args.exporter.resolve(), directory)


if __name__ == "__main__":
    main()
