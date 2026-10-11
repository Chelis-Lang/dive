"""Verify immutable slice source archives and distinct current integration bytes."""
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BASE = ROOT / "conformance/probes/native-parity-integration"
def sha(data):
    return hashlib.sha256(data).hexdigest()

history = json.loads((BASE / "historical-slices.json").read_text())
assert history["schema_version"] == 1
slices = history["slices"]
assert len(slices) == len({row["slice"] for row in slices})
assert {"graph", "conditional", "checkpoint", "fairness", "public-ir", "multiple-processes"} <= {row["slice"] for row in slices}
for row in slices:
    directory = BASE / "history" / row["slice"]
    assert json.loads((directory / "record.json").read_text()) == row
    assert len(row["source_commit"]) == 40 and len(row["source_tree"]) == 40
    assert row["source_sha256"]
    for name, digest in row["matrices_sha256"].items():
        matrix = BASE / name
        assert sha(matrix.read_bytes()) == digest, name
        document = json.loads(matrix.read_text())
        field = row.get("matrix_source_fields", {}).get(name)
        sources = document[field] if field else document.get("source_sha256", document.get("checked_sources", {}))
        assert isinstance(sources, dict), (name, field)
        for source, expected in sources.items():
            assert row["source_sha256"][source] == expected, source
    for name, digest in row["source_sha256"].items():
        assert sha(gzip.decompress((directory / "sources" / (name + ".gz")).read_bytes())) == digest, name
current = json.loads((BASE / "current-source.json").read_text())
assert current["schema_version"] == 1 and current["family_certified"] is False
assert current["historical_slices_sha256"] == sha((BASE / "historical-slices.json").read_bytes())
assert current["source_sha256"]
# Generated integrity outputs are hashed separately, never recursively as their own source.
assert "conformance/probes/native-parity-integration/current-source.json" not in current["source_sha256"]
for name, digest in current.get("manifest_sha256", {}).items():
    assert sha((ROOT / name).read_bytes()) == digest, name
for name, digest in current["source_sha256"].items():
    assert sha((ROOT / name).read_bytes()) == digest, name
print(f"verified {len(slices)} immutable slice source histories and distinct combined source hashes; no family certification")

# These runs belong to the frozen combined runtime source. Later test-only and
# evidence bookkeeping commits do not rewrite their recorded commit/tree IDs.
controls = BASE / "current-controls"
delivery = json.loads((controls / "delivery.json").read_text())
assert delivery["family_certified"] is False
for name, digest in delivery["evidence_sha256"].items():
    assert sha((controls / name).read_bytes()) == digest, name
runtime_history = next(row for row in slices if row["slice"] == "combined-control-runtime")
assert runtime_history["source_commit"] == delivery["runtime_source_commit"]
assert runtime_history["source_tree"] == delivery["runtime_source_tree"]
assert runtime_history["source_sha256"] == delivery["runtime_source_sha256"]
def frozen_runtime(name):
    return gzip.decompress((BASE / "history/combined-control-runtime/sources" / (name + ".gz")).read_bytes())
for name, digest in delivery["runtime_source_sha256"].items():
    assert sha(frozen_runtime(name)) == digest, name

def raw_streams(value, directory):
    if isinstance(value, dict):
        for stream in ("stdout", "stderr"):
            if stream + "_sha256" in value:
                path = directory / value[stream]
                data = path.read_bytes()
                if path.suffix == ".gz":
                    data = gzip.decompress(data)
                assert sha(data) == value[stream + "_sha256"], path
        for child in value.values():
            raw_streams(child, directory)
    elif isinstance(value, list):
        for child in value:
            raw_streams(child, directory)

