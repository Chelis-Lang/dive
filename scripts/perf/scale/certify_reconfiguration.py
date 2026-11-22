#!/usr/bin/env python3
"""Reproduce the finite certificate and independently replay I3DP reconfiguration paths."""

import argparse
from copy import deepcopy
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

from certify_twophase import measured, replay


MODEL = "MongoLoglessDynamicRaft"
PINS = {
    MODEL + ".config.json": "71ffd5e063d553d2f9943e1541bbe7b596ffb93711c209d36a8eec238e435624",
    MODEL + ".tla": "d4a0a219c734ae3ae08173ae05d2123dea8367c068f4bc42d5f9311bd9ebaed6",
    MODEL + "_IndProofs.tla": "adc1d11f90074815cdec5504400476caf604c2380297c6f255ed1650f3dc63e5",
}
CHECKER = Path(__file__).resolve().parents[2] / "symbolic/check_named_induction.py"


def ensure(condition, explanation):
    if not condition:
        raise ValueError(explanation)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(command, root, *, expected=0, seconds=240):
    run, usage = measured(command, root, seconds)
    if run.returncode != expected:
        raise ValueError((command, run.returncode, run.stdout[-1200:], run.stderr[-800:]))
    return json.loads(run.stdout), usage


def original_config(settings, size):
    constants = "\n".join(settings["constants"])
    ensure("Nat <- NatFinite" in constants and settings["constraint"] == "CONSTRAINT StateConstraint",
           "unexpected I3DP finite configuration")
    if size == 2:
        constants = constants.replace("{n1,n2,n3}", "{n1,n2}")
    return constants + "\nINIT Init\nNEXT Next\nCONSTRAINT StateConstraint\nINVARIANT Safety\nCHECK_DEADLOCK FALSE\n"


def write_input(source, settings, root, size):
    base = (source / (MODEL + ".tla")).read_text()
    proof = (source / (MODEL + "_IndProofs.tla")).read_text()
    for name in [MODEL + ".tla", "Randomization.tla"]:
        shutil.copy(source / name, root / name)
    ensure(re.search(r"^NatFinite == 0\.\.3$", base, re.M), "changed finite integer carrier")
    # In this pinned source Nat occurs in TypeOK and other unused definitions.
    # The original Init/Next, Safety, constraint and seven strengthening clauses use
    # only finite named domains and integer operations. Specialize TypeOK's Nat
    # bounds explicitly because the finite exporter refuses operator redirection.
    original_type = base.split("TypeOK ==\n", 1)[1].split("\nTypeOKRandom ==", 1)[0]
    finite_type, count = re.subn(r"\bNat\b", "NatFinite", original_type)
    ensure(count == 3, "changed TypeOK integer bounds")
    clauses = proof[proof.index("StrConj_0 =="):proof.index("ActiveConfigSet ==")]
    ensure(clauses.count(r"/\ TypeOK") == 1, "unexpected source strengthening")
    header = f"---- MODULE ProofTrial ----\nEXTENDS {MODEL}\nCONSTANTS n1,n2,n3\n"
    (root / "ProofTrial.tla").write_text(
        header + "\nFiniteCandidateType ==\n" + finite_type
        + "\nLayout ==\n" + finite_type.replace("NatFinite", "0..4")
        + "\n" + clauses.replace(r"/\ TypeOK", r"/\ FiniteCandidateType") + "\n====\n")
    cfg = original_config(settings, size)
    (root / "Original.cfg").write_text(cfg)
    (root / "ProofTrial.cfg").write_text(cfg.replace("Nat <- NatFinite\n", ""))
    return root / (MODEL + ".tla"), root / "Original.cfg"


def check_core(dive, module, cfg, root, expected):
    return invoke([dive, "check", module, "--config", cfg, "--store", "exact", "--workers", "1",
                   "--seed", "0", "--fp", "0", "--max-memory", "4g", "--time-limit", "180s",
                   "--exit-codes", "native", "--output", "json"], root, expected=expected)


def validate_pair(dive, module, cfg, trace, root):
    ensure(replay(dive, module, cfg, trace, root) == "valid", "independent validator refused genuine trace")
    damaged = deepcopy(trace)
    damaged["states"][1]["values"] = deepcopy(trace["states"][-1]["values"])
    ensure(replay(dive, module, cfg, damaged, root) == "invalid", "independent validator accepted damaged trace")
    return {"length": len(trace["states"]), "sha256": hashlib.sha256(json.dumps(trace, sort_keys=True).encode()).hexdigest(),
            "validated": True, "damaged_rejected": True}


