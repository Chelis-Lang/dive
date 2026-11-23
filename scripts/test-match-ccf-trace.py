#!/usr/bin/env python3
"""Exercise the four pinned CCF histories and independent replay controls."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time
from types import SimpleNamespace
from tempfile import TemporaryDirectory


def check(prefix, path, code, *options):
    process = subprocess.run([*prefix, path, *options], capture_output=True, text=True, timeout=35)
    if process.returncode != code:
        raise RuntimeError((process.returncode, process.stdout, process.stderr))
    report = json.loads(process.stdout)
    print(json.dumps({"input": path.name, **report}))
    return report


def mixed_fixture(script, directory, binary):
    spec = importlib.util.spec_from_file_location("ccf_matcher", script)
    matcher = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(matcher)
    protocol = r"""EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 1
MNRSpec == Init /\ [][Next]_x
"""
    observations = r"""VARIABLE l
TraceInit == Init /\ l = 1
ObservedNext == l = 1 /\ l' = 2 /\ x' \in VALUES
"""
    for module, body, cfg in [
        ("Observation", protocol + observations, "INIT TraceInit\nNEXT ObservedNext\n"),
        ("Refinement", protocol + observations, "INIT TraceInit\nNEXT ObservedNext\nPROPERTY MNRSpec\n"),
        ("Protocol", protocol + r"ProtocolNext == Next \/ UNCHANGED x" + "\n", "INIT Init\nNEXT ProtocolNext\n"),
        ("Match", protocol + observations + r"MatchNext == ObservedNext /\ (Next \/ UNCHANGED x)" + "\n",
         "INIT TraceInit\nNEXT MatchNext\nINVARIANT LogPending\n"),
    ]:
        (directory / f"{module}.cfg").write_text(cfg + "CHECK_DEADLOCK FALSE\n")
        if module == "Match":
            match = f"---- MODULE {module} ----\n{body}"
        else:
            (directory / f"{module}.tla").write_text(f"---- MODULE {module} ----\n{body}====\n")
    options = ["--store", "exact", "--workers", "1", "--exit-codes", "tlc"]
    args = SimpleNamespace(dive=binary)
    deadline = time.monotonic() + 5
    remaining = lambda: deadline - time.monotonic()
    for domain, verdict in [("{1, 2}", "violation"), ("{1}", "refines")]:
        for module in ("Observation", "Refinement"):
            (directory / f"{module}.tla").write_text(
                f"---- MODULE {module} ----\n{protocol}{observations.replace('VALUES', domain)}====\n"
            )
        status, _ = matcher.search_prefix(binary, directory, match.replace("VALUES", domain), 1, options, remaining())
        assert status == "matched"
        row = matcher.refinement(args, directory, options, remaining)
        assert row["result"] == verdict, row
        print(json.dumps({"fixture": domain, "existential": status, "refinement": row}))
        if verdict == "violation":
            word = json.loads((directory / "refinement-observation.json").read_text())
            assert word["states"][-1]["values"] == {"l": 2, "x": 2}
            word["states"][-1]["values"].pop("l")
            try:
                matcher.project(word)
            except matcher.Unsupported:
                pass
            else:
                raise AssertionError("malformed observation layout accepted")
        stopped = matcher.refinement(args, directory, [*options, "--max-states", "1"], remaining)
        assert stopped["result"] == "inconclusive", stopped


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--dive", default="dive")
    parser.add_argument("--refinement", action="store_true")
    args = parser.parse_args()
    script = Path(__file__).with_name("match-ccf-trace.py")
    prefix = [sys.executable, script, args.source, "--dive", args.dive]
    options = ["--refinement"] if args.refinement else []
    cases = {
        "election_no_rollback": 8,
        "election_non_contiguous_view": 8,
        "rollback_same_seqno": 11,
        "rollback_two_transactions": 16,
    }
    for name, events in cases.items():
        row = check(prefix, args.source / "traces" / (name + ".ndjson"), 0, *options)
        assert row["result"] == "matched" and row["matched_observations"] == events
        assert row["witness_states"] > events and row["observation_replay"] == row["protocol_replay"] == "valid"
        if args.refinement:
            assert row["refinement"]["result"] == "refines", row
    if args.refinement:
        with TemporaryDirectory() as directory:
            binary = subprocess.check_output(["which", args.dive], text=True).strip()
            mixed_fixture(script, Path(directory), Path(binary))
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
