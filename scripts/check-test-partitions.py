"""Verify actual nextest hash partitions form the complete workspace inventory."""
import json
import subprocess


def inventory(partition=None):
    command = ["cargo", "nextest", "list", "--workspace", "--locked", "--profile", "ci", "--message-format", "json"]
    if partition is not None:
        command.extend(["--partition", f"hash:{partition}/8"])
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE, text=True)
    data = json.loads(result.stdout)
    selected = set()
    ignored = set()
    excluded = set()
    all_tests = set()
    for binary, suite in data["rust-suites"].items():
        for name, test in suite["testcases"].items():
            key = (binary, name)
            if key in all_tests:
                raise SystemExit(f"Duplicate inventory test: {key!r}")
            all_tests.add(key)
            match = test["filter-match"]
            if match["status"] == "matches":
                if test["ignored"]:
                    raise SystemExit(f"Ignored test unexpectedly selected: {key!r}")
                selected.add(key)
            elif match["status"] == "mismatch" and match["reason"] == "ignored":
                ignored.add(key)
            elif partition is not None and match["status"] == "mismatch" and match["reason"] == "partition":
                excluded.add(key)
            else:
                raise SystemExit(f"Unexpected inventory filter for {key!r}: {match!r}")
    if not all_tests or not selected or len(all_tests) != data["test-count"]:
        raise SystemExit("Empty or inconsistent nextest inventory")
    return selected, ignored, all_tests


complete, ignored, all_tests = inventory()
covered = set()
counts = []
for number in range(1, 9):
    selected, partition_ignored, partition_all = inventory(number)
    if partition_all != all_tests or partition_ignored != ignored:
        raise SystemExit(f"Inventory changed in partition {number}")
    if covered & selected or not selected <= complete:
        raise SystemExit(f"Duplicate or extra tests in partition {number}")
    covered.update(selected)
    counts.append(len(selected))
if covered != complete:
    raise SystemExit(f"Partitions omit {len(complete - covered)} workspace tests")
print(json.dumps({"selected_tests": len(complete), "ignored_tests": len(ignored), "hash_partitions": counts, "disjoint_and_complete": True}))
