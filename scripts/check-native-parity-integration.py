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
