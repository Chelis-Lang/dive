"""Verify captured primary/native tuple-history observations without executing them."""

import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/inductive-tuple-replay"
EXPECTED = {
    "tail", "wrapper", "alias", "conditional", "concat", "subseq",
    "unchanged_branch", "let", "off_path_string", "off_path_constructor",
    "string_init", "unconstrained_next",
}
for epoch in ("original-malformed-primary", "corrected-driver-failure", "corrected-closed"):
    folder = BASE / epoch
    index = json.loads((folder / "index.json").read_text())
    assert not index["implementation_changed"] and not index["family_certified"]
    for name, expected in index["files_sha256"].items():
        assert hashlib.sha256((folder / name).read_bytes()).hexdigest() == expected, name

folder = BASE / "corrected-closed"
read = lambda name: gzip.decompress((folder / (name + ".gz")).read_bytes())
closure = json.loads(read("closure.json"))
rows = json.loads(read("rows.json"))
assert closure["all_consumers_closed"] and not closure["implementation_changed"]
assert closure["producer"] == rows["native_producer"]
assert closure["producer"]["source_commit"] == "7577f187ff475d74f9ac36d51494688ece3b34e2"
assert len(rows["rows"]) == closure["consumer_count"] == 68
assert closure["selected_cases"] == len(EXPECTED) == 12
assert {row["case"] for row in rows["rows"]} == EXPECTED
assert len({(row["case"], row["label"]) for row in rows["rows"]}) == 68
for row in rows["rows"]:
    streams = {}
    for stream in ("stdout", "stderr"):
        data = read(f"{row['case']}/{row['label']}.{stream}")
        assert hashlib.sha256(data).hexdigest() == row[stream + "_sha256"]
        streams[stream] = data
    if row["label"] == "primary":
        incomplete = row["case"] == "unconstrained_next"
        assert row["exit"] == (75 if incomplete else 12)
        assert (b"@!@!@STARTMSG 2109:1" if incomplete else b"@!@!@STARTMSG 2110:1") in streams["stdout"]
    elif row["label"] == "native-check":
        doc = json.loads(streams["stdout"])
        if row["case"] == "unconstrained_next":
            assert row["exit"] == 75 and doc["verdict"] == "incomplete-state"
            assert doc["counterexample"]["trace"] is None
        else:
            assert row["exit"] == 12 and doc["verdict"] == "invariant-violation"
            assert doc["property"] == "Inv"
    else:
        doc = json.loads(streams["stdout"])
        if row["label"].startswith("tlc-"):
            assert doc["result"] == "unsupported" and row["exit"] == 3
        else:
            assert doc["result"] == ("valid" if row["label"].endswith("original") else "invalid")
            assert row["exit"] == (0 if row["label"].endswith("original") else 1)
print("verified68 preimplementation consumers,12 pinned primary cases and retained capture failures; no current support or family certificate")

# Preserve the first actual implementation qualification gap independently
# of later fixes: the native configured-constant word was refused.
epoch = BASE / "first-implementation-constant-gap"
index = json.loads((epoch / "index.json").read_text())
assert index["source_commit"] == "19deb55bfa94685e273cab286c20c6f5f0674d28"
assert index["all_consumers_closed"] and not index["family_certified"]
for name, expected in index["files_sha256"].items():
    assert hashlib.sha256((epoch / name).read_bytes()).hexdigest() == expected
read_gap = lambda name: gzip.decompress((epoch / (name + ".gz")).read_bytes())
gap_steps = json.loads(read_gap("binary-production/steps.json"))
assert gap_steps[-1]["label"] == "fresh-primary-and-words" and gap_steps[-1]["exit"] == 1
gap_result = json.loads(read_gap("first-runtime/alias/tlc-original.stdout"))
assert gap_result["result"] == "unsupported"
gap_gate = json.loads(read_gap("gate-steps.json"))
assert len(gap_gate) == 8 and all(row["exit"] == 0 for row in gap_gate)
assert b"1214 tests run: 1214 passed, 0 skipped" in read_gap("gate/2.log")
print("retained actual configured-native qualification failure and original1214 full-gate epoch")
