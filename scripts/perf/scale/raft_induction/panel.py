#!/usr/bin/env python3
"""Reproduce the two finite log proofs, independently replayed original path and mutant."""

import argparse
from copy import deepcopy
import json
from pathlib import Path
import shutil
import sys
import tempfile

from check import (
    TARGET,
    ORIGINAL_SOURCE,
    UPSTREAM_CFG,
    GRAPH_SHA,
    config,
    digest,
    graph_path,
)
from values import require

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from certify_twophase import measured, replay

CHECKER = Path(__file__).with_name("check.py")
PINS = {"AsyncRaft.tla": ORIGINAL_SOURCE, "AsyncRaft.cfg": UPSTREAM_CFG}


def invoke(argv, where, expected=0):
    run, usage = measured(argv, where, 240)
    require(
        run.returncode == expected,
        (run.returncode, run.stdout[-1000:], run.stderr[-600:]),
    )
    return json.loads(run.stdout), usage


def core(dive, module, cfg, where, expected):
    return invoke(
        [
            dive,
            "check",
            module,
            "--config",
            cfg,
            "--store",
            "exact",
            "--workers",
            "1",
            "--seed",
            "0",
            "--fp",
            "0",
            "--max-memory",
            "4g",
            "--time-limit",
            "160s",
            "--exit-codes",
            "native",
            "--output",
            "json",
        ],
        where,
        expected,
    )


def validated(dive, module, cfg, trace, root):
    require(
        replay(dive, module, cfg, trace, root) == "valid",
        "genuine word failed independent replay",
    )
    damaged = deepcopy(trace)
    damaged["states"][1]["values"] = deepcopy(trace["states"][-1]["values"])
    require(
        replay(dive, module, cfg, damaged, root) == "invalid", "damaged trace accepted"
    )
    return dict(
        states=len(trace["states"]),
        sha256=digest(json.dumps(trace, sort_keys=True).encode()),
        validated=True,
        damaged_rejected=True,
    )


def field(values, key):
    return {k["model"]: v for k, v in values[key]["fcn"]}


def bad_indices(values):
    terms = field(values, "currentTerm")
    logs = {k: v["seq"] for k, v in field(values, "log").items()}
    return [
        (leader, owner, index + 1, entry)
        for leader, role in field(values, "state").items()
        if role["model"] == "Leader"
        for owner, log in logs.items()
        for index, entry in enumerate(log)
        if entry == terms[leader]
        and (len(logs[leader]) <= index or logs[leader][index] != entry)
    ]


def election():
    steps = [("RequestVoteAction", "RequestVote(n1)")]
    for peer in ("n2", "n3"):
        steps.extend(
            [
                ("UpdateTermAction", f"UpdateTerm(1,{peer})"),
                message(
                    "HandleRequestVoteRequest", "requestVoteRequestMsgs", "n1", peer
                ),
                message(
                    "HandleRequestVoteResponse", "requestVoteResponseMsgs", peer, "n1"
                ),
            ]
        )
    steps.append(("BecomeLeaderAction", "BecomeLeader(n1)"))
    return steps


def message(action, carrier, source, dest, guard="TRUE"):
    return (
        action + "Action",
        rf"(\E m \in {carrier} : m.msource={source} /\ m.mdest={dest} /\ {guard} /\ {action}(m))",
    )


def path(dive, module, cfg, root):
    steps = election() + [("ClientRequestAction", "ClientRequest(n1)")] * 2
    for index in (1, 2):
        steps.extend(
            [
                ("AppendEntriesAction", "AppendEntries(n1,n2)"),
                message(
                    "AcceptAppendEntriesRequestAppend",
                    "appendEntriesRequestMsgs",
                    "n1",
                    "n2",
                    f"m.mprevLogIndex={index - 1}",
                ),
                message(
                    "HandleAppendEntriesResponse",
                    "appendEntriesResponseMsgs",
                    "n2",
                    "n1",
                    f"m.mmatchIndex={index}",
                ),
            ]
        )
    steps.append(("RequestVoteAction", "RequestVote(n2)"))
    trial = root / "OriginalWord.tla"
    trial.write_text(
        "---- MODULE OriginalWord ----\nEXTENDS AsyncRaft\nCONSTANTS n1,n2,n3\nVARIABLE stage\n"
        + r"Start == Init /\ stage=0"
        + "\nStep ==\n"
        + "\n".join(
            rf"    \/ /\ stage={i} /\ {action} /\ stage'={i + 1}"
            for i, (_, action) in enumerate(steps)
        )
        + f"\nWaiting == stage<{len(steps)}\n====\n"
    )
    trialcfg = trial.with_suffix(".cfg")
    trialcfg.write_text(
        cfg.read_text()
        .replace("INIT Init", "INIT Start")
        .replace("NEXT Next", "NEXT Step")
        .replace("INVARIANT " + TARGET, "INVARIANT Waiting")
    )
    result, _ = core(dive, trial, trialcfg, root, 10)
    require(result["property"] == "Waiting", "failed to reach scheduled end")
    trace = result["counterexample"]["trace"]
    require(len(trace["states"]) == len(steps) + 1, "incomplete execution")
    trace["variables"].remove("stage")
    trace["ending"] = {"kind": "behavior"}
    for i, st in enumerate(trace["states"]):
        require(st["values"].pop("stage") == i, "wrong stage")
        st["action"] = None if i == 0 else {"name": steps[i - 1][0]}
        require(not bad_indices(st["values"]), "unexpected failure on original path")
    last = trace["states"][-1]["values"]
    require(
        field(last, "log")["n1"] == field(last, "log")["n2"] == {"seq": [1, 1]}
        and field(last, "state")["n1"] == {"model": "Leader"}
        and field(last, "currentTerm")["n2"] == 2,
        "failed two-index replication and term-change control",
    )
    return dict(
        **validated(dive, module, cfg, trace, root),
        steps=[k for k, _ in steps],
        copied_indices=[1, 2],
        elected="n1",
        replicated_to="n2",
        final_follower_term=2,
    )