def original_path(dive, root, module, cfg):
    roles = [
        ("BecomeLeaderAction", "BecomeLeader(n1,{n1,n2})"),
        ("SendConfigAction", "SendConfig(n1,n2)"),
        ("ReconfigAction", "Reconfig(n1,{n1,n2,n3})"),
        ("SendConfigAction", "SendConfig(n1,n2)"),
        ("UpdateTermsAction", "UpdateTerms(n1,n3)"),
        ("SendConfigAction", "SendConfig(n1,n3)"),
        ("BecomeLeaderAction", "BecomeLeader(n2,{n2,n3})"),
    ]
    text = f"---- MODULE Positive ----\nEXTENDS {MODEL}\nCONSTANTS n1,n2,n3\nVARIABLE stage\n"
    text += r"Start == Init /\ config[n1]={n1,n2} /\ stage=0" + "\nStep ==\n"
    text += "\n".join(fr"    \/ /\ stage={i} /\ {action} /\ stage'={i + 1}" for i, (_, action) in enumerate(roles))
    text += f"\nWaiting == stage<{len(roles)}\n====\n"
    path = root / "Positive.tla"
    path.write_text(text)
    modified = root / "Positive.cfg"
    modified.write_text(cfg.read_text().replace("INIT Init", "INIT Start").replace("NEXT Next", "NEXT Step")
                        .replace("INVARIANT Safety", "INVARIANT Waiting"))
    row, _ = check_core(dive, path, modified, root, 10)
    word = row["counterexample"]["trace"]
    ensure(len(word["states"]) == 1 + len(roles), "unexpected number of scheduled steps")
    word["ending"] = {"kind": "behavior"}
    word["variables"] = [v for v in word["variables"] if v != "stage"]
    for i, state in enumerate(word["states"]):
        ensure(state["values"].pop("stage") == i, "schedule annotation disagrees with step")
        state["action"] = None if i == 0 else {"name": roles[i - 1][0]}
    return {**validate_pair(dive, module, cfg, word, root), "steps": [name for name, _ in roles]}


def proof_command(args, root):
    return [args.python, CHECKER, root / "ProofTrial.tla", root / "ProofTrial.cfg", "--exporter", args.exporter,
            "--layout", "Layout", "--candidate", "IndAuto", "--property", "Safety"]


def run(args, root):
    ensure(all(sha(args.source / name) == pin for name, pin in PINS.items()), "source pin mismatch")
    settings = json.loads((args.source / (MODEL + ".config.json")).read_text())
    for size in (2, 3):
        directory = root / str(size)
        directory.mkdir()
        model, cfg = write_input(args.source, settings, directory, size)
        cmd = proof_command(args, directory)
        cert = directory / "proof.json"
        result, usage = invoke([*cmd, "--save", cert], directory)
        relocated = root / f"receiving-{size}"
        shutil.copytree(directory, relocated)
        checked, receiving = invoke([*proof_command(args, relocated), "--verify", cert], relocated)
        ensure(result == checked and result["result"] == "proved", "relocated verification disagreed")
        record = dict(servers=size, proof=result, checking=usage, receiving=receiving,
                      certificate_bytes=cert.stat().st_size, certificate_sha256=sha(cert))
        if size == 2:
            core, core_usage = check_core(args.dive, model, cfg, directory, 0)
            ensure(core["verdict"] == "success" and core["statistics"]["left_on_queue"] == 0, "incomplete Core check")
            record["core"] = dict(statistics=core["statistics"], usage=core_usage)
        else:
            record["original_path"] = original_path(args.dive, directory, model, cfg)
            wrong = directory / "missing-action.json"
            doc = json.loads(cert.read_text())
            doc["actions"] = [x for x in doc["actions"] if x["action"] != "ReconfigAction"]
            wrong.write_text(json.dumps(doc))
            refused, _ = invoke([*cmd, "--verify", wrong], directory, expected=2)
            ensure("differs from independently checked" in refused["reason"], "unexpected omitted-action verdict")
            text = model.read_text()
            guard = r"/\ currentTerm[i] < term"
            ensure(text.count(guard) == 1, "unexpected mutation site")
            model.write_text(text.replace(guard, r"/\ TRUE"))
            refused, _ = invoke([*cmd, "--verify", cert], directory, expected=2)
            ensure("different source" in refused["reason"], "unexpected stale-receipt verdict")
            unsafe, usage = check_core(args.dive, model, cfg, directory, 10)
            word = unsafe["counterexample"]["trace"]
            ensure(unsafe["property"] == "Safety", "wrong violated property")
            def primary_terms(state):
                v = state["values"]
                terms = {k["model"]: term for k, term in v["currentTerm"]["fcn"]}
                return [terms[k["model"]] for k, r in v["state"]["fcn"] if r["model"] == "Primary"]
            terms = list(map(primary_terms, word["states"]))
            ensure(all(len(set(t)) == len(t) for t in terms[:-1]) and len(set(terms[-1])) < len(terms[-1]),
                   "two-primary violation reported at the wrong state")
            record["mutant"] = dict(guard=guard, first_failure=len(terms)-1, terms=terms[-1], usage=usage,
                                    **validate_pair(args.dive, model, cfg, word, directory))
            record.update(stale_rejected=True, omitted_action_rejected=True)
        print(json.dumps(record), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--python", default=sys.executable)
    args = parser.parse_args()
    args.source, args.exporter, args.dive = (p.resolve() for p in (args.source, args.exporter, args.dive))
    with tempfile.TemporaryDirectory() as fresh:
        run(args, Path(fresh))


if __name__ == "__main__":
    main()
