#!/usr/bin/env python3
"""Check offline parity accounting. This does not certify semantic conformance."""

from collections import Counter
import json
from pathlib import Path, PurePosixPath
import re
import sys

IDS = tuple(f"F-{number:03}" for number in range(1, 38))
STATUSES = {"implemented", "partial", "missing"}
NUMBERS = dict(enumerate("zero one two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty".split()))
NUMBERS.update({n: "twenty-" + NUMBERS[n - 20] for n in range(21, 30)})
NUMBERS[30] = "thirty"
NUMBERS.update({n: "thirty-" + NUMBERS[n - 30] for n in range(31, 38)})


def require(condition, message):
    if not condition:
        raise ValueError(message)


def relative_path(value, label):
    require(isinstance(value, str) and bool(value), f"{label}: expected individual path")
    path = PurePosixPath(value)
    require(not path.is_absolute() and ".." not in path.parts and "\\" not in value,
            f"{label}: path must stay repository relative")
    return path


def evidence(root, value, label):
    path = relative_path(value, label)
    target = root / path
    require(target.resolve().is_relative_to(root.resolve()), f"{label}: evidence escapes repository")
    require(target.is_file(), f"{label}: evidence file does not exist: {value}")


def validate(root, ledger, markdown):
    """Reject broken evidence references, stable IDs and Markdown accounting."""
    require(isinstance(ledger, dict), "ledger must be an object")
    require(type(ledger.get("schema_version")) is int and ledger["schema_version"] == 1,
            "unsupported parity ledger schema")
    baseline = ledger.get("baseline_commit")
    require(isinstance(baseline, str) and re.fullmatch(r"[0-9a-f]{40}", baseline),
            "invalid baseline commit")
    rows = ledger.get("families")
    require(isinstance(rows, list), "families must be a list")
    require(all(isinstance(row, dict) for row in rows), "families must contain objects")
    require([r.get("id") for r in rows] == list(IDS), "stable family IDs must be F-001 through F-037, exactly once and in order")
    for row in rows:
        fid = row["id"]
        for key in ("name", "acceptance", "pinned_source"):
            require(isinstance(row.get(key), str) and row[key].strip(), f"{fid}: missing {key}")
        source = row["pinned_source"].removeprefix("CommunityModules:")
        relative_path(source, f"{fid} source")
        require(source.endswith(".java"), f"{fid}: pinned source must identify a Java file")
        require(row.get("implementation_status") in STATUSES, f"{fid}: unknown implementation status")
        # Family certification needs a separately reviewed case/matrix schema.
        # Merely changing this string must not credit a completed family.
        require(row.get("certification") == "audit-required", f"{fid}: unsupported certification claim; supply a reviewed certification schema")
        require(type(row.get("phase")) is int and 1 <= row["phase"] <= 9, f"{fid}: invalid delivery phase")
        issues = row.get("issues")
        require(isinstance(issues, list) and all(type(n) is int and n > 0 for n in issues)
                and len(set(issues)) == len(issues), f"{fid}: invalid issue IDs")
        interactions = row.get("interactions")
        require(isinstance(interactions, list) and all(i in IDS and i != fid for i in interactions)
                and len(set(interactions)) == len(interactions), f"{fid}: invalid family interaction")
        for key in ("rust_evidence", "conformance_evidence"):
            evidence(root, row.get(key), f"{fid} {key}")
        additional = row.get("additional_evidence", [])
        require(isinstance(additional, list), f"{fid}: additional evidence must be individual paths")
        for path in additional:
            evidence(root, path, f"{fid} additional evidence")

    table = []
    for line in markdown.splitlines():
        if re.match(r"\| F-\d{3} \|", line):
            fields = [field.strip() for field in line.split("|")[1:-1]]
            require(len(fields) == 5, "invalid Markdown family row")
            table.append(fields[:4])
    expected = [[r["id"], r["name"], r["implementation_status"], str(r["phase"])] for r in rows]
    require(table == expected, "Markdown family table differs from JSON ledger")
    counts = Counter(r["implementation_status"] for r in rows)
    census = re.search(r"stable feature families\*\*: ([\w-]+) have an implemented core,\s*([\w-]+) are partial,\s*and ([\w-]+) are missing", markdown)
    require(census is not None, "Markdown implementation census is missing")
    expected_counts = [counts[s] for s in ("implemented", "partial", "missing")]
    require(list(census.groups()) in ([NUMBERS[n] for n in expected_counts], [str(n) for n in expected_counts]),
            "Markdown implementation census differs from JSON ledger")
    return counts


def main():
    root = Path(__file__).resolve().parent.parent
    try:
        ledger = json.loads((root / "conformance/parity-features.json").read_text())
        markdown = (root / "docs/tlc-parity.md").read_text()
        counts = validate(root, ledger, markdown)
    except (ValueError, TypeError, KeyError, OSError) as error:
        print(f"parity ledger: {error}", file=sys.stderr)
        return 1
    print(f"Parity accounting: 37 families, {dict(counts)}; certification audit remains required.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
