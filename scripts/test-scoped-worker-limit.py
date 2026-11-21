#!/usr/bin/env python3
"""Exercise ordinary safety worker admission and reuse under Linux task quotas."""

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import uuid


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
    assert workers["requested"] == 16 and workers["used"] == 1
    assert "operating system" in (workers["reason"] or ""), workers
    if actual["counterexample"]:
        trace = root / "trace.json"
        trace.write_text(json.dumps(actual["counterexample"]["trace"]))
        replay = subprocess.check_output(
            [binary, "validate-trace", module, "--config", cfg, "--trace", trace,
             "--seed", "0", "--fp", "0", "--output", "json"]
        )
        assert json.loads(replay)["result"] == "valid"


def check_late_quota(binary, root):
    module = root / "ThreadLimit.tla"
    module.write_text(
        "---- MODULE ThreadLimit ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\n"
        "Next == x' \\in 1..512\n====\n"
    )
    cfg = root / "model.cfg"
    cfg.write_text("INIT Init\nNEXT Next\n")
    scratch = root / "candidates"
    scratch.mkdir()
    unit = f"dive-worker-limit-{uuid.uuid4().hex}.service"
    cmd = [
        "systemd-run", "--user", "--quiet", "--wait", "--pipe", "--collect",
        f"--unit={unit}", "--property=TasksMax=64", binary, "check", module,
        "--config", cfg, "--store", "exact", "--seed", "0", "--fp", "0",
        "--output", "json", "--workers", "4", "--candidate-memory-bytes", "128",
        "--candidate-directory", scratch,
    ]
    with subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as proc:
        # One root occupies one spool. Two coexisting spools mean the first
        # wave has committed and workers are expanding the next frontier.
        while proc.poll() is None and len(list(scratch.iterdir())) < 2:
            time.sleep(0.001)
        assert proc.poll() is None, "missed the active second frontier"
        subprocess.run(
            ["systemctl", "--user", "set-property", "--runtime", unit, "TasksMax=1"],
            check=True,
        )
        output, errors = proc.communicate(timeout=45)
    assert proc.returncode == 0, errors
    report = json.loads(output)
    assert report["verdict"] == "success"
    assert report["statistics"]["distinct"] == 513
    assert report["statistics"]["generated"] == 1 + 513 * 512
    workers = report["options"]["workers"]
    assert workers["used"] == workers["requested"] == 4 and workers["reason"] is None, workers


if __name__ == "__main__":
    dive = Path(sys.argv[1]).resolve()
    for candidate_spill in (False, True):
        for action, invariant in (
            (r"x' \in 1..128", r"x \in 0..128"),
            (r"IF x = 0 THEN x' \in 1..128 ELSE x' = 129", "x < 129"),
        ):
            with tempfile.TemporaryDirectory() as directory:
                check(dive, Path(directory), action, invariant, candidate_spill)
    with tempfile.TemporaryDirectory() as directory:
        check_late_quota(dive, Path(directory))
