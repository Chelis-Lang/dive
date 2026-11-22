#!/usr/bin/env python3
"""Check authored and inferred TwoPhase/Raft obligations with external TLAPS."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
from tempfile import TemporaryDirectory
import time


EXAMPLES = "6a7b26a31c1a6aeec789b455e9bbfa3b3ad2728f"
SCIMITAR = "f1b902dc0801cf435cb9dd6e6be495d5ed121787"
AUTHORED = {
    "TCommit.tla": "98974d8c5ee007f4f62188737c20c7dcdb05af099cc53a59c8b8651331fd925d",
    "TwoPhase.tla": "e6e72ed0097cca55e80b97af33c3bc54792a504bcbdf639d6d9c666f49c17817",
    "TwoPhase_proof.tla": "01a016d40ffdea5f0350dfe75c853d9b0766d3c7a62b41373a0c33af1f66df0b",
}
PUBLISHED = {
    "TwoPhase.tla": "787afe81e60270d4b1489527504a4fb42c471d26a516bc0562126b57f23f0b87",
    "TwoPhase.cfg": "e8ba979a5927599985026b7b0907616501c95f8a2e1a6efb1bb9f2b312f1a03b",
    "TwoPhase_IndProofs.tla": "7a76e56e4177623195957fe22791f1ca91fa5f824b005131dff32fa6870e377f",
    "AsyncRaft.tla": "7d739ad5f8637e974b57fabd83bd18f3b9a739c03c254af4fb087dd8ab5f2322",
    "AsyncRaft_IndProofs_OnePrimaryPerTerm_1.tla": "a7238f4cf990fb00613b40d4dae74b1a9f8ce0b463a8690cdca49f5620392c66",
    "AsyncRaft_IndProofs_PrimaryHasEntriesItCreated_4.tla": "57f506110f383f9fa83c647063971489d9e8db3d88344cd1e90e17706a41c5df",
    "saved_results/TwoPhase_ind-proof-tree-sd3.proofgraph.json": "1cc2b4e4aa764646886ef4749cf08e6085df51523b3a8902e33bd57700c3a311",
    "saved_results/AsyncRaft_OnePrimaryPerTerm/AsyncRaft_ind-proof-tree-sd1.proofgraph.json":
        "1dd7b8e7abced903c3f5d76b9a24856c1a45e711a94727603927e32c984918b9",
    "saved_results/AsyncRaft_PrimaryHasEntriesItCreated/AsyncRaft_ind-proof-tree-sd4.proofgraph.json":
        "c71a7d7e4f036d97025c0c6091c9fe0f6bfec146b9eceeca7c0511870d7d5d16",
}
CASES = (
    ("TwoPhase", "TwoPhase_IndProofs", "TwoPhase_ind-proof-tree-sd3", "H_TCConsistent"),
    ("AsyncRaft", "AsyncRaft_IndProofs_OnePrimaryPerTerm_1",
     "AsyncRaft_OnePrimaryPerTerm/AsyncRaft_ind-proof-tree-sd1", "OnePrimaryPerTerm"),
    ("AsyncRaft", "AsyncRaft_IndProofs_PrimaryHasEntriesItCreated_4",
     "AsyncRaft_PrimaryHasEntriesItCreated/AsyncRaft_ind-proof-tree-sd4", "PrimaryHasEntriesItCreated"),
)
PREPARED = r'  /\ \A rm \in tmPrepared : PrepMsg(rm) \in msgs'
CTL_CFG = """CONSTANT RM = {rm1, rm2}
INIT HypothesisState
NEXT TMCommit
INVARIANT AfterCommit
CHECK_DEADLOCK FALSE
"""


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_inputs(directory, pins):
    for relative, digest in pins.items():
        if sha256(directory / relative) != digest:
            raise ValueError(f"unexpected source or artifact: {relative}")


def controls(source, directory):
    text = source.read_text()
    invariant = text.split("\nInv ==\n", 1)[1].split("\n\nLEMMA InvInductive", 1)[0]
    lines = invariant.splitlines()
    if lines.count(PREPARED) != 1:
        raise ValueError("the pinned prepared-message hypothesis has changed")
    weakened = "\n".join(line for line in lines if line != PREPARED)
    for module, premise in (("CommitPreserves", "Inv"), ("CommitWithoutPrepared", "WeakenedInv")):
        definition = "" if premise == "Inv" else f"WeakenedInv ==\n{weakened}\n"
        definitions = "Inv, TPTypeOK, TMCommit, Message, CommitMsg, AbortMsg, PrepMsg"
        if definition:
            definitions = f"WeakenedInv, {definitions}"
        proof = rf"""---- MODULE {module} ----