def altered(dive, src, cfg, root):
    place = root / "mutant"
    place.mkdir()
    for name in PINS:
        shutil.copy(src / name, place / name)
    shutil.copy(graph_path(src), place / graph_path(src).name)
    original = src / "AsyncRaft.tla"
    mutant = place / original.name
    text = original.read_text()
    old = "ClientRequest(i) ==\n    /\\ state[i] = Leader"
    require(text.count(old) == 1, "mutation site changed")
    mutant.write_text(
        text.replace(
            old, "ClientRequest(i) ==\n    /\\ state[i] \\in {Leader, Follower}"
        )
    )
    record, usage = core(dive, mutant, cfg, place, 10)
    trace = record["counterexample"]["trace"]
    indices = [bad_indices(s["values"]) for s in trace["states"]]
    require(
        record["property"] == TARGET and not any(indices[:-1]) and indices[-1],
        "wrong first failure",
    )
    require(
        replay(dive, original, cfg, trace, root) == "invalid",
        "original accepted a mutant execution",
    )
    return place, dict(
        **validated(dive, mutant, cfg, trace, place),
        first_bad_step=len(indices) - 1,
        first_bad_entries=indices[-1],
        steps=[s["action"]["name"] for s in trace["states"][1:]],
        original_rejected=True,
        core=usage,
    )


def run(args, root):
    for file, sha in PINS.items():
        require(
            digest((args.source / file).read_bytes()) == sha, "changed pinned " + file
        )
    require(
        digest(graph_path(args.source).read_bytes()) == GRAPH_SHA,
        "changed pinned graph",
    )
    for size in (2, 3):
        work = root / str(size)
        work.mkdir()
        for name in PINS:
            shutil.copy(args.source / name, work / name)
        shutil.copy(graph_path(args.source), work / graph_path(args.source).name)
        module = work / "AsyncRaft.tla"
        cfg = work / "Study.cfg"
        cfg.write_text(config(work / "AsyncRaft.cfg", size))

        def check(directory, *opts, expected=0):
            return invoke(
                [
                    sys.executable,
                    CHECKER,
                    directory,
                    "--exporter",
                    args.exporter,
                    "--servers",
                    str(size),
                    *opts,
                ],
                work,
                expected,
            )

        exhaustive, usage = core(args.dive, module, cfg, work, 0)
        ordinary = dict(counts=exhaustive["statistics"], usage=usage)
        cert = work / "cert.json"
        proof, usage = check(work, "--save", cert)
        relocated = root / f"receiver-{size}"
        shutil.copytree(work, relocated)
        receiver, receiving = check(relocated, "--verify", cert)
        require(
            receiver == proof and receiver["result"] == "proved",
            "independent proof mismatch",
        )
        row = dict(
            servers=size,
            proof=proof,
            checking=usage,
            receiving=receiving,
            certificate_bytes=cert.stat().st_size,
            certificate_sha256=digest(cert.read_bytes()),
        )
        row["core"] = ordinary
        if size == 2:
            mistyped = deepcopy(proof)
            mistyped["initial"] = True
            wrong = work / "mistyped.json"
            wrong.write_text(json.dumps(mistyped))
            for flags in ([], ["-O"]):
                refused, _ = invoke(
                    [
                        sys.executable,
                        *flags,
                        CHECKER,
                        work,
                        "--exporter",
                        args.exporter,
                        "--servers",
                        str(size),
                        "--verify",
                        wrong,
                    ],
                    work,
                    2,
                )
                require(
                    "independent action discharge" in refused["reason"],
                    "boolean initial count accepted",
                )
            row["boolean_count_rejected"] = True
        else:
            row["original_path"] = path(args.dive, module, cfg, work)
            place, row["mutant"] = altered(args.dive, work, cfg, work)
            refused, _ = check(place, "--verify", cert, expected=2)
            require("different source" in refused["reason"], "stale receipt accepted")
            failure, _ = check(place, "--save", work / "mutant-cert.json", expected=2)
            require(
                "ClientRequestAction" in failure["reason"],
                "expected failed ClientRequest induction",
            )
            damaged = deepcopy(proof)
            damaged["actions"].pop()
            wrong = work / "omitted.json"
            wrong.write_text(json.dumps(damaged))
            refused, _ = check(work, "--verify", wrong, expected=2)
            require(
                "independent action discharge" in refused["reason"],
                "missing action accepted",
            )
            graph = graph_path(relocated)
            changed = json.loads(graph.read_bytes())
            changed["strengthening_conjuncts"][0][1] = "FALSE"
            graph.write_text(json.dumps(changed))
            refused, _ = check(relocated, "--verify", cert, expected=2)
            require("different source" in refused["reason"], "changed formula accepted")
            row["controls"] = dict(
                stale_rejected=True,
                changed_formula_rejected=True,
                omitted_action_rejected=True,
                mutant_induction_failure=failure["reason"],
            )
        print(json.dumps(row), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("--exporter", type=Path, required=True)
    p.add_argument("--dive", type=Path, required=True)
    args = p.parse_args()
    for key in ("source", "exporter", "dive"):
        setattr(args, key, getattr(args, key).resolve())
    with tempfile.TemporaryDirectory() as temporary:
        run(args, Path(temporary))


if __name__ == "__main__":
    main()
