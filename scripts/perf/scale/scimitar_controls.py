#!/usr/bin/env python3
"""Select a TwoPhase supporting lemma on finite CTIs and replay authored proof controls."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
from tempfile import TemporaryDirectory

PINS = {
    "TwoPhase": "787afe81e60270d4b1489527504a4fb42c471d26a516bc0562126b57f23f0b87",
    "TwoPhase_IndProofs_nfm": "fbacfe5b8349100d4d59554205b0046f2fd4fb59b873dd7cbe4109fbfe20e23f",
    "AbstractRaft": "1c664febaf32f7be402d6afff5acf0e6d6a109c8453bfc83accd90bc18f0de85",
    "AbstractRaft_IndProofs_nfm": "bbb02656b8f15ba34324f3b31d37242da103b7c033acb92396478342488eabf2",
    "TCommit": "98974d8c5ee007f4f62188737c20c7dcdb05af099cc53a59c8b8651331fd925d",
    "TCommit_proof": "a6e1f248ce225bbf9e61c45006cbed84b63381c5f6ea99194ad4ee9477fc22ff",
}


def prove(tlapm, root, name, *, expect_failure=False):
    run = subprocess.run([tlapm, "--threads", "1", "--nofp", "--method", "smt", "--stretch", "0.2",
                          name + ".tla"], cwd=root, capture_output=True, text=True, timeout=90)
    output = run.stdout + run.stderr
    if expect_failure:
        failed = re.search(r"(\d+)/(\d+) obligations? failed", output)
        if run.returncode != 3 or failed is None or failed.groups() != ("1", "1"):
            raise ValueError("expected one checked and failed obligation: " + output[-800:])
        return 1
    matches = re.findall(r"All (\d+) obligations? proved", output)
    if run.returncode or not matches or int(matches[-1]) == 0:
        raise ValueError(name + " external proof failed: " + output[-800:])
    return int(matches[-1])


def leaf(text, token):
    lines = [line.strip() for line in text.splitlines() if token in line and " BY DEF " in line]
    if len(lines) != 1:
        raise ValueError("expected one published leaf containing " + token)
    return lines[0].split(". ", 1)[1]


def finite(dive, root, hypothesis):
    hyp = r" /\ " + hypothesis if hypothesis else ""
    spec = root / "Receive.tla"
    cfg = spec.with_suffix(".cfg")
    spec.write_text(r"""---- MODULE Receive ----
EXTENDS TwoPhase
VARIABLE stepped
Start == /\ TypeOK /\ H_TCConsistent /\ stepped = FALSE""" + hyp + r"""
Receive == /\ ~stepped /\ RMRcvAbortMsgAction /\ stepped' = TRUE
====
""")
    cfg.write_text("CONSTANT RM = {rm1, rm2}\nINIT Start\nNEXT Receive\nINVARIANT H_TCConsistent\nCHECK_DEADLOCK FALSE\n")
    common = [spec, "--config", cfg, "--output", "json", "--seed", "0", "--fp", "0"]
    r = subprocess.run([dive, "check", *common, "--store", "exact", "--workers", "1",
                        "--time-limit", "15s", "--exit-codes", "native"], text=True, capture_output=True, timeout=20)
    row = json.loads(r.stdout)
    if row["verdict"] == "success" and r.returncode == 0:
        assert row["statistics"]["depth"] == 2 and row["statistics"]["left_on_queue"] == 0
        return dict(candidate=hypothesis, result="finite-preservation", states=row["statistics"]["distinct"])
    if r.returncode != 10 or row["verdict"] != "invariant-violation":
        raise ValueError((row["verdict"], row["message"]))
    trace = row["counterexample"]["trace"]
    assert len(trace["states"]) == 2 and trace["ending"]["name"] == "H_TCConsistent"
    result = subprocess.run([dive, "validate-trace", *common, "--trace", "/dev/stdin"],
                            input=json.dumps(trace), capture_output=True, text=True, timeout=10)
    if result.returncode or json.loads(result.stdout).get("result") != "valid":
        raise ValueError("independent CTI validation failed")
    return dict(candidate=hypothesis, result="validated-CTI", states=len(trace["states"]))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("benchmarks", type=Path, help="Scimitar benchmarks directory")
    p.add_argument("transaction_commit", type=Path, help="Examples transaction_commit directory")
    p.add_argument("--dive", default="dive")
    p.add_argument("--tlapm", default="tlapm")
    args = p.parse_args()
    with TemporaryDirectory() as tmp:
        root = Path(tmp)
        sources = {}
        for name, digest in PINS.items():
            source = args.transaction_commit if name.startswith("TCommit") else args.benchmarks
            contents = (source / (name + ".tla")).read_bytes()
            if hashlib.sha256(contents).hexdigest() != digest:
                raise ValueError("unexpected artifact: " + name)
            (root / (name + ".tla")).write_bytes(contents)
            sources[name] = contents.decode()
        proof = sources["TwoPhase_IndProofs_nfm"]
        pool = proof.split("IndGlobal ==", 1)[1].split("\n\n", 1)[0]
        candidates = [name for name in re.findall(r"/\\ (H_\w+)", pool) if name != "H_TCConsistent"]
        if not candidates:
            raise ValueError("missing published candidates")
        results = [finite(args.dive, root, name) for name in [None, *candidates]]
        print(json.dumps(dict(finite=results)), flush=True)
        winners = [row["candidate"] for row in results if row["result"] == "finite-preservation"]
        support = "H_RMCommittedImpliesNoAbortMsg"
        assert results[0]["result"] == "validated-CTI" and winners == [support]
        step = leaf(proof, "RMRcvAbortMsgAction => H_TCConsistent'")
        # The artifact's BY DEF list leaves this set union opaque to the solver.
        (root / "ReceiveProof.tla").write_text("---- MODULE ReceiveProof ----\nEXTENDS TwoPhase, TLAPS\n"
                                              + "THEOREM " + step + ",msgsAbortCommit\n====\n")
        prefix = "---- MODULE MissingProof ----\nEXTENDS TwoPhase, TLAPS\nTHEOREM "
        weaker = step.replace(support + r" /\ ", "", 1)
        assert weaker != step
        (root / "MissingProof.tla").write_text(prefix + weaker + ",msgsAbortCommit\n====\n")
        print(json.dumps(dict(external="TLAPS", module="MissingProof",
                              failed=prove(args.tlapm, root, "MissingProof", expect_failure=True))), flush=True)
        raft = leaf(sources["AbstractRaft_IndProofs_nfm"], "RollbackEntriesAction => H_OnePrimaryPerTerm'")
        (root / "RollbackProof.tla").write_text("---- MODULE RollbackProof ----\nEXTENDS AbstractRaft, TLAPS\n"
                                               + "THEOREM " + raft + "\n====\n")
        for name in ("ReceiveProof", "RollbackProof", "TCommit_proof"):
            print(json.dumps(dict(external="TLAPS", module=name, proved=prove(args.tlapm, root, name))), flush=True)


if __name__ == "__main__":
    main()
