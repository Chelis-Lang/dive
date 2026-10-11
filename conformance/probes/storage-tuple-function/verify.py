"""Read back scoped structural storage evidence without certifying all storage."""
from pathlib import Path
import gzip
import hashlib
import json

ROOT = Path(__file__).resolve().parents[3]
BASE = Path(__file__).resolve().parent
CURRENT = BASE / "current"

def load(p):
    return json.loads(p.read_text())

def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest()

receipt = load(CURRENT / "receipt.json")
for name, digest in receipt["files_sha256"].items():
    p = CURRENT / name
    assert p.resolve().is_relative_to(CURRENT.resolve()), name
    assert sha(p) == digest, name
producer = receipt["runtime_producer"]
assert producer["source_commit"] == "ead7260e48d69fc111bd8a94a60b526c6f68b1a2"
assert producer["binaries_sha256"]["dive"] == "4202c6ef537e6c5678e0d3b582eaee5752160eea2304a6489d0d975e52d2bbf4"
assert producer["binaries_sha256"]["dive-conformance"] == "66ee04e6531142f4074555635521f07cbad18e6921b375dffdafc75b21167c48"
source = load(CURRENT / "gate/source/index.json")
assert len(source["inputs"]) == 445
assert source["producer_commit"] == producer["source_commit"]
for name, row in source["inputs"].items():
    archive = CURRENT / "gate/source" / row["archive"]
    assert archive.resolve().is_relative_to((CURRENT / "gate/source").resolve()), name
    raw = gzip.decompress(archive.read_bytes())
    assert hashlib.sha256(raw).hexdigest() == row["sha256"], name

# The closed runtime keeps its original producer and every original source hash.
# Receiving changes need their own source integrity; they are not reruns of that
# producer. Fresh receiving gate/runtime receipts qualify the composed code.
authority = load(BASE / "current-source.json")
assert authority["schema_version"] == 1 and authority["family_certified"] is False
assert authority["runtime_producer"] == producer["source_commit"]
assert authority["source_index_sha256"] == sha(CURRENT / "gate/source/index.json")
assert set(authority["current_sha256"]) == set(source["inputs"])
for name, expected in authority["current_sha256"].items():
    assert (ROOT / name).resolve().is_relative_to(ROOT.resolve()), name
    assert sha(ROOT / name) == expected, name
assert authority["current_rust_cargo_equals_runtime"] == all(
    authority["current_sha256"][name] == row["sha256"]
    for name, row in source["inputs"].items()
)

gate = load(CURRENT / "gate/index.json")
assert len(gate["stages"]) == 8
assert all(c["exit"] == 0 for s in gate["stages"] for c in s["commands"])
assert "1167 tests run: 1167 passed, 0 skipped" in (CURRENT / "gate/nextest-0.log").read_text()
old = BASE / "before-current-format-fixtures-gate"
failed = load(old / "index.json")
assert failed["producer_commit"] == "dc0f97d424a99d669c040062d7fd197eb6d4ebf0"
assert [(s["name"], c["exit"]) for s in failed["stages"] for c in s["commands"] if c["exit"]] == [("nextest", 100)]
assert "1167 tests run: 1160 passed, 7 failed, 0 skipped" in (old / "nextest-0.log").read_text()
mutation = load(BASE / "current-header-mutation/receipt.json")
assert mutation["mutated_exit"] != 0 and mutation["restored_exit"] == 0
assert "coherent current-format corruption must reach payload admission" in (BASE / "current-header-mutation/mutation.log").read_text()

runtime = load(CURRENT / "runtime/receipt.json")
assert runtime["runtime_producer"] == producer
assert len(runtime["rows"]) == 192 and len(runtime["validation"]) == 768
assert sum(r["result"] == "valid" for r in runtime["validation"]) == 192
assert sum(r["result"] == "invalid" for r in runtime["validation"]) == 576
assert len(runtime["old_archives"]) == 12
for row in runtime["primary"]:
    assert row["jar_sha256"] == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
    assert row["exit"] == (75 if row["name"] == "Apply" else 12)
for row in runtime["rows"]:
    folder = CURRENT / "runtime" / row["case"] / row["mode"]
    for name, digest in row["files_sha256"].items():
        assert sha(folder / name) == digest
    for stage in ["check", "recover"]:
        doc = load(folder / (stage + ".stdout"))
        if row["case"].startswith("Apply-"):
            assert doc["tlc"]["code"] == 2103 and doc["tlc"]["reported"] == 1000
            assert doc["tlc"]["verified"] is False
            assert doc["message"].endswith('Attempted to apply function with integer domain to the non-integer argument "a"')
            assert [doc["statistics"][k] for k in ["generated", "distinct", "depth"]] == [1, 1, 1]
        else:
            assert doc["counterexample"]["trace"]["states"][0]["values"]["s"]["repr"] == "tuple-fcn"
            assert [doc["statistics"][k] for k in ["generated", "distinct", "depth"]] == [4, 4, 4]
        assert doc["collision"] is None
for row in runtime["old_archives"]:
    cp = BASE / "before" / f'{"tlc" if row["profile"] else "dive"}-{row["store"]}-w{row["workers"]}.cp'
    assert cp.read_bytes()[:8] == b"DIVECP01" and sha(cp) == row["archive_sha256"]
guides = load(CURRENT / "guides/receipt.json")
assert all(r["exit"] == 0 for r in guides["rows"])
assert {r["name"] for r in guides["rows"]} == {"rust-guide", "cm1", "validation-smoke"}
print("Verified1167/0skip full gate;192 check/pause/recover triples;192 Valid576 Invalid;12 actual old archives refused. Canonical keys/FP remain unchanged; no family/archive/total-memory certificate.")