EXTENDS TwoPhase_proof
{definition}
THEOREM CommitPreservation == {premise} /\ TMCommit => Inv'
BY DEF {definitions}
====
"""
        (directory / f"{module}.tla").write_text(proof)
    model = directory / "MissingPreparedHistory.tla"
    model.write_text(rf"""---- MODULE MissingPreparedHistory ----
EXTENDS TwoPhase_proof
WeakenedInv ==
{weakened}
HypothesisState ==
  /\ rmState = [r \in RM |-> "working"]
  /\ tmState = "init"
  /\ tmPrepared = RM
  /\ msgs = {{}}
  /\ WeakenedInv
  /\ ~Inv
AfterCommit == tmState = "committed" => Inv
====
""")
    model.with_suffix(".cfg").write_text(CTL_CFG)
    return model


def prove(tlapm, module, directory, deadline):
    start = time.monotonic()
    log = directory / f"{module}.log"
    with log.open("w") as output:
        result = subprocess.run(
            ["timeout", "--kill-after=5", str(deadline), tlapm, "--nofp", "-k",
                 "--threads", "2", "--toolbox", "0", "0", f"{module}.tla"],
            cwd=directory, stdout=output, stderr=subprocess.STDOUT, check=False,
        )
    text = log.read_text(errors="replace")
    final = {}
    for block in re.findall(r"@!!BEGIN\n(.*?)@!!END", text, re.S):
        info = dict(line.removeprefix("@!!").split(":", 1) for line in block.splitlines()
                    if line.startswith("@!!") and ":" in line)
        if info.get("type") == "obligation" and "status" in info:
            final[info["id"]] = info
    discharged = [row for row in final.values() if row["status"] in ("proved", "trivial")]
    passed = re.findall(r"\[INFO\]: All (\d+) obligations? proved\.", text)
    counts = Counter(row["status"] for row in final.values())
    if result.returncode in (124, 137):
        status = "budget-stop"
    elif result.returncode == 0 and final and passed and int(passed[-1]) == len(final) == len(discharged):
        status = "proved"
    elif counts["failed"] or counts["omitted"]:
        status = "unproved"
    else:
        status = "tool-error"
    failed = next((row for row in final.values() if row["status"] == "failed"), None)
    return dict(module=module, module_sha256=sha256(directory / f"{module}.tla"),
                exit=result.returncode, status=status, wall_seconds=round(time.monotonic() - start, 3),
                obligations=dict(counts), discharged_by=dict(Counter(row.get("prover", "unreported") for row in discharged)),
                first_failed_location=failed.get("loc") if failed else None)


def validate_cti(dive, model):
    options = [model, "--config", model.with_suffix(".cfg"), "--compat=tlc", "--output", "json"]
    checked = subprocess.run(
        [dive, "check", *options, "--store", "exact", "--workers", "1", "--exit-codes", "native"],
        text=True, capture_output=True, timeout=30, check=False,
    )
    row = json.loads(checked.stdout)
    trace = row["counterexample"]["trace"]
    if row["verdict"] != "invariant-violation" or len(trace["states"]) != 2 or trace["ending"]["name"] != "AfterCommit":
        raise RuntimeError("the exact Core step did not refute the weakened preservation claim")
    replay = subprocess.run(
        [dive, "validate-trace", *options, "--trace", "/dev/stdin"], input=json.dumps(trace),
        text=True, capture_output=True, timeout=30, check=False,
    )
    if replay.returncode or json.loads(replay.stdout)["result"] != "valid":
        raise RuntimeError("independent replay rejected the counterexample to induction")
    return dict(depth=2, ending=trace["ending"], trace_validated=True,
                claim="Commit preservation from the weakened hypothesis",
                model_sha256=sha256(model), config_sha256=sha256(model.with_suffix(".cfg")))


def graph_evidence(graph, property_name):
    saved = json.loads(graph.read_text())
    nodes = saved["proof_graph"]["nodes"]
    actions = [node for node in nodes.values() if node.get("is_action")]
    return dict(property=property_name, graph_sha256=sha256(graph), seed=saved["seed"],
                lemma_names=sorted(key for key, val in nodes.items() if val.get("is_lemma")),
                initial_ctis=sum(node["total_initial_ctis"] for node in actions),
                remaining_ctis=sum(node["ctis_remaining"] for node in actions),
                action_nodes=len(actions), producer_stats=saved["stats"])


def native_twophase(dive, source):
    checked = subprocess.run(
        [dive, "check", source / "TwoPhase.tla", "--config", source / "TwoPhase.cfg",
         "--strategy", "symbolic", "--property-proof", "H_TCConsistent", "--type-invariant", "TypeOK",
         "--time-limit", "30s", "--max-memory", "2g", "--output", "json"],
        text=True, capture_output=True, timeout=45, check=True,
    )
    proof = json.loads(checked.stdout)
    if (proof["format"], proof["proof"], proof["property"], proof["failed_obligation"]) != (
        "dive-property-proof-v1", "inductive-safety", "H_TCConsistent", None,
    ) or proof["transition_obligations"] < 1:
        raise RuntimeError("Dive did not certify the configured Scimitar TwoPhase property")
    return {"config_sha256": PUBLISHED["TwoPhase.cfg"],
            **{key: proof[key] for key in ("format", "proof", "property", "transition_obligations",
                                         "support_bits", "seconds")}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("tlapm", "dive", "examples", "scimitar", "work"):
        parser.add_argument(f"--{arg}", type=lambda s: Path(s).resolve(), required=True)
    parser.add_argument("--deadline", type=int, default=160, help="seconds per proof")
    args = parser.parse_args()
    source = args.examples / "specifications" / "transaction_commit"
    published = args.scimitar / "benchmarks"
    verify_inputs(source, AUTHORED)
    verify_inputs(published, PUBLISHED)
    args.work.mkdir()
    backend_config = subprocess.check_output([args.tlapm, "--config"], text=True, stderr=subprocess.DEVNULL)
    backend_versions = dict(re.findall(r'^([^\n=]+) version == "([^"\n]*)"$', backend_config, flags=re.M))
    report = dict(schema="dive-proof-controls-1", examples_commit=EXAMPLES,
                  scimitar_commit=SCIMITAR, deadline_seconds=args.deadline,
                  tlapm_version=subprocess.check_output([args.tlapm, "--version"], text=True).strip(),
                  tlapm_sha256=sha256(args.tlapm), dive_sha256=sha256(args.dive), backend_versions=backend_versions)
    authored = args.work / "authored"
    authored.mkdir()
    for name in AUTHORED:
        shutil.copyfile(source / name, authored / name)
    model = controls(authored / "TwoPhase_proof.tla", authored)
    report["authored_inputs"] = AUTHORED
    proofs = [prove(args.tlapm, name, authored, args.deadline) for name in
              ("TwoPhase_proof", "CommitPreserves", "CommitWithoutPrepared")]
    if [row["status"] for row in proofs] != ["proved", "proved", "unproved"]:
        raise RuntimeError(f"unexpected authored proof controls: {proofs}")
    report.update(authored=proofs, counterexample_to_induction=validate_cti(args.dive, model),
                  native_twophase=native_twophase(args.dive, published))

    with TemporaryDirectory(dir=args.work) as parent:
        for spec, module, graph, property_name in CASES:
            directory = Path(parent) / module
            directory.mkdir()
            for name in (spec, module):
                shutil.copyfile(published / f"{name}.tla", directory / f"{name}.tla")
            derived = graph_evidence(published / "saved_results" / f"{graph}.proofgraph.json", property_name)
            derived.update(model=spec, model_sha256=sha256(directory / f"{spec}.tla"),
                           check=prove(args.tlapm, module, directory, args.deadline))
            shutil.copyfile(directory / f"{module}.log", args.work / f"{module}.log")
            report.setdefault("published", []).append(derived)
    if report["published"][0]["check"]["status"] != "proved":
        raise RuntimeError("TLAPS did not discharge the published TwoPhase proof")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
