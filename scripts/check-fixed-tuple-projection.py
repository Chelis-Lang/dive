"""Retain actual projection baselines, capture failures and the unsound mutation."""

import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/fixed-tuple-projection-replay/preimplementation"
index = json.loads((BASE / "index.json").read_text())
assert not index["family_certified"]
assert index["native_baseline_producer"] == "f8b3b51521ba166dd5ce5f8031a8bfb7185acefb"
assert index["native_baseline_consumers"] == 12
for rel, digest in index["files_sha256"].items():
    assert hashlib.sha256((BASE / rel).read_bytes()).hexdigest() == digest, rel
read = lambda rel: gzip.decompress((BASE / (rel + ".gz")).read_bytes())
for folder in ["unchanged-alias", "conditional-assigned"]:
    closure = json.loads(read(folder + "/closure.json"))
    assert closure["all_consumers_closed"] and closure["consumer_count"] == 6
    assert closure["producer"]["source_commit"] == index["native_baseline_producer"]
    rows = json.loads(read(folder + "/rows.json"))["rows"]
    assert len(rows) == 6
    for row in rows:
        data = read(f"{folder}/{row['case']}/{row['label']}.stdout")
        assert hashlib.sha256(data).hexdigest() == row["stdout_sha256"]
        if row["label"].startswith("tlc-"):
            assert json.loads(data)["result"] == "unsupported" and row["exit"] == 3
        elif row["label"].startswith("dive-"):
            expected = "valid" if row["label"].endswith("original") else "invalid"
            assert json.loads(data)["result"] == expected
mutation = json.loads(read("drivers-and-controls/dive-fixed-tuple-projection-mutation-receipt.json"))
assert mutation["exit"] == 101 and mutation["actual"] == "Valid" and mutation["required"] == "Unsupported"
assert mutation["original_restored"]
assert b"left: Valid" in read("drivers-and-controls/dive-fixed-tuple-projection-unsound-permutation-mutant.log")
assert b"17 passed; 2 failed" in read("fixture-spacing-failure/focused.log")
assert b"20 passed; 0 failed" in read("depth-control-clippy-failure/passed20-tests.log")
print("verified original12 projection baselines, genuine false-proof mutation, retained development failures; no new runtime/family certificate")
