"""Exercise guarded and undefined source reads in the actual bound original initial state."""

import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import z3

from check import config, simplified
from source import Model
from values import dis, require

PROBES = {
    "Guarded": (
        r"\A j \in Server : IF Len(log[j]) = 0 THEN TRUE ELSE log[j][1] = 0",
        True,
    ),
    "Projection": (r"\A j \in Server : j \in DOMAIN log /\ Len(log[j]) = 0", True),
    "InvalidServer": (r"log[Nil] = <<>>", False),
    "InvalidIndex": (r"\A j \in Server : log[j][1] = 0", False),
    "InvalidChoice": (r"(CHOOSE j \in Server : FALSE) \in Server", False),
    "AmbiguousChoice": (r"(CHOOSE j \in Server : TRUE) \in Server", False),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--exporter", type=Path, required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        shutil.copy(args.source / "AsyncRaft.tla", root / "AsyncRaft.tla")
        trial = root / "Definedness.tla"
        trial.write_text(
            "---- MODULE Definedness ----\nEXTENDS AsyncRaft\n"
            + "".join(f"{name} == {formula}\n" for name, (formula, _) in PROBES.items())
            + "====\n"
        )
        cfg = trial.with_suffix(".cfg")
        cfg.write_text(config(args.source / "AsyncRaft.cfg", 2))
        run = subprocess.run(
            [args.exporter.resolve(), trial, cfg], capture_output=True, check=True
        )
        doc = json.loads(run.stdout)
        model = Model(doc)
        ((guard, state),) = model.branches(doc["init"], {})
        require(guard is True, "unexpected Init")
        model.state = state
        for name, (_, expected_defined) in PROBES.items():
            (ref,) = [d["body"] for d in model.defs if d["name"] == name]
            model.errors = []
            value = model.eval(ref, {})
            undefined = simplified(dis(*model.errors))
            require(
                z3.is_false(undefined) if expected_defined else z3.is_true(undefined),
                f"wrong definedness for {name}",
            )
            if expected_defined:
                require(
                    z3.is_true(simplified(value)),
                    f"expected true in initial state: {name}",
                )
        print("six source-definedness controls passed")


if __name__ == "__main__":
    main()
