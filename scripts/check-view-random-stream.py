"""Verify narrow VIEW/random evidence without promoting refused shapes or words."""
import gzip
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "conformance/probes/view-random-stream/current"


def load(path):
    return json.loads(path.read_text())


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


receipt = load(BASE / "receipt.json")
for path, expected in receipt["files_sha256"].items():
    if sha(BASE / path) != expected:
        raise SystemExit(f"Current VIEW stream evidence changed: {path}")
producer = receipt["runtime_producer"]
assert producer["source_commit"] == "d749f9b06e7e727e4cb4e1b4231a12a5cddcd696"
assert producer["binary_sha256"] == "9c6b94b1c5d8a05f6af60b9626b969716f5c8fd43fb1d301c19d1e91744c46ea"
source = load(BASE / "source/index.json")
assert source["producer_commit"] == producer["source_commit"]
for path, row in source["inputs"].items():
    raw = gzip.decompress((BASE / "source" / row["archive"]).read_bytes())
    assert hashlib.sha256(raw).hexdigest() == row["sha256"], path

current = load(BASE.parent / "current-source.json")
for path, expected in current["current_sha256"].items():
    assert sha(ROOT / path) == expected, f"Current VIEW/random source authority changed: {path}"
actual_equal = all(current["current_sha256"][p] == row["sha256"] for p, row in source["inputs"].items())
assert current["current_rust_cargo_equals_runtime"] == actual_equal
gate = load(BASE / "gate/index.json")
assert len(gate["stages"]) == 8
assert all(c["exit"] == 0 for s in gate["stages"] for c in s["commands"])
assert "1144 tests run: 1144 passed, 0 skipped" in (BASE / "gate/nextest-0.log").read_text()
primary = load(BASE.parent / "primary/index.json")
assert len(primary["cases"]) == 140
runtime = load(BASE / "runtime-campaign/receipt.json")
assert len(runtime["rows"]) == 3360 and len(runtime["validation"]) == 960
assert sum(r["result"] == "valid" and r["kind"] == "original" for r in runtime["validation"]) == 240
assert sum(r["result"] == "invalid" and r["kind"] != "original" for r in runtime["validation"]) == 720
assert runtime["equality_allowlist"] == receipt["precise_equality_allowlist"]
first = load(BASE / "first-successor-native/receipt.json")
assert len(first["rows"]) == 240 and all(r["exit"] == 3 for r in first["rows"])
assert len(first["validation"]) == 80
assert sum(r["result"] == "valid" for r in first["validation"]) == 20
assert sum(r["result"] == "invalid" for r in first["validation"]) == 60
admission = load(BASE / "admission/receipt.json")
assert len(admission["rows"]) == 192 and all(r["exit"] == 3 for r in admission["rows"])
assert all(r["exit"] == 0 for r in load(BASE / "current-gates/receipt.json")["rows"])
print("Verified frozen1144/0-skip gate,140 clean rows,3360 native runs,240 Valid/720 Invalid words;240 partial-print refusals and192 no-effect refusals remain explicit; no family certification")
