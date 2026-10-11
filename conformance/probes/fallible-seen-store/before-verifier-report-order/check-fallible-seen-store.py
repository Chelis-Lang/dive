"""Verify portable evidence for the library-only #205 delivery.

No compilation, network calls, model execution or source edits. This verifies
receipt consistency and file hashes, not authenticity or semantic certification.
The original frozen producer gate and fresh runtime receipts remain separate
from later current-source integration metadata.
"""

import argparse
import gzip
import hashlib
import json
from pathlib import Path, PurePosixPath
import re

PRODUCER = "0be524349e4b09f452ee399edc7005653a4825d5"
SOURCE_INDEX = "34cac2407cb68cf6717201babd480eeb6900c42bd48c61f2facc4f82cb4d90e5"
WORKFLOW = "06f16d6de56ffeb5162c10748e4890da47441b6efb5c9fd0ef2d5b3ffd39ed95"
NEXTEST_POLICY = "7b658a5c3403a139e4a3fb0a52ab51940a32de632e5dc77f688ac1196ba19867"
CONFORMANCE_GATE = "43a9297410fa00d02f6971f9720cdafaedaff3a2aa72c286f0d5e4ff5aede89a"
BINARIES = {
    "dive": "7b43a600e4f66d0b8d570e4e949f3119984fedd89044e102787dd631749ee03d",
    "dive-conformance": "e01cbdf57f0cd9cde87ab3ee9675e13c36a6b759f6defa9c8744df70d98bedf0",
    "dive-store-controls": "f99a849b20f0a4ea3588e5b37f02d7ee2edeb0799c6c9efaac847e785039795f",
    "disk_seen": "fc3cf0974fef880a3035a827a09eed21187934456af4aaa51852a6e32f46f865",
}
TESTS = {
    "complete_u64_identity_and_rehash_match_memory",
    "exact_canonical_bytes_survive_forced_full_hash_collisions",
    "faults_never_become_absence_or_success_and_first_error_survives_cleanup",
    "malformed_files_offsets_and_records_refuse_explicitly",
    "coherent_bad_offset_and_duplicate_index_entries_fail_completion_audit",
    "resource_limits_are_deterministic_and_precede_allocation",
    "cleanup_never_removes_caller_added_files",
    "committed_data_before_failed_index_is_not_an_inserted_entry",
    "failed_rehash_commit_keeps_old_capacity_and_first_failure",
    "probe_bound_refuses_instead_of_claiming_absence",
    "unavailable_files_are_io_errors_and_closed_stores_refuse_work",
    "malformed_key_mode_and_constructor_bounds_do_no_file_work",
}
STAGES = ["controls", "clippy", "nextest", "doctests", "docs", "layering", "standalone", "deny"]
MAX_FILE = 64 * 1024 * 1024
MAX_SOURCE = 16 * 1024 * 1024


def checked(root, name):
    """Resolve bounded relative paths without accepting an escaping symlink."""
    assert isinstance(name, str) and len(name) <= 1024, "invalid relative path"
    part = PurePosixPath(name)
    assert name and not part.is_absolute() and ".." not in part.parts, name
    assert "\\" not in name, name
    target = root / name
    assert target.resolve().is_relative_to(root.resolve()), name
    return target


def read(path, limit=MAX_FILE):
    with path.open("rb") as handle:
        raw = handle.read(limit + 1)
    assert len(raw) <= limit, f"file policy limit: {path.name}"
    return raw


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def load(path):
    return json.loads(read(path))


def check_focused(raw):
    text = raw.decode()
    rows = re.findall(r"^test seen::tests::(\w+) \.\.\. ok$", text, re.MULTILINE)
    passed = set(rows)
    assert len(rows) == len(TESTS) and passed == TESTS, (passed - TESTS, TESTS - passed)
    assert "12 passed; 0 failed; 0 ignored;" in text


