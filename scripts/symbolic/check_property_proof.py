#!/usr/bin/env python3
"""Run original Recomp-Verify properties, a changed protocol, and two adverse controls."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def run(check, module, cfg, prop, *options, status=0):
    command = [*check, str(module), str(cfg), "--property-proof", prop, "--type-invariant", "TypeOK", *options]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != status:
        raise RuntimeError((command, result.returncode, result.stdout, result.stderr))
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    for row in rows:
        print(json.dumps(row), flush=True)
    return rows


def replace(source, dest, old, new):
    text = source.read_text()
    if text.count(old) != 1:
        raise ValueError(f"unexpected input for {old!r}")
    dest.parent.mkdir(exist_ok=True)
    dest.write_text(text.replace(old, new))


def check_named_fragment(work, check):
    module = work / "CounterTrace.tla"
    module.write_text(r"""---- MODULE CounterTrace ----
EXTENDS Integers
VARIABLES x, n
TypeOK == x \in {0,1,2}
Init == /\ x = 0 /\ n = 7
Next == \/ /\ x = 0 /\ x' = 1 /\ n' = n + 3
        \/ /\ x = 1 /\ x' = 2 /\ n' = 4
Safe == x # 2
WithinLayout == x \in {0,1,2}
====
""")
    conf = module.with_suffix(".cfg")
    conf.write_text("INIT Init\nNEXT Next\nINVARIANTS Safe WithinLayout\n")
    bad, = run(check, module, conf, "Safe", status=1)
    assert bad["trace_validated"] and [s["values"]["n"] for s in bad["trace"]["states"]] == [7, 10, 4]
    predicate = work / "counter-predicate.json"
    proved, = run(check, module, conf, "WithinLayout", "--write-predicate", str(predicate))
    assert proved["projected_integers"] == ["n"] and proved["proof"] == "inductive-safety"
    failed, = run(check, module, conf, "Safe", "--verify-predicate", str(predicate), status=2)
    assert failed["proof"] == "inconclusive" and failed["failed_obligation"] == "Safe"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lock", type=Path)
    parser.add_argument("counter", type=Path)
    parser.add_argument("--exporter", required=True)
    parser.add_argument("--dive", required=True)
    args = parser.parse_args()
    check = [sys.executable, Path(__file__).with_name("check.py"), "--exporter", args.exporter,
             "--dive", args.dive, "--time-limit", "90", "--max-memory", "4096"]
    lock, counter = args.lock, args.counter
    cfg = lock.with_suffix(".cfg")
    with tempfile.TemporaryDirectory() as directory:
        work = Path(directory)
        edit = work / "edit" / lock.name
        replace(lock, edit, "SendLock(n) == ", "SendLock(n) ==\n    /\\ ~holds_lock[n]")
        rows = run(check, lock, cfg, "Mutex", "--recheck", str(edit), str(cfg), "--recheck", str(lock), str(cfg))
        fresh, = run(check, edit, cfg, "Mutex")
        assert all(row["proof"] == "inductive-safety" for row in [*rows, fresh])
        assert rows[0]["closure_sweeps"] > 0 and fresh["closure_sweeps"] > 0
        assert all(row["closure_sweeps"] == 0 and 0 < row["reused_obligations"] < row["transition_obligations"]
                   for row in rows[1:])
        fault = work / "fault" / lock.name
        trace = work / "lock.trace"
        replace(lock, fault, "server_holds_lock' = FALSE", "server_holds_lock' = TRUE")
        broken, = run(check, fault, cfg, "Mutex", "--trace", str(trace), status=1)
        assert broken["trace_validated"] and json.loads(trace.read_text())["ending"]["name"] == "Mutex"
        passed, = run(check, counter, counter.with_suffix(".cfg"), "Consistent")
        assert passed["projected_integers"] == ["dummy"] and passed["proof"] == "inductive-safety"
        certificate = work / "closed.json"
        run(check, lock, cfg, "Mutex", "--write-predicate", str(certificate))
        checked, = run(check, edit, cfg, "Mutex", "--verify-predicate", str(certificate))
        assert checked["proof"] == "inductive-safety" and checked["closure_sweeps"] == 0
        failed, = run(check, fault, cfg, "Mutex", "--verify-predicate", str(certificate), status=2)
        assert failed["failed_obligation"] == "Next closure"
        check_named_fragment(work, check)
        observed = work / "observed" / counter.name
        replace(counter, observed, "Consistent == ", "Consistent == dummy = 0 /\\ ")
        refused, = run(check, observed, counter.with_suffix(".cfg"), "Consistent", status=2)
        assert "projected integer is read: dummy" in refused["reason"]


if __name__ == "__main__":
    main()
