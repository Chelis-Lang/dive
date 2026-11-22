#!/usr/bin/env python3
"""Replay the pinned TwoPhase and AsyncRaft finite discovery and TLC controls."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory
from time import monotonic
from types import SimpleNamespace

from discover_induction import Inconclusive, Trial
from scimitar_controls import finite


RAFT_DOMAIN = r""" /\ requestVoteRequestMsgs = {}
 /\ requestVoteResponseMsgs = {}
 /\ appendEntriesRequestMsgs = {}
 /\ appendEntriesResponseMsgs = {}
 /\ currentTerm \in [Server -> 0..1]
 /\ state \in [Server -> {Leader,Follower,Candidate}]
 /\ votedFor = [i \in Server |-> Nil]
 /\ votesGranted \in [Server -> SUBSET Server]
 /\ nextIndex = [i \in Server |-> [j \in Server |-> 1]]
 /\ matchIndex = [i \in Server |-> [j \in Server |-> 0]]
 /\ log = [i \in Server |-> <<>>]
 /\ commitIndex = [i \in Server |-> 0]
 /\ TypeOK"""

CASES = {
    "TwoPhase": ("787afe81e60270d4b1489527504a4fb42c471d26a516bc0562126b57f23f0b87",
                 "H_TCConsistent", "RMRcvAbortMsgAction", "RM", "TypeOK"),
    "AsyncRaft": ("7d739ad5f8637e974b57fabd83bd18f3b9a739c03c254af4fb087dd8ab5f2322",
                  "H_OnePrimaryPerTerm", "BecomeLeaderAction", "Server", RAFT_DOMAIN),
}


def pin(path, expected):
    source = path.read_bytes()
    if sha256(source).hexdigest() != expected:
        raise ValueError(f"unexpected source digest for {path.name}")
    return source.decode()


def arguments(args, name, directory):
    digest, target, action, carrier, domain = CASES[name]
    model = args.benchmarks / (name + ".tla")
    pin(model, digest)
    if name == "TwoPhase":
        constants = "CONSTANT RM = {rm1, rm2}"
    else:
        original = model.with_suffix(".cfg").read_text()
        block = original.split("CONSTANTS\n", 1)[1].split("\nCONSTRAINT", 1)[0]
        constants = "CONSTANTS\n" + "\n".join(dict.fromkeys(
            line for line in block.replace("{n1, n2, n3}", "{n1, n2}").splitlines()
            if line.strip() and not line.lstrip().startswith(r"\*")
        ))
    domain_path = directory / "domain.txt"
    constants_path = directory / "constants.cfg"
    domain_path.write_text(domain)
    constants_path.write_text(constants)
    return SimpleNamespace(module=model, target=target, action=action, carrier=carrier,
                           domain=domain_path, constants=constants_path, variables="vars",
                           init="Init", dependency=[], dive=args.dive, exporter=args.exporter, seconds=60)


def tlc(args, trial, candidate, expected, **options):
    trial.prepare(candidate, **options)
    start = monotonic()
    run = subprocess.run([
        args.java, "-cp", args.tlc_jar, "tlc2.TLC", "-workers", "1", "-fp", "0",
        "-seed", "0", "-metadir", "tlc-states", "-config", trial.cfg.name, trial.module.name,
    ], cwd=trial.module.parent, capture_output=True, text=True, timeout=60)
    output = run.stdout + run.stderr
    if expected == "success":
        assert run.returncode == 0 and "Model checking completed. No error has been found." in output, output[-3200:]
    else:
        goal = options.get("goal", trial.args.target)
        assert run.returncode != 0 and f"Invariant {goal} is violated" in output, output[-3200:]
    counts = re.search(r"([\d,]+) states generated, ([\d,]+) distinct states found", output)
    row = {"verdict": expected, "wall_s": round(monotonic() - start, 6)}
    if counts:
        row.update(zip(("generated", "distinct"), (int(value.replace(",", "")) for value in counts.groups())))
    return row


def published(args, name, trial, candidate):
    root = args.benchmarks
    if name == "TwoPhase":
        source = pin(root / "TwoPhase_IndProofs_nfm.tla",
                     "fbacfe5b8349100d4d59554205b0046f2fd4fb59b873dd7cbe4109fbfe20e23f")
        pool = source.split("IndGlobal ==", 1)[1].split("\n\n", 1)[0]
        names = [value for value in re.findall(r"/\\ (H_\w+)", pool) if value != trial.args.target]
        assert len(names) == 15
        rows = []
        for value in names:
            start = monotonic()
            row = finite(args.dive, trial.module.parent, value)
            rows.append({**row, "wall_s": round(monotonic() - start, 6)})
        winners = [row["candidate"] for row in rows if row["result"] == "finite-preservation"]
        assert winners == ["H_RMCommittedImpliesNoAbortMsg"]
        comparison = expression = winners[0]
        scan = {"candidates": rows, "cold_full_scan_s": sum(row["wall_s"] for row in rows)}
    else:
        source = pin(root / "AsyncRaft_IndProofs_OnePrimaryPerTerm_1.tla",
                     "a7238f4cf990fb00613b40d4dae74b1a9f8ce0b463a8690cdca49f5620392c66")
        comparison = "Inv42_d848_R0_0_I0"
        expression = re.search(r"^" + comparison + r" == (.+)$", source, re.M)[1]
        start = monotonic()
        row = trial.check(expression)
        assert row["verdict"] == "success"
        scan = {"published_lemma_same_domain_s": round(monotonic() - start, 6), "counts": row["statistics"]}
    extra = f"Equivalent == Strengthening <=> ({expression})"
    equivalence = tlc(args, trial, candidate, "success", init="CoverageInit",
                      next_action="TrialHold", goal="Equivalent", extra=extra)
    return {"published": comparison, "equivalence_on_typed_target_prestates": equivalence, **scan}


def run_case(args, name):
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        options = arguments(args, name, root)
        cold_start = monotonic()
        result = Trial(options, root).discover()
        candidate = result["candidate"]
        checked = result["checked"]
        assert result["baseline_cti"]["original_init_replay"] == "invalid"
        assert result["omitted_cti"]["original_init_replay"] == "invalid"
        assert checked["candidate_preservation"] == ("success" if name == "TwoPhase" else "invariant-violation")
        # A fresh finite enumerator independently checks the surviving formula.
        trial = Trial(options, root)
        target_step = tlc(args, trial, candidate, "success")
        cold_to_tlc_checked_target_s = monotonic() - cold_start
        independent = {
            "target_step": target_step,
            "Init_inclusion": tlc(args, trial, candidate, "success", init="OriginalInit",
                                  next_action="TrialHold", goal="Strengthening"),
            "safe_prestate_coverage": tlc(args, trial, candidate, "success", init="CoverageInit",
                                           next_action="TrialHold", goal="Coverage"),
            "candidate_preservation": tlc(args, trial, candidate, checked["candidate_preservation"], goal="Strengthening"),
            "without_premise": tlc(args, trial, "TRUE", "invariant-violation"),
        }
        assert (independent["target_step"]["distinct"], independent["target_step"]["generated"]) == (
            checked["counts"]["distinct"], checked["counts"]["generated"])
        comparison = published(args, name, Trial(options, root), candidate)
        stopped = Trial(options, root)
        stopped.deadline = monotonic()
        try:
            stopped.discover()
        except Inconclusive as exc:
            assert str(exc) == "discovery deadline"
        else:
            raise AssertionError("expired discovery budget was accepted")
        return {"model": name, "domain": options.domain.read_text(), "constants": options.constants.read_text(),
                "discovery": result, "independent_tlc": independent, "comparison": comparison,
                "cold_to_tlc_checked_target_s": cold_to_tlc_checked_target_s,
                "expired_budget": "inconclusive"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("benchmarks", type=lambda path: Path(path).resolve())
    parser.add_argument("--dive", required=True, type=lambda path: Path(path).resolve())
    parser.add_argument("--exporter", required=True, type=lambda path: Path(path).resolve())
    parser.add_argument("--java", default="java")
    parser.add_argument("--tlc-jar", required=True, type=lambda path: Path(path).resolve())
    args = parser.parse_args()
    for name in CASES:
        print(json.dumps(run_case(args, name)), flush=True)


if __name__ == "__main__":
    main()
