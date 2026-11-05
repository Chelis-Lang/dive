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
    parser.add_argument("--analyze-only", action="store_true", help="test library resolution only; does not satisfy R3 checking")
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
    environment = os.environ.copy()
    for key in ("DIVE_COMPAT", "DIVE_ENGINE"):
        environment.pop(key, None)
    for model in MODELS:
        source = checkout / "specifications" / f"{model}.tla"
        command = [str(binary), "analyze" if arguments.analyze_only else "check", source.name, "--output", "json"]
        if not arguments.analyze_only:
            command += ["--workers", "1", "--seed", "0", "--fp", "0", "--time-limit", "2m", "--max-memory", "2g"]
        try:
            run = subprocess.run(
                command,
                cwd=source.parent, env=environment, capture_output=True, text=True, timeout=150,
                check=False,
            )
        except subprocess.TimeoutExpired:
            sys.exit(f"bare-use: {model}: process exceeded its deadline")
        try:
            report = json.loads(run.stdout)
        except json.JSONDecodeError:
            sys.exit(f"bare-use: {model}: invalid JSON, exit {run.returncode}: {run.stderr}")
        if arguments.analyze_only:
            if run.returncode != 0 or report.get("accepted") is not True:
                sys.exit(f"bare-use: {model}: analysis failed: {report.get('diagnostics')} {run.stderr}")
            print(f"bare-use: {model}: analysis accepted (checking not tested)", flush=True)
            continue
        # The existing Reference report omits `engine`; Core and Auto fallback name it.
        engine = report.get("engine", "reference")
        if run.returncode != 0 or report.get("verdict") != "success" or engine not in {"core", "reference"}:
            sys.exit(f"bare-use: {model}: exit {run.returncode}, {report.get('verdict')}: "
                     f"{report.get('message') or report.get('diagnostics')} {run.stderr}")
        print(f"bare-use: {model}: success on {engine}", flush=True)
    operation = "resolve; R3 checking remains pending" if arguments.analyze_only else "check successfully"
    print(f"bare-use: {len(MODELS)} original Examples {operation}, without module/library flags")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
