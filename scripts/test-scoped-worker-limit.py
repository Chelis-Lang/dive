#!/usr/bin/env python3
"""Exercise ordinary safety checking in a Linux user cgroup with three task slots."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile


def check(binary, root, relation, safe, disk):
    module = root / "ThreadLimit.tla"
    module.write_text(
        "---- MODULE ThreadLimit ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n"
        f"Next == {relation}\nSafe == {safe}\n====\n"
    )
    cfg = root / "model.cfg"
    cfg.write_text("INIT Init\nNEXT Next\nINVARIANT Safe\n")
    command = [binary, "check", module, "--config", cfg, "--store", "exact",
               "--seed", "0", "--fp", "0", "--output", "json"]
    reference = subprocess.run(command + ["--workers", "1"], capture_output=True)
    expected = json.loads(reference.stdout)
    if disk:
        command += ["--candidate-memory-bytes", "128", "--candidate-directory", root]
    bounded = subprocess.run(
        ["systemd-run", "--user", "--quiet", "--wait", "--pipe", "--collect",
         "--property=TasksMax=3", *command, "--workers", "16"],
        capture_output=True,
    )
    actual = json.loads(bounded.stdout)
    assert bounded.returncode == reference.returncode, bounded.stderr
    for field in ("verdict", "statistics", "property", "output"):
        assert actual[field] == expected[field], (field, actual[field], expected[field])
    workers = actual["options"]["workers"]
    assert workers["requested"] == 16 and 1 <= workers["used"] < 16
    assert "operating system" in (workers["reason"] or ""), workers
    if actual["counterexample"]:
        trace = root / "trace.json"
        trace.write_text(json.dumps(actual["counterexample"]["trace"]))
        replay = subprocess.check_output(
            [binary, "validate-trace", module, "--config", cfg, "--trace", trace,
             "--seed", "0", "--fp", "0", "--output", "json"]
        )
        assert json.loads(replay)["result"] == "valid"


if __name__ == "__main__":
    dive = Path(sys.argv[1]).resolve()
    for candidate_spill in (False, True):
        for action, invariant in (
            (r"x' \in 1..128", r"x \in 0..128"),
            (r"IF x = 0 THEN x' \in 1..128 ELSE x' = 129", "x < 129"),
        ):
            with tempfile.TemporaryDirectory() as directory:
                check(dive, Path(directory), action, invariant, candidate_spill)
