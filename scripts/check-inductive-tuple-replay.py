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

# The corrected frozen implementation has separate source authority from this
# publication. Compare archived bytes, never retag old captures as the live tree.
epoch = BASE / "implemented-epoch"
index = json.loads((epoch / "index.json").read_text())
assert index["producer"] == "f8b3b51521ba166dd5ce5f8031a8bfb7185acefb"
assert index["all_consumers_closed"] and not index["family_certified"]
for rel, digest in index["files_sha256"].items():
    assert hashlib.sha256((epoch / rel).read_bytes()).hexdigest() == digest, rel
read_current = lambda rel: gzip.decompress((epoch / (rel + ".gz")).read_bytes())
source = json.loads(read_current("source-index.json"))
assert source["source_commit"] == index["producer"]
for rel, digest in index["portable_compile_source_sha256"].items():
    assert source["source_sha256"][rel] == digest
    assert hashlib.sha256(read_current("source/" + rel)).hexdigest() == digest
steps = json.loads(read_current("gate/steps.json"))
assert len(steps) == 8 and all(row["exit"] == 0 for row in steps)
for i, row in enumerate(steps):
    assert hashlib.sha256(read_current(f"gate/{i}.log")).hexdigest() == row["log_sha256"]
assert b"1216 tests run: 1216 passed, 0 skipped" in read_current("gate/2.log")
offline = json.loads(read_current("offline/steps.json"))["steps"]
assert len(offline) == 43 and all(row["exit"] == 0 for row in offline)
for i, row in enumerate(offline):
    assert hashlib.sha256(read_current(f"offline/{i}.log")).hexdigest() == row["sha256"]
rows = json.loads(read_current("runtime/rows.json"))
closure = json.loads(read_current("runtime/closure.json"))
assert closure["all_consumers_closed"] and closure["consumer_count"] == 68
assert rows["native_producer"] == closure["producer"]
assert closure["producer"]["source_commit"] == index["producer"]
assert {row["case"] for row in rows["rows"]} == EXPECTED
assert len({(row["case"], row["label"]) for row in rows["rows"]}) == 68
negative = {"string_init", "off_path_string", "off_path_constructor"}
for row in rows["rows"]:
    stream = read_current(f"runtime/{row['case']}/{row['label']}.stdout")
    assert hashlib.sha256(stream).hexdigest() == row["stdout_sha256"]
    error = read_current(f"runtime/{row['case']}/{row['label']}.stderr")
    assert hashlib.sha256(error).hexdigest() == row["stderr_sha256"]
    if row["label"] not in {"primary", "native-check"}:
        result = json.loads(stream)
        expected = "unsupported" if row["case"] in negative and row["label"].startswith("tlc-") else ("valid" if row["label"].endswith("original") else "invalid")
        assert result["result"] == expected
        assert row["exit"] == {"unsupported": 3, "valid": 0, "invalid": 1}[expected]
widths = json.loads(read_current("width-controls/steps.json"))
closure = json.loads(read_current("width-controls/closure.json"))
assert closure["all_consumers_closed"] and closure["model_checks"] == 288 and closure["replay_checks"] == 528
assert widths["producer"] == closure["producer"] and widths["producer"]["source_commit"] == index["producer"]
assert len(widths["rows"]) == 816 and len({row["label"] for row in widths["rows"]}) == 816
for row in widths["rows"]:
    case = next(case for case in EXPECTED if row["label"].startswith(case + "-"))
    raw = read_current(f"width-controls/{case}/{row['label']}.stdout")
    assert hashlib.sha256(raw).hexdigest() == row["stdout_sha256"]
    doc = json.loads(raw)
    if row["label"].endswith("-check"):
        if case == "unconstrained_next":
            assert doc["verdict"] == "incomplete-state" and doc["counterexample"]["trace"] is None and doc["tlc"]["code"] == 2109
        else:
            assert doc["verdict"] == "invariant-violation" and doc["property"] == "Inv"
            assert all(doc["statistics"][key] == 2 for key in ["generated", "distinct", "depth"])
    else:
        strict = row["label"].startswith(case + "-tlc-")
        expected = "unsupported" if strict and case in negative else ("valid" if row["label"].endswith("-original") else "invalid")
        assert doc["result"] == expected
        assert row["exit"] == {"unsupported": 3, "valid": 0, "invalid": 1}[expected]
# A successful bounded variable proof does not certify general saved stacks.
for folder in ["unchanged-alias-gap", "projection-negative-assigned"]:
    closure = json.loads(read_current(folder + "/closure.json"))
    assert closure["all_consumers_closed"] and closure["consumer_count"] == 6
    assert closure["producer"]["source_commit"] == index["producer"]
    cases = json.loads(read_current(folder + "/rows.json"))["rows"]
    for row in cases:
        if row["label"].startswith("tlc-"):
            doc = json.loads(read_current(f"{folder}/{row['case']}/{row['label']}.stdout"))
            assert doc["result"] == "unsupported"
print("verified corrected 1216-test producer, 68 pinned cases, 816 worker/store consumers and preserved fixed-projection refusals; no general stack certificate")
