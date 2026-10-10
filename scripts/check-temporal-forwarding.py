#!/usr/bin/env python3
"""Verify the complete retained temporal scope raw capture and historical map."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROBES = ROOT / "conformance/probes/temporal-forwarding"
manifest = json.loads((PROBES / "capture.json").read_text())
assert manifest["jar_sha256"] == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
assert len(manifest["cases"]) == 14
assert len({r["name"] for r in manifest["cases"]}) == 14
for row in manifest["cases"]:
    assert row["sany_exit"] == 0
    for relative, expected in row["sha256"].items():
        assert hashlib.sha256((PROBES / relative).read_bytes()).hexdigest() == expected, relative
mapping = json.loads((ROOT / "conformance/studies/temporal-scopes-53-map.json").read_text())
source = ROOT / mapping["source_report"]
assert hashlib.sha256(source.read_bytes()).hexdigest() == mapping["source_report_sha256"]
old = json.loads(source.read_text())
assert len(mapping["rows"]) == len(old["rows"]) == 53
for mapped, original in zip(mapping["rows"], old["rows"]):
    assert mapped["model"] == original["model"]
    assert mapped["original_outcome"] == original["outcome"]
    assert mapped["original_detail"] == original["detail"]
print("14 raw scope controls and all 53 historical rows verified")