def verify(base, current_root=None, binary_root=None):
    receipt = load(base / "receipt.json")
    assert receipt["schema_version"] == 1
    assert receipt["runtime_producer"]["source_commit"] == PRODUCER
    assert receipt["runtime_producer"]["binaries_sha256"] == BINARIES
    assert receipt["scope"] == {
        "library_only": True,
        "engine_admission": False,
        "checkpoint_compatibility": False,
        "crash_durable_insert": False,
        "total_memory_bound": False,
        "family_certification": False,
    }
    records = receipt["files_sha256"]
    assert 1 <= len(records) <= 10000
    for name, expected in records.items():
        assert digest(read(checked(base, name))) == expected, name

    source = load(base / "source/index.json")
    assert source["producer_commit"] == PRODUCER
    assert digest(read(base / "source/index.json")) == receipt["source_index_sha256"] == SOURCE_INDEX
    assert len(source["inputs"]) == 449
    required = {
        "crates/dive-store/src/lib.rs",
        "crates/dive-store/src/seen.rs",
        "crates/dive-store/src/seen/files.rs",
        "crates/dive-store/src/seen/tests.rs",
        "crates/dive-store/examples/disk_seen.rs",
    }
    assert required <= source["inputs"].keys()
    for name, row in source["inputs"].items():
        archive = checked(base / "source", row["archive"])
        with gzip.open(archive, "rb") as handle:
            raw = handle.read(MAX_SOURCE + 1)
        assert len(raw) <= MAX_SOURCE and digest(raw) == row["sha256"], name
    archived_tests = checked(base / "source", source["inputs"]["crates/dive-store/src/seen/tests.rs"]["archive"])
    with gzip.open(archived_tests, "rb") as handle:
        test_source = handle.read(MAX_SOURCE + 1).decode()
    assert set(re.findall(r"#\[test\]\s*fn (\w+)", test_source)) == TESTS

    gate = load(base / "gate/index.json")
    assert gate["producer_commit"] == PRODUCER
    assert [s["name"] for s in gate["stages"]] == STAGES
    workflow = read(base / "gate/producer-workflow.yml").decode()
    assert digest(workflow.encode()) == receipt["workflow_sha256"] == WORKFLOW
    assert digest(read(base / "gate/producer-nextest.toml")) == receipt["nextest_policy_sha256"] == NEXTEST_POLICY
    assert digest(read(base / "gate/producer-conformance-gate.sh")) == receipt["conformance_gate_sha256"] == CONFORMANCE_GATE
    commands = re.findall(r"(python3(?: -I)? scripts/[^\n]+)", workflow)
    assert len(commands) == 36
    expected_commands = {
        "controls": ["cargo fmt --all --check"] + commands,
        "clippy": ["cargo clippy --workspace --all-targets --locked -- -D warnings"],
        "nextest": ["cargo nextest run --workspace --locked --profile ci --test-threads 2"],
        "doctests": ["cargo test --workspace --doc --locked"],
        "docs": ["cargo doc --workspace --no-deps --locked"],
        "layering": ["cargo xtask check-layering"],
        "standalone": ["cargo build -p dive-trace --locked"],
        "deny": ["cargo deny --locked check"],
    }
    for stage in gate["stages"]:
        assert [c["command"] for c in stage["commands"]] == expected_commands[stage["name"]]
        for command in stage["commands"]:
            assert command["exit"] == 0, (stage["name"], command["exit"])
            assert digest(read(checked(base / "gate", command["log"]))) == command["sha256"]
    assert gate["source_index_sha256"] == receipt["source_index_sha256"]
    nextest = read(base / "gate/nextest-0.log").decode()
    count = receipt["nextest_tests"]
    assert count == 1179
    assert f"{count} tests run: {count} passed, 0 skipped" in nextest
    for name in TESTS:
        assert re.search(r"PASS[^\n]*dive-store[^\n]*seen::tests::" + name + r"(?:\s|$)", nextest), name
    inventory = json.loads(read(base / "gate/controls-35.log").decode().splitlines()[-1])
    assert inventory == {
        "selected_tests": 1179, "ignored_tests": 0,
        "hash_partitions": [145, 152, 167, 147, 160, 147, 140, 121],
        "disjoint_and_complete": True,
    }

    runtime = load(base / "runtime/receipt.json")
    assert runtime["runtime_producer"] == receipt["runtime_producer"]
    assert [r["name"] for r in runtime["rows"]] == ["library-controls", "rust-guide", "cm1", "validation-smoke"]
    for row in runtime["rows"]:
        assert row["exit"] == 0, row["name"]
        for stream in ["stdout", "stderr"]:
            assert digest(read(checked(base / "runtime", row[stream]))) == row[stream + "_sha256"]
    check_focused(read(base / "runtime" / runtime["rows"][0]["stdout"]))
    assert read(base / "runtime" / runtime["rows"][1]["stdout"]).decode().splitlines() == [
        "4 exact keys, explicit owned cleanup complete",
        "4 exact keys, explicit owned cleanup complete",
    ]
    assert runtime["widths"] == [1, 4, 8]
    assert runtime["gate_selection"] == {"cm1_models": 21, "validation_smoke_models": 16}
    for name, count in [("cm1", 21), ("validation-smoke", 16)]:
        row = next(row for row in runtime["rows"] if row["name"] == name)
        raw = read(base / "runtime" / row["stdout"]).decode()
        selected = re.findall(r"^\[(\d+)/(\d+)\] (\S+) ", raw, re.MULTILINE)
        assert len(selected) == count
        assert [int(row[0]) for row in selected] == list(range(1, count + 1))
        assert all(int(row[1]) == count and row[2] == "agree" for row in selected)
        assert f"gate: {count} of {count} models run" in raw and raw.endswith("gate: passed\n")

    equality = None
    if current_root is not None:
        current = load(base / "current-source.json")
        assert current["runtime_producer"] == PRODUCER
        assert current["current_sha256"].keys() == source["inputs"].keys()
        for name, expected in current["current_sha256"].items():
            assert digest(read(checked(current_root, name), MAX_SOURCE)) == expected, name
        equality = all(current["current_sha256"][name] == row["sha256"] for name, row in source["inputs"].items())
        assert current["current_rust_cargo_equals_runtime"] is equality

    binaries_verified = False
    if binary_root is not None:
        binaries = receipt["runtime_producer"]["binaries_sha256"]
        assert binaries.keys() == {"dive", "dive-conformance", "disk_seen", "dive-store-controls"}
        for name, expected in binaries.items():
            assert digest(read(checked(binary_root, name), 256 * 1024 * 1024)) == expected, name
        binaries_verified = True
    return {"library_only": True, "producer": PRODUCER, "current_source_equals_runtime": equality, "external_binaries_verified": binaries_verified}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    root = Path(__file__).resolve().parents[1]
    parser.add_argument("delivery", type=Path, nargs="?", default=root / "conformance/probes/fallible-seen-store/current")
    parser.add_argument("--current-root", type=Path, default=root)
    parser.add_argument("--binary-root", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.delivery, args.current_root, args.binary_root), sort_keys=True))
