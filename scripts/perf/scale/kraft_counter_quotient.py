"""Build a parity quotient of the released Kafka KIP-996 pre-vote counter and a replay word."""

import argparse
import hashlib
import json
from pathlib import Path

from kraft_prevote import extend

EVIDENCE = Path(__file__).resolve().parents[3] / "conformance/reports/perf/2026-11-21-kafka-liveness.json"
COUNTER = "IF TestLiveness THEN pv + 1 ELSE pv"


def generate(source, sample, dest):
    pins = json.loads(EVIDENCE.read_text())["hashes"]
    inputs = {}
    for name, digest in pins.items():
        if not name.endswith(".tla") and name != "model_liveness.cfg":
            continue
        raw = (source / name).read_bytes()
        if hashlib.sha256(raw).hexdigest() != digest:
            raise ValueError(f"unexpected input for {name}")
        inputs[name] = raw
    name = "kraft_kip_996_functions.tla"
    if inputs[name].count(COUNTER.encode()) != 1:
        raise ValueError("expected one passive pre-vote increment")
    original = inputs.copy()
    inputs[name] = inputs[name].replace(COUNTER.encode(), b"IF TestLiveness THEN (pv + 1) % 2 ELSE pv")
    document = json.loads(sample.read_text())
    # Two traversals close the parity word even when one traversal toggles the bit.
    word, shape = extend(document.get("document", document), 2)
    for state in word["states"]:
        state["values"]["aux_ctrs"]["record"]["prevote_ctr"] %= 2
    loop = shape["start_index"]
    endpoint = word["states"].pop()
    if endpoint["values"] != word["states"][loop]["values"]:
        raise ValueError("projected segment must close on all state variables")
    word.update(ending={"kind": "lasso", "back_to": loop}, length=len(word["states"]))
    word.pop("properties", None)
    dest.mkdir()
    (dest / "concrete").mkdir()
    for name, raw in inputs.items():
        (dest / name).write_bytes(raw)
        (dest / "concrete" / name).write_bytes(original[name])
    (dest / "parity-word.json").write_text(json.dumps(word, separators=(",", ":")))
    return shape


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("released_modules", type=Path)
    p.add_argument("original_trace", type=Path)
    p.add_argument("new_directory", type=Path)
    args = p.parse_args()
    print(json.dumps(generate(args.released_modules, args.original_trace, args.new_directory)))
