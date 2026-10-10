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
        for source, expected in json.loads(matrix.read_text()).get("source_sha256", json.loads(matrix.read_text()).get("checked_sources", {})).items():
            assert row["source_sha256"][source] == expected, source
    for name, digest in row["source_sha256"].items():
        assert sha(gzip.decompress((directory / "sources" / (name + ".gz")).read_bytes())) == digest, name
current = json.loads((BASE / "current-source.json").read_text())
assert current["schema_version"] == 1 and current["family_certified"] is False
assert current["historical_slices_sha256"] == sha((BASE / "historical-slices.json").read_bytes())
assert current["source_sha256"]
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
