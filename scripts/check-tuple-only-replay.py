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

# New implementation observations have their own producer. The earlier epochs
# are kept above, including their original mislabelled/malformed selections.
folder = BASE / "implemented-epoch"
index = json.loads((folder / "index.json").read_text())
assert index["all_consumers_closed"] and not index["family_certified"]
for rel, expected in index["files_sha256"].items():
    assert hashlib.sha256((folder / rel).read_bytes()).hexdigest() == expected, rel
source = json.loads(archived(folder, "source-index.json"))
assert source["source_commit"] == index["producer"]
assert source["source_tree"] == index["source_tree"]
assert len(source["source_sha256"]) == index["full_source_paths"]
for rel, expected in index["portable_compile_source_sha256"].items():
    assert source["source_sha256"][rel] == expected
    assert hashlib.sha256(archived(folder, "source/" + rel)).hexdigest() == expected
steps = json.loads(archived(folder, "gate/steps.json"))
assert len(steps) == 8 and all(row["exit"] == 0 for row in steps)
for number, row in enumerate(steps):
    assert hashlib.sha256(archived(folder, "gate/" + str(number) + ".log")).hexdigest() == row["log_sha256"]
assert b"1212 tests run: 1212 passed, 0 skipped" in archived(folder, "gate/2.log")
closure = json.loads(archived(folder, "runtime/closure.json"))
producer = json.loads(archived(folder, "runtime/producer.json"))
runtime = json.loads(archived(folder, "runtime/steps.json"))
assert closure["all_consumers_closed"] and not closure["family_certified"]
assert closure["source_commit"] == producer["source_commit"] == runtime["source_commit"] == index["producer"]
assert producer["source_tree"] == index["source_tree"]
assert len(runtime["steps"]) == closure["steps"] == 582
for row in runtime["steps"]:
    for stream in ("stdout", "stderr"):
        data = archived(folder, "runtime/raw/" + row["name"] + "." + stream)
        assert hashlib.sha256(data).hexdigest() == row[stream + "_sha256"]
    if row["name"].endswith("-check"):
        doc = json.loads(archived(folder, "runtime/raw/" + row["name"] + ".stdout"))
        assert doc["verdict"] == "invariant-violation" and doc["property"] == "Inv"
        assert all(doc["statistics"][key] == 2 for key in ("generated", "distinct", "depth"))
        assert row["exit"] == (12 if "--compat=tlc" in row["command"] else 10)
    elif row["name"].endswith(("-original", "-mutant")):
        doc = json.loads(archived(folder, "runtime/raw/" + row["name"] + ".stdout"))
        original = row["name"].endswith("-original")
        assert doc["result"] == ("valid" if original else "invalid")
        assert row["exit"] == (0 if original else 1)
    else:
        assert row["exit"] == 0
replays = json.loads(archived(folder, "runtime/replays.json"))["rows"]
assert len(replays) == 384 and closure["replays"] == 384
assert sum(row["result"] == "valid" for row in replays) == 192
assert sum(row["result"] == "invalid" for row in replays) == 192
for model in ("literal", "definition", "tuple_if", "concat", "subseq", "wrapper", "let", "alias"):
    selected = [row for row in replays if row["model"] == model]
    assert len(selected) == 48
    assert {(row["profile"], row["workers"], row["store"], row["chaos"], row["kind"]) for row in selected} == {
        (profile, width, store, chaos, kind)
        for profile in ("tlc", "dive") for width in (1, 4, 8)
        for store in ("fp", "exact") for chaos in (0, 13)
        for kind in ("original", "mutant")
    }
for corpus, count in (("examples", 21), ("validation-smoke", 16)):
    paths = [rel for rel in index["files_sha256"] if rel.startswith("runtime/report-" + corpus + "/") and rel.endswith(".json.gz")]
    assert len(paths) == 1
    report = json.loads(gzip.decompress((folder / paths[0]).read_bytes()))
    assert report["summary"]["models_selected"] == report["summary"]["models"]["agree"] == count
    assert len(report["models"]) == count
    for model in report["models"]:
        assert model["outcome"] == "agree"
        assert {run["workers"] for run in model["runs"]} == {1, 4, 8}
        assert all(run["outcome"] == "agree" and not run["diffs"] for run in model["runs"])
print("verified frozen 1212/0skip gate,192 actual checks,192 Valid192 Invalid replays and37 complete width-gate models; original producer retained; variable-stack proof and family certification remain pending")
