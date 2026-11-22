#!/usr/bin/env python3
"""Exercise portable whole-value proof admission, edits and failure controls."""

import argparse
import json
import os
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory


ORIGINAL = r"""---- MODULE WholeControl ----
EXTENDS ControlInputs
VARIABLE item
TypeInvariant == item \in [members: SUBSET {"a","b","c"}, stage: {"set","empty"}]
Init == /\ TypeInvariant
        /\ item = [members |-> {}, stage |-> "empty"]
Next == /\ item.stage = "empty"
        /\ item' = [members |-> StepValue, stage |-> "set"]
Bounded == item.members # {"a", "b", "c"}
====
"""


def checked(command, expected, key=None):
    env = dict(os.environ)
    env.pop("DIVE_SYMBOLIC_PROOF_KEY", None)
    if key is not None:
        env["DIVE_SYMBOLIC_PROOF_KEY"] = str(key)
    completed = subprocess.run(command, capture_output=True, text=True, env=env, timeout=30)
    if completed.returncode != expected:
        raise RuntimeError((completed.returncode, completed.stdout, completed.stderr))
    return json.loads(completed.stdout)


def run(dive, sail, root):
    module = root / "WholeControl.tla"
    source = root / "ControlInputs.tla"
    cfg = module.with_suffix(".cfg")
    doc = "INIT Init\nNEXT Next\nINVARIANTS TypeInvariant Bounded\nCHECK_DEADLOCK FALSE\n"
    imported = '---- MODULE ControlInputs ----\nStepValue == {"a"}\n====\n'
    module.write_text(ORIGINAL)
    source.write_text(imported)
    cfg.write_text(doc)
    key = root / "key"
    key.write_bytes(os.urandom(32))
    key.chmod(0o600)
    predicate = root / "predicate.json"
    base = [dive, "check", module, "--config", cfg, "--strategy", "symbolic", "--output", "json"]

    def verify(status, sign=True):
        return checked([*base, "--verify-predicate", predicate], status, key if sign else None)

    exact = checked([*base, "--write-predicate", predicate], 0, key)
    assert exact["encoding"] == "whole-values" and exact["verdict"] == "success"
    initial = predicate.read_text()
    assert verify(0)["authorized_closure"] and not verify(0, False)["authorized_closure"]
    cfg.write_text(doc.replace("TypeInvariant Bounded", "TypeInvariant"))
    assert verify(0)["authorized_closure"]
    cfg.write_text(doc)
    assert verify(0)["authorized_closure"]
    source.write_text(imported.replace('"a"', '"b"'))
    assert verify(2)["failed_obligation"] == "Next closure"
    source.write_text(imported)
    module.write_text(ORIGINAL.replace("members |-> StepValue", 'members |-> {"c"}'))
    assert verify(2)["failed_obligation"] == "Next closure"
    module.write_text(ORIGINAL.replace('{"a","b","c"}', '{"a","b","c","d"}'))
    assert verify(2)["failed_obligation"] == "whole-value encoding"
    module.write_text(ORIGINAL)
    cfg.write_text(doc.replace("DEADLOCK FALSE", "DEADLOCK TRUE"))
    assert verify(2)["failed_obligation"] == "deadlock"
    cfg.write_text(doc)
    data = json.loads(initial)
    predicate.write_text(json.dumps(dict(data, root=-1)))
    assert verify(2)["failed_obligation"] == "Init inclusion"
    predicate.write_text(json.dumps(dict(data, seal="0" * 64)))
    assert not verify(0)["authorized_closure"]
    predicate.write_text(json.dumps(dict(data, root=0)))
    assert verify(2)["verdict"] == "unsupported"
    assert checked([*base, "--verify-predicate", predicate, "--time-limit", "1ms"], 4, key)["verdict"] == "stopped"
    if sail:
        session = root / "session"
        session.mkdir()
        artifact = session / "sail-symbolic-predicate-v1.json"
        artifact.write_bytes(predicate.read_bytes())
        command = [sail, "runtime", "symbolic-check", "--dive", dive, "--module", module,
                   "--config", cfg, "--session", session, "--time-limit", "10"]
        assert checked(command, 0, key)["verdict"] == "success"
        assert checked(command, 0, key)["authorized_closure"]


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dive", type=Path)
    parser.add_argument("--sail", type=Path)
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        run(args.dive.resolve(), args.sail.resolve() if args.sail else None, Path(directory))
