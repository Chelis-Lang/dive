#!/usr/bin/env python3
"""Check pinned Examples directly, without the harness's module search paths."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import subprocess
import sys
import tomllib
import json

# Original configurations: two PlusCal specifications and several temporal ones.
MODELS = (
    "Paxos/MCPaxos",
    "Bakery-Boulangerie/MCBakery",
    "ewd998/EWD998ChanID",
    "DiningPhilosophers/DiningPhilosophers",
    "TeachingConcurrency/Simple",
    "ewd998/AsyncTerminationDetection",
    "TwoPhase/MCTwoPhase",
    "Paxos/MCConsensus",
    "Paxos/MCVoting",
    "locks_auxiliary_vars/Peterson",
)


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", default=root / "target/release/dive")
    parser.add_argument("--analyze-only", action="store_true", help="test library resolution only, without checking")
    arguments = parser.parse_args()
    binary = Path(arguments.binary).resolve()
    config = tomllib.loads((root / "conformance/corpora.toml").read_text())
    examples = next(c for c in config["corpus"] if c["id"] == "examples")
    cache = Path(os.environ.get("DIVE_CACHE_DIR", Path.home() / ".cache/dive"))
    checkout = cache / "git/examples" / examples["commit"]
    if not checkout.is_dir():
        sys.exit("bare-use: fetch the pinned Examples corpus first")
    actual = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()
    if actual != examples["commit"]:
        sys.exit("bare-use: Examples checkout does not match its pin")
    clean = subprocess.run(["git", "-C", str(checkout), "diff", "--quiet", "HEAD", "--"], check=False)
    if clean.returncode != 0:
        sys.exit("bare-use: Examples checkout has tracked changes")
    environment = os.environ.copy()
    for key in ("DIVE_COMPAT", "DIVE_ENGINE"):
        environment.pop(key, None)
    checked = 0
    refused = 0
    for model in MODELS:
        source = checkout / "specifications" / f"{model}.tla"

        def run_json(command: list[str]) -> tuple[int, dict]:
            try:
                run = subprocess.run(command, cwd=source.parent, env=environment,
                                     capture_output=True, text=True, timeout=150, check=False)
            except subprocess.TimeoutExpired:
                sys.exit(f"bare-use: {model}: process exceeded its deadline")
            try:
                report = json.loads(run.stdout)
            except json.JSONDecodeError:
                sys.exit(f"bare-use: {model}: invalid JSON, exit {run.returncode}: {run.stderr}")
            if run.stderr:
                sys.exit(f"bare-use: {model}: unexpected stderr: {run.stderr}")
            return run.returncode, report

        status, analysis = run_json([str(binary), "analyze", source.name, "--output", "json"])
        if status != 0 or analysis.get("accepted") is not True:
            sys.exit(f"bare-use: {model}: analysis failed: {analysis.get('diagnostics')}")
        if arguments.analyze_only:
            print(f"bare-use: {model}: analysis accepted (checking not tested)", flush=True)
            continue
        status, report = run_json([
            str(binary), "check", source.name, "--output", "json", "--workers", "1",
            "--seed", "0", "--fp", "0", "--time-limit", "2m", "--max-memory", "2g",
        ])
        # Existing Reference reports omit `engine`; Core and Auto fallback name it.
        engine = report.get("engine", "reference")
        if status == 0 and report.get("verdict") == "success" and engine in {"core", "reference"}:
            checked += 1
            print(f"bare-use: {model}: success on {engine}", flush=True)
            continue
        ewd_refusals = {
            "reference": "graph capture with VIEW requires quotient edge and closing-state evidence",
            "core": "a recursive function used other than applied (M1 core)",
        }
        if (model == "ewd998/EWD998ChanID" and status == 3
                and engine in ewd_refusals and report.get("verdict") == "unsupported"
                and report.get("message") == ewd_refusals.get(engine)):
            refused += 1
            print(f"bare-use: {model}: named refusal on {engine}: {report['message']}", flush=True)
            continue
        sys.exit(f"bare-use: {model}: exit {status}, {report.get('verdict')}: "
                 f"{report.get('message') or report.get('diagnostics')}")
    if arguments.analyze_only:
        print(f"bare-use: resolved={len(MODELS)}; checking not tested")
    else:
        print(f"bare-use: resolved={len(MODELS)}, checked={checked}, refused={refused}; "
              "no module/library flags; no Core parity credit")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
