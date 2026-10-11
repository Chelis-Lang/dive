"""Verify scoped v2 transport evidence without certifying arbitrary stored shapes."""

import copy
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/trace-representation-v2"
CURRENT = BASE / "current"


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def observables(doc, transport=False, cross=False):
    result = copy.deepcopy(doc)
    result.pop("elapsed_ms")
    result["options"].pop("workers")
    if cross:
        result["options"].pop("store")
        result["options"].pop("fingerprint")
    if transport:
        result["counterexample"].pop("trace")
        result["counterexample"].pop("trace_encoding", None)
        result["options"].pop("trace_format", None)
    return result


receipt = load(CURRENT / "receipt.json")
for name, expected in receipt["files_sha256"].items():
    assert sha(CURRENT / name) == expected, name
producer = receipt["runtime_producer"]
assert producer["source_commit"] == "e6c56158620d986c7acb9580b3c15f892d4de270"
assert producer["binaries_sha256"]["dive"] == "1681de6f282ff18b09216bf8bdcdab3c68f97ff8a43aef0486d182b05e012288"
assert producer["binaries_sha256"]["dive-conformance"] == "8e4aa766739e1d59aca4667f2525b04e53ab8b2a945aba6206ef87f059dcf9db"
source = load(CURRENT / "source/index.json")
assert source["producer_commit"] == producer["source_commit"]
assert len(source["inputs"]) == 443
for name, row in source["inputs"].items():
    archive = ROOT / row["archive"]
    assert archive.resolve().is_relative_to(ROOT.resolve()), name
    assert hashlib.sha256(gzip.decompress(archive.read_bytes())).hexdigest() == row["sha256"], name
authority = load(BASE / "current-source.json")
assert authority["runtime_producer"] == producer["source_commit"]
assert set(authority["current_sha256"]) == set(source["inputs"])
for name, expected in authority["current_sha256"].items():
    assert sha(ROOT / name) == expected, name
assert authority["current_rust_cargo_equals_runtime"] == all(
    authority["current_sha256"][name] == row["sha256"]
    for name, row in source["inputs"].items()
)

gate = load(CURRENT / "gate/index.json")
assert gate["producer_commit"] == producer["source_commit"]
assert len(gate["stages"]) == 8
assert all(c["exit"] == 0 for stage in gate["stages"] for c in stage["commands"])
assert "1159 tests run: 1159 passed, 0 skipped" in (CURRENT / "gate/nextest-0.log").read_text()
assert sha(CURRENT / "gate/original-source-index.json") == producer["source_index_sha256"]
for old, count in [("before-style-gate", 1156), ("before-decoder-admission-gate", 1157)]:
    history = load(BASE / old / "index.json")
    assert f"{count} tests run: {count} passed, 0 skipped" in (BASE / old / "nextest-0.log").read_text()
    failures = [(s["name"], c["exit"]) for s in history["stages"] for c in s["commands"] if c["exit"] != 0]
    assert failures == ([("clippy", 101)] if old == "before-style-gate" else [])
raw_counterexample = (BASE / "before-decoder-admission-gate/nested-raw-counterexample/stdout").read_text()
assert raw_counterexample.count("encoded Ok(") == 2
assert raw_counterexample.count('decoded Err("stored representation disagrees with checked constructor")') == 2

primary = load(BASE / "primary/index.json")
assert primary["jar_sha256"] == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
assert len(primary["rows"]) == 3
for row in primary["rows"]:
    fixture = BASE / "specs" / row["case"]
    assert row["exit"] == 12
    assert sha(fixture / "Transport.tla") == row["spec_sha256"]
    assert sha(fixture / "Transport.cfg") == row["config_sha256"]