for family, subdirectories, expected in (
    ("fairness", ("main", "positive", "interactions", "sequential", "sequential-positive", "combined"), (404, 250, 16)),
    ("multiple", ("main", "extra", "bounds"), (264, 206, 60)),
):
    meta = json.loads((controls / family / "execution.json").read_text())
    assert meta["source_commit"] == delivery["runtime_source_commit"]
    assert meta["source_tree"] == delivery["runtime_source_tree"]
    assert meta["binary_sha256"] == delivery["binary_sha256"]
    assert meta["example_sha256"] == delivery["example_sha256"]
    for name, digest in meta["source_sha256"].items():
        assert sha(frozen_runtime(name)) == digest, name
    selected = artifacts = models = 0
    for sub in subdirectories:
        directory = controls / family / sub
        comparison_path = directory / "comparison.json"
        comparison = json.loads(comparison_path.read_text())
        assert sum(comparison["summary"].values()) == comparison["selected"]
        assert len(comparison["rows"]) == comparison["selected"]
        assert len({row["id"] for row in comparison["rows"]}) == comparison["selected"]
        assert not comparison["summary"].get("disagree")
        selected += comparison["selected"]
        report_path = directory / "validation/report.json"
        report = json.loads(report_path.read_text())
        assert report["selection_report_sha256"] == sha(comparison_path.read_bytes())
        assert report["family_certified"] is False
        assert report["native_sha256"] == delivery["binary_sha256"]
        assert report["cst_dumper_sha256"] == delivery["example_sha256"]
        assert len(report["artifact_rows"]) == report["selected_artifacts"]
        assert report["selected_artifacts"] == comparison["summary"].get("token-artifact-match", 0)
        for row in report["artifact_rows"]:
            assert row["outcome"] == "native-sany-match" and row["operator_structure_equal"]
            assert sha((report_path.parent / row["artifact"]).read_bytes()) == row["artifact_sha256"]
        for row in report["model_rows"]:
            assert row["outcome"] == "finite-model-match"
            for key in ("source", "config"):
                assert sha((report_path.parent / row[key]).read_bytes()) == row[key + "_sha256"]
            assert {(v["workers"], v["store"]) for v in row["workers_stores"]} == {
                (w, s) for w in (1, 4, 8) for s in ("fp", "exact")
            }
            assert all(v["outcome"] == "match" for v in row["workers_stores"])
            assert all(v["passed"] for v in row["independent_validation"])
        raw_streams(report, report_path.parent)
        artifacts += len(report["artifact_rows"])
        models += len(report["model_rows"])
    assert (selected, artifacts, models) == expected

mutants = json.loads((controls / "multiple/document-corruption/mutation-controls.json").read_text())
assert mutants["control_count"] == 28 and len(mutants["rows"]) == 28
assert all(row["outcome"]["outcome"] == "disagree" for row in mutants["rows"])
checkpoint = json.loads((controls / "checkpoint/index.json").read_text())
assert len(checkpoint["controls"]) == 576 and len(checkpoint["archive_sha256"]) == 144
assert len({tuple(row[k] for k in ("case", "profile", "store", "width", "chaos", "stage")) for row in checkpoint["controls"]}) == 576
assert checkpoint["source_commit"] == delivery["runtime_source_commit"]
assert checkpoint["source_tree"] == delivery["runtime_source_tree"]
assert checkpoint["native_binary_sha256"] == delivery["binary_sha256"]
raw_streams(checkpoint["controls"], controls / "checkpoint")
for name, digest in checkpoint["archive_sha256"].items():
    assert sha((controls / "checkpoint" / name).read_bytes()) == digest, name
print("verified retained combined-runtime 576 recovery runs, 668 translation cases, 456 full-CST/SANY artifacts, 76 finite models and 28 document mutants")

