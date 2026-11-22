#!/usr/bin/env python3
"""Exercise the four pinned CCF histories and independent replay controls."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory


def check(prefix, path, code, *options):
    process = subprocess.run([*prefix, path, *options], capture_output=True, text=True, timeout=35)
    if process.returncode != code:
        raise RuntimeError((process.returncode, process.stdout, process.stderr))
    report = json.loads(process.stdout)
    print(json.dumps({"input": path.name, **report}))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--dive", default="dive")
    args = parser.parse_args()
    script = Path(__file__).with_name("match-ccf-trace.py")
    prefix = [sys.executable, script, args.source, "--dive", args.dive]
    cases = {
        "election_no_rollback": 8,
        "election_non_contiguous_view": 8,
        "rollback_same_seqno": 11,
        "rollback_two_transactions": 16,
    }
    for name, events in cases.items():
        row = check(prefix, args.source / "traces" / (name + ".ndjson"), 0)
        assert row["result"] == "matched" and row["matched_observations"] == events
        assert row["witness_states"] > events and row["observation_replay"] == row["protocol_replay"] == "valid"
    original = args.source / "traces/election_no_rollback.ndjson"
    events = [json.loads(line) for line in original.read_text().splitlines()]
    assert events[2]["action"] == "RwTxResponseAction" and events[2]["tx_id"] == [2, 10]
    with TemporaryDirectory() as directory:
        corrupted = Path(directory) / "corrupted.ndjson"
        repaired = Path(directory) / "repaired.ndjson"
        events[2]["tx_id"] = [2, 9]
        corrupted.write_text("".join(json.dumps(event) + "\n" for event in events))
        reject = check(prefix, corrupted, 1)
        assert reject["result"] == "no-completion" and reject["first_incompatible_observation"] == 3
        assert reject["matched_observations"] == 2
        events[2]["tx_id"] = [2, 10]
        repaired.write_text("".join(json.dumps(event) + "\n" for event in events))
        match = check(prefix, repaired, 0)
        assert match["matched_observations"] == 8 and match["protocol_replay"] == "valid"
        stopped = check(prefix, repaired, 5, "--max-states", "1")
        assert stopped["result"] == "inconclusive" and "first_incompatible_observation" not in stopped


if __name__ == "__main__":
    main()