runtime = load(CURRENT / "runtime/receipt.json")
assert runtime["runtime_producer"] == producer
assert len(runtime["rows"]) == 72 and len(runtime["validation"]) == 288
assert sum(r["kind"] == "original" and r["result"] == "valid" for r in runtime["validation"]) == 72
assert sum(r["kind"] != "original" and r["result"] == "invalid" for r in runtime["validation"]) == 216
assert runtime["precise_equality_allowlist"] == receipt["precise_equality_allowlist"] == {
    "within_store": ["elapsed_ms", "options.workers"],
    "cross_store_extra": ["options.store", "options.fingerprint"],
    "transport_only_extra": ["counterexample.trace", "counterexample.trace_encoding", "options.trace_format"],
}
baselines = {}
legacy_refusals = 0
for row in runtime["rows"]:
    folder = CURRENT / "runtime" / row["case"]
    doc = load(folder / "v2.stdout")
    legacy = load(folder / "v1.stdout")
    profile = row["profile"]
    assert row["exit"] == row["legacy_exit"] == (12 if profile == "tlc" else 10)
    assert doc["verdict"] == "invariant-violation" and doc["property"] == "Inv"
    assert tuple(doc["statistics"][name] for name in ["generated", "distinct", "depth"]) == (4, 4, 4)
    assert doc["options"]["workers"] == {
        "requested": row["workers"], "used": row["workers"], "sequential": False,
        "reason": None, "chaos": row["chaos"],
    }
    assert doc["options"]["store"] == ("fingerprint" if row["store"] == "fp" else "exact")
    assert doc["options"]["fingerprint"] == ({
        "polynomial_index": 0, "polynomial_source": "flag",
        "scheme": "tlc" if profile == "tlc" else "dive",
    } if row["store"] == "fp" else None)
    assert doc["collision"] is None and doc["output"] == []
    assert doc["counterexample"]["trace"]["version"] == 2
    assert doc["counterexample"]["trace_encoding"]["semantic_validation"] is False
    assert doc["counterexample"]["trace_encoding"]["status"] == "encoded"
    name = row["case"].split("-")[0]
    for key, cross in [((name, profile, row["store"]), False), ((name, profile, "cross"), True)]:
        value = observables(doc, cross=cross)
        if key in baselines:
            assert value == baselines[key], row["case"]
        else:
            baselines[key] = value
    assert legacy["counterexample"]["trace"]["version"] == 1
    assert "trace_encoding" not in legacy["counterexample"]
    assert observables(legacy, transport=True) == observables(doc, transport=True), row["case"]
    for kind in ["original", "mutant-init", "mutant-middle", "mutant-final"]:
        report = load(folder / (kind + ".stdout"))
        assert report["result"] == ("valid" if kind == "original" else "invalid")
    if name == "Mixed":
        report = load(folder / "legacy-shape.stdout")
        assert report["result"] == "invalid" and report["failure"]["check"] == "shape"
        legacy_refusals += 1
assert legacy_refusals == 24
unsupported = load(CURRENT / "unsupported/receipt.json")
assert unsupported["runtime_producer"] == producer and len(unsupported["rows"]) == 12
for row in unsupported["rows"]:
    doc = load(CURRENT / "unsupported" / (row["case"] + ".stdout"))
    assert doc["verdict"] == "invariant-violation"
    assert doc["counterexample"]["trace"] is None
    assert doc["counterexample"]["trace_encoding"]["status"] == "unsupported"
    assert doc["counterexample"]["trace_encoding"]["semantic_validation"] is False
    assert tuple(doc["statistics"][name] for name in ["generated", "distinct", "depth"]) == (4, 4, 4)
guides = load(CURRENT / "guides/receipt.json")
assert guides["runtime_producer"] == producer
assert [row["name"] for row in guides["rows"]] == ["rust-guide", "cm1", "validation-smoke"]
assert all(row["exit"] == 0 for row in guides["rows"])
assert "21 of 21" in (CURRENT / "guides/cm1.stdout").read_text()
assert "16 of 16" in (CURRENT / "guides/validation-smoke.stdout").read_text()
print("Verified1159/0skip gate;72v2+72legacy;72Valid216semanticInvalid;24legacyshapeInvalid and12encodingUnsupported remain explicit; guides21/16 at1/4/8. No arbitrary-shape or family certificate.")