# These closed observations retain their original producer, independent of the
# current integrity maps and later feature integrations.
frozen = controls / "core-options-random-frozen"
receipt = json.loads((frozen / "receipt.json").read_text())
gate = json.loads((frozen / "gate/verification.json").read_text())
producer = json.loads((frozen / "producer.json").read_text())
assert receipt["source_commit"] == gate["source_commit"] == producer["source_commit"] == "550d959e4223e1b6fce47ffd29d039d8dfef016c"
assert receipt["source_tree"] == gate["source_tree"] == producer["source_tree"]
assert gate["tests_passed"] == receipt["tests_passed"] == 1076
assert gate["tests_skipped"] == receipt["tests_skipped"] == 0
assert gate["gate_stages_passed"] == 8 and gate["source_guards_passed"] == 24
assert sum(gate["inventory"]["hash_partitions"]) == 1076
assert gate["inventory"]["disjoint_and_complete"] and gate["inventory"]["ignored_tests"] == 0
assert receipt["family_certified"] is False and gate["family_certified"] is False
for name, digest in gate["source_sha256"].items():
    assert sha(gzip.decompress((frozen / "gate/source" / (name + ".gz")).read_bytes())) == digest, name
for name, digest in gate["logs_sha256"].items():
    assert sha((frozen / "gate/logs" / name).read_bytes()) == digest, name
for name, digest in receipt["compressed_artifact_sha256"].items():
    assert sha((frozen / name).read_bytes()) == digest, name
for name, digest in receipt["raw_artifact_sha256"].items():
    assert sha(gzip.decompress((frozen / "runtime" / (name + ".gz")).read_bytes())) == digest, name
process_options = json.loads(gzip.decompress((frozen / "runtime/process-options/full-controls/report.json.gz").read_bytes()))
assert process_options["selected"] == len(process_options["rows"]) == 524
assert process_options["native_sha256"] == producer["binary_sha256"]["dive"]
assert process_options["cst_sha256"] == producer["binary_sha256"]["cst"]
for profile in ("dive", "tlc"):
    assert process_options["summary"][profile] == {"full-derivation-and-admission": 320, "named-cli-option-rejection": 28, "rejected": 128, "named-refusal": 48}
    for row in process_options["rows"]:
        result = row[profile]
        if result["outcome"] == "full-derivation-and-admission":
            assert result["operator_structure_equal"] and result["sany"]["exit_code"] == result["validator"]["exit_code"] == 0, row["id"]
print("Verified frozen550 complete1076 zero-skip gate and524 explicit-option cases;640 complete CST/SANY/Validator pairs; no family certification")

# Always verify the portable LOCAL/function DEFINE source and raw runtime bytes.
# This receipt is historical and is never inferred from current integration hashes.
import subprocess
import sys
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/define-forms-acceptance" / verifier)], check=True)

# Always verify every portable sequential-procedure checksum runtime/source record.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/procedure-checksum-acceptance" / verifier)], check=True)

# Always verify portable nested procedure-header source and raw runtime records.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/nested-procedure-checksum-acceptance" / verifier)], check=True)

# Mandatory portable P elsif source/runtime authority, distinct from live generation.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/p-elsif-checksum-acceptance" / verifier)], check=True)


# New bounded procedure ownership has its own complete clean primary authority;
# this readback grants no runtime or family certification.
subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/multiple-procedure-checksum-acceptance/verify.py")], check=True)

# Disjoint sequential-procedure checksum observations retain their frozen producer.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/multiple-procedure-checksum-acceptance" / verifier)], check=True)

# The complete acyclic checksum capture preserves source/error stages separately.
subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/acyclic-procedure-checksum-acceptance/verify.py")], check=True)

# Acyclic procedure observations keep frozen source/runtime and repaired command epochs.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/acyclic-procedure-checksum-acceptance" / verifier)], check=True)

# Recursive checksum derivation keeps selected source, closed runtime and failure epochs portable.
for verifier in ("verify.py", "verify-delivery.py"):
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/recursive-procedure-checksum-acceptance" / verifier)], check=True)

# Bounded finite-process procedure header reference epochs remain separate.
subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/process-procedure-checksum-acceptance/verify.py")], check=True)
for verifier in ["verify-delivery.py", "test-delivery.py"]:
    subprocess.run([sys.executable, "-I", str(ROOT / "crates/dive-pluscal/process-procedure-checksum-acceptance" / verifier)], check=True)
