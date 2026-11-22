#!/usr/bin/env python3
"""Compare the published LuPastry+ function equation with two finite controls."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
from time import monotonic

from lupastry import routing_table


def run(command, expected):
    start = monotonic()
    completed = subprocess.run(command, text=True, capture_output=True, timeout=155)
    if completed.returncode != expected:
        raise RuntimeError((completed.returncode, completed.stdout, completed.stderr))
    return round(monotonic() - start, 6), json.loads(completed.stdout)


def prepare(archive, directory, bits, ready, next_op="NextMC", variant="published"):
    script = Path(__file__).with_name("lupastry.py")
    subprocess.run([sys.executable, script, archive, directory, "--bits", str(bits),
                    "--ready", *map(str, ready), "--next", next_op, "--definition", variant], check=True)
    return directory / "Model.cfg"


def check(dive, module, config, *options):
    return [dive, "check", module, "--config", config, "--store", "exact", "--workers", "1",
            "--seed", "0", "--fp", "0", "--time-limit", "150s", "--exit-codes", "native",
            "--output", "json", *options]


def model_controls(dive, archive, root):
    controls = []
    for name, bits, ready, next_op, expected in (
        ("next", 1, [0], "Next", (190, 1268, 17)),
        ("next_mc", 1, [0], "NextMC", (14, 40, 11)),
        ("three_ready", 2, [0, 1, 2], "NextMC", (71, 654, 18)),
    ):
        config = prepare(archive, root / name, bits, ready, next_op)
        source = config.with_name("Pastry.tla")
        command = check(dive, source, config)
        seconds, row = run(command, 0)
        stats = row["statistics"]
        assert row["verdict"] == "success" and tuple(stats[key] for key in ("distinct", "generated", "depth")) == expected
        _, nominal = run([*command, "--compat=tlc"], 3)
        assert nominal["verdict"] == "unsupported" and "unbounded quantifier or CHOOSE" in nominal["message"]
        controls.append({"case": name, "default": stats, "tlc_profile": nominal["message"], "wall_s": seconds,
                         "source_sha256": sha256(source.read_bytes()).hexdigest(), "cfg_sha256": sha256(config.read_bytes()).hexdigest()})
    config = root / "next_mc" / "Model.cfg"
    source = config.with_name("Replay.tla")
    source.write_text(r"""---- MODULE Replay ----
EXTENDS Pastry
NoTraffic == /\ SharedDigitsAux(0,0)[RTRowCount] = RTRowCount
             /\ MessagePool = {}
====
""")
    bad_cfg = config.with_name("Replay.cfg")
    bad_cfg.write_text(config.read_text().replace("INVARIANTS TypeInvariant CorrectDelivery", "INVARIANT NoTraffic"))
    _, row = run(check(dive, source, bad_cfg), 10)
    word = row["counterexample"]["trace"]
    assert row["property"] == "NoTraffic" and len(word["states"]) == 2
    trace = root / "word.json"
    trace.write_text(json.dumps(word))
    validate = [dive, "validate-trace", source, "--config", bad_cfg, "--trace", trace, "--output", "json"]
    assert run(validate, 0)[1]["result"] == "valid"
    word["states"][1]["values"]["MessagePool"] = word["states"][0]["values"]["MessagePool"]
    trace.write_text(json.dumps(word))
    assert run(validate, 1)[1]["result"] == "invalid"
    return controls, {"states": 2, "original_replay": "valid", "damaged_replay": "invalid"}


def compare_functions(dive, archive, root, bits, variant, compare):
    directory = root / f"digits_{bits}_{variant}"
    prepare(archive, directory, bits, [0], variant=variant)
    source = directory / "Digits.tla"
    begin = "---- MODULE Digits ----\nEXTENDS RoutingTable, FiniteSets\nVARIABLE probeMarker\n"
    if compare:
        raw = (directory / "RoutingTable.tla").read_bytes()
        for variant, name in (("bounded", "BoundedAux"), ("counted", "CountedAux")):
            transformed = routing_table(raw, variant)
            definition = transformed[transformed.index(b"SharedDigitsAux(i,j) =="):transformed.index(b"SharedDigits(i,j) ==")]
            begin += definition.decode().replace("SharedDigitsAux(i,j)", f"{name}(i,j)")
        test = r"ASSUME \A i,j \in I : (SharedDigitsAux(i,j) = BoundedAux(i,j)) /\ (SharedDigitsAux(i,j) = CountedAux(i,j))"
    else:
        test = r"""ASSUME \A i,j \in I :
    LET result == SharedDigitsAux(i,j) IN
    /\ DOMAIN result = RTRows
    /\ \A n \in RTRows : result[n] = Cardinality({k \in 1..n :
             IthDigit(i, RTRowCount-k+1) = IthDigit(j, RTRowCount-k+1)})
"""
    source.write_text(begin + test + "\nInit == probeMarker = 0\nNext == UNCHANGED probeMarker\n====\n")
    config = source.with_suffix(".cfg")
    config.write_text(f"INIT Init\nNEXT Next\nCONSTANTS A = {{0}}\n B = 1\n M = {bits}\n L = 1\n NullNode = NullNode\n")
    seconds, row = run(check(dive, source, config), 0)
    assert row["verdict"] == "success"
    return {"pairs": 4 ** bits, "wall_s": seconds}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("dive", type=Path)
    args = parser.parse_args()
    with TemporaryDirectory() as directory:
        root = Path(directory)
        models, mutation = model_controls(args.dive.resolve(), args.archive.resolve(), root)
        print(json.dumps({"models": models, "mutation": mutation}), flush=True)
        equality = [compare_functions(args.dive.resolve(), args.archive.resolve(), root, bits, "published", True)
                    for bits in (1, 2, 3, 4)]
        print(json.dumps({"whole_function_equality": equality}), flush=True)
        performance = {tag: compare_functions(args.dive.resolve(), args.archive.resolve(), root, 5, tag, False)
                       for tag in ("published", "bounded", "counted")}
        print(json.dumps({"32_addresses": performance}), flush=True)
