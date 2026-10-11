"""Keep pre-implementation characterization distinct from tuple replay support."""

import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/tuple-only-replay"


def archived(folder, name):
    return gzip.decompress((folder / (name + ".gz")).read_bytes())


for name in ("before-implementation-original", "before-implementation-corrected"):
    folder = BASE / name
    index = json.loads((folder / "index.json").read_text())
    assert index["all_consumers_closed"] and not index["family_certified"]
    for rel, expected in index["files_sha256"].items():
        assert hashlib.sha256((folder / rel).read_bytes()).hexdigest() == expected, rel
    rows = json.loads(archived(folder, "rows.json"))
    closure = json.loads(archived(folder, "closure.json"))
    assert closure["all_consumers_closed"] and not closure["implementation_changed"]
    assert closure["source_commit"] == rows["source_commit"] == index["runtime_source_commit"]
    assert closure["binary_sha256"] == rows["binary_sha256"]
    assert closure["rows"] == len(rows["rows"]) == 30
    assert len({(row["case"], row["label"]) for row in rows["rows"]}) == 30
    for row in rows["rows"]:
        case, label = row["case"], row["label"]
        streams = {}
        for stream in ("stdout", "stderr"):
            data = archived(folder, f"{case}/{label}.{stream}")
            assert hashlib.sha256(data).hexdigest() == row[stream + "_sha256"]
            streams[stream] = data
        if label == "primary":
            assert row["exit"] == 12
            assert b"@!@!@STARTMSG 2110:1" in streams["stdout"]
            assert b"@!@!@STARTMSG 2121:1" in streams["stdout"]
            assert b"2 states generated, 2 distinct states found" in streams["stdout"]
            assert b"depth of the complete state graph search is 2" in streams["stdout"]
        else:
            doc = json.loads(streams["stdout"])
            if label.startswith("tlc-"):
                assert doc["result"] == "unsupported" and "#124" in doc["unsupported"]
            elif name == "before-implementation-corrected" or case != "concat":
                assert doc["result"] == ("valid" if label == "dive-actual" else "invalid")
            else:
                # Preserve the original mislabeled word, never claim it as a match.
                assert doc["result"] == ("invalid" if label == "dive-actual" else "valid")

folder = BASE / "before-fixture-repair"
index = json.loads((folder / "index.json").read_text())
assert not index["family_certified"]
for rel, expected in index["files_sha256"].items():
    assert hashlib.sha256((folder / rel).read_bytes()).hexdigest() == expected
source = json.loads(archived(folder, "source.json"))
assert source["focused_tests"] == {"passed": 12, "failed": 1, "skipped": 0}
for rel, expected in source["source_sha256"].items():
    assert hashlib.sha256(archived(folder, "source/" + rel)).hexdigest() == expected
assert b"12 passed; 1 failed; 0 ignored" in archived(folder, "focused.log")
print("verified two complete 30-consumer historical epochs and retained malformed-fixture failure; no current support or family certification inferred")
