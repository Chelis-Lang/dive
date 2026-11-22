#!/usr/bin/env python3
"""Check boundary revalidation and the owned deadline through an installed CLI."""

import argparse
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


def run(dive, root):
    module = root / "Bounded.tla"
    module.write_text(r"""---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Stop == x' = 1
InDomain == x < 2
Nonnegative == x >= 0
NoBoundary == x < 2
====
""")
    cfg = module.with_suffix(".cfg")
    base = "INIT Init\nNEXT Next\nCONSTRAINT InDomain\nCHECK_DEADLOCK FALSE\n"
    cfg.write_text(base + "INVARIANT Nonnegative\n")
    artifact = root / "certificate.json"
    key = root / "key"
    with open(key, "xb", opener=lambda path, flags: os.open(path, flags, 0o600)) as stream:
        stream.write(os.urandom(32))
    command = [dive, "check", module, "--config", cfg, "--strategy", "finite-induction", "--output", "json"]

    def checked(option, status, sign=True, deadline="10s", profile="dive"):
        env = dict(os.environ)
        env.pop("DIVE_SYMBOLIC_PROOF_KEY", None)
        env["DIVE_COMPAT"] = profile
        if sign:
            env["DIVE_SYMBOLIC_PROOF_KEY"] = str(key)
        result = subprocess.run([*command, option, artifact, "--time-limit", deadline],
                                capture_output=True, text=True, env=env, timeout=12)
        if result.returncode != status:
            raise ValueError((result.returncode, result.stdout, result.stderr))
        return json.loads(result.stdout)

    assert checked("--write-predicate", 0)["proof"] == "inductive-safety"
    first = artifact.read_bytes()
    assert checked("--write-predicate", 0)["proof"] == "inductive-safety"
    assert artifact.read_bytes() == first
    data = json.loads(first)["predicate"]
    assert len(data["inside"]) == 2 and len(data["boundary"]) == 1
    assert checked("--verify-predicate", 0)["authorized_closure"]
    assert checked("--verify-predicate", 4, deadline="1ms")["verdict"] == "stopped"
    cfg.write_text(base + "INVARIANT Nonnegative NoBoundary\n")
    for sign in (True, False):
        result = checked("--verify-predicate", 2, sign)
        assert result["proof"] == "inconclusive" and "NoBoundary" in result["failed_obligation"]

    cfg.write_text(base.replace("NEXT Next", "NEXT Stop") + "INVARIANT NoBoundary\n")
    stopped_relation = checked("--verify-predicate", 0)
    assert stopped_relation["proof"] == "inductive-safety"
    assert stopped_relation["reused_obligations"] == 0

    source = module.read_text()
    module.write_text(source.replace("EXTENDS Naturals", "EXTENDS Naturals, TLC")
                      .replace("x' = x + 1", "x' = RandomElement({0, 1})"))
    cfg.write_text(base + "INVARIANT Nonnegative\n")
    report = checked("--write-predicate", 2)
    assert report["verdict"] == "unsupported" and "RandomElement" in report["reason"]
    assert artifact.read_bytes() == first

    module.write_text(r"""---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
Init == x = 2147483647
Next == IF x = 2147483647 THEN x' = x + 1 ELSE UNCHANGED x
Nonnegative == x >= 0
====
""")
    cfg.write_text("INIT Init\nNEXT Next\nINVARIANT Nonnegative\n")
    assert checked("--write-predicate", 0)["proof"] == "inductive-safety"
    for operation in ("--write-predicate", "--verify-predicate"):
        refusal = checked(operation, 2, profile="tlc")
        assert refusal["verdict"] == "unsupported" and "profile" in refusal["reason"]

    module.write_text(r"""---- MODULE Bounded ----
VARIABLE x
Step == 0
Init == x = 0
Next == x' = Step
Safety == x = 0
====
""")
    original_cfg = "INIT Init\nNEXT Next\nINVARIANT Safety\nCONSTANT Step = 0\n"
    cfg.write_text(original_cfg)
    assert checked("--write-predicate", 0)["proof"] == "inductive-safety"
    cfg.write_text(original_cfg.replace("Step = 0", "Step = 1"))
    assert "Safety" in checked("--verify-predicate", 2)["failed_obligation"]
    cfg.write_text(original_cfg)
    assert checked("--verify-predicate", 0)["authorized_closure"]

    module.write_text(r"""---- MODULE Bounded ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = 1..2
Next == UNCHANGED x
Baseline == x = {1,2}
Rendered == ToString(x) = "{1, 2}"
====
""")
    cfg.write_text("INIT Init\nNEXT Next\nINVARIANT Baseline Rendered\n")
    assert "ToString" in checked("--write-predicate", 2)["reason"]
    assert "ToString" in checked("--verify-predicate", 2)["failed_obligation"]

    module.write_text(r"""---- MODULE Bounded ----
EXTENDS Naturals, FiniteSets
VARIABLE x
Init == x = (SUBSET {1, 2}) \ {{}}
Next == UNCHANGED x
Safety == IsFiniteSet(x)
====
""")
    cfg.write_text("INIT Init\nNEXT Next\nINVARIANT Safety\n")
    expected_refusal = "representation-sensitive FiniteSets!IsFiniteSet"
    assert expected_refusal in checked("--write-predicate", 2)["reason"]
    assert expected_refusal in checked("--verify-predicate", 2)["failed_obligation"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    options = parser.parse_args()
    with TemporaryDirectory() as work:
        run(options.dive.resolve(), Path(work))
