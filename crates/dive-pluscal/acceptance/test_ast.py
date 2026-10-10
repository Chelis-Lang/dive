#!/usr/bin/env python3
"""Offline grammar oracle controls and comparator false-agreement mutations."""
import copy
from collections import Counter
import json
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).parent))
from compare import reference, outcome, sha
from capture import JAR_SHA

root = Path(__file__).parent
native = json.load(sys.stdin)
counts = Counter()
controls = {}
for oracle, inputs in [(root / "fixtures/curated", root / "cases"),
                       (root / "fixtures/edges", root / "edge-cases"),
                       (root / "fixtures/imported/oracle", root / "fixtures/imported/inputs")]:
    manifest = json.loads((oracle / "manifest.json").read_text())
    assert manifest["oracle_sha256"] == JAR_SHA
    assert manifest["adapter_sha256"] == sha((root / "PlusCalProbe.java").read_bytes())
    for row in manifest["rows"]:
        source = (inputs / row["input"]).read_bytes()
        assert sha(source) == row["input_sha256"]
        secondary = reference(oracle, row["source-grammar-secondary"], row["input"])
        primary = reference(oracle, row["primary"], row["input"])
        document = native.pop(row["id"])
        result = outcome(row, document, secondary, primary, source)
        assert result["outcome"] != "disagree", (row["id"], result)
        counts[result["outcome"]] += 1
        controls[row["id"]] = (row, document, secondary, primary, source)
assert not native, "unaccounted native input"

# These controls start from full real source ASTs, never malformed short fakes.
row, genuine, secondary, primary, source = controls["C-process"]
mutations = []
fake = copy.deepcopy(genuine); fake["translation"] = "complete"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["stage"] = "translated"; mutations.append(fake)
fake = {"stage": "source-grammar", "error": {"message": "worker bug"}, "code": "DIVE-P0050"}; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["name"]["text"] = "Wrong"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["body"]["Processes"][0]["fairness"] = "Unfair"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["body"]["Processes"][0]["body"]["statements"] = []; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["statements"].append(copy.deepcopy(fake["ast"]["statements"][0])); mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["source"] += " "; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["tokens"][0]["text"] = "wrong"; mutations.append(fake)
for fake in mutations:
    try:
        result = outcome(row, fake, secondary, primary, source)
        assert result["outcome"] == "disagree", result
    except (ValueError, KeyError, TypeError):
        pass  # Integrity failure cannot turn into an agreement.

# A normalization reject must not excuse a native bug/false translation claim.
row, genuine, secondary, primary, source = controls["P-labels_macro"]
fake = {"stage": "source-grammar", "error": {"message": "bug"}, "code": "DIVE-P0050"}
assert outcome(row, fake, secondary, primary, source)["outcome"] == "disagree"
print(json.dumps({"summary": dict(counts), "false_agreement_mutations": len(mutations) + 1}))
