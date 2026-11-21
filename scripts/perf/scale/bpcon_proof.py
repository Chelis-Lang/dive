#!/usr/bin/env python3
"""Prepare the configured Byzantine Paxos invariant proof from pinned sources."""

import argparse
from hashlib import sha256
from pathlib import Path

EXAMPLES = {
    "BPConProof.cfg": "8a3af0fba0f0ed0d96827717e0edacf7ad9bedc23b9f7aebf82f3c2bb5434e9a",
    "BPConProof.tla": "daa731dcadf12a892415131881d82d5b23e56c53097e90b986a24b1408a2e3c6",
    "PConProof.tla": "c78bd085b8e5ec696034d8a259d6d00df1576cf6c553c2c0d89a78d3198fe659",
    "VoteProof.tla": "89f9ca0345801ed0834542c1332bc2b6bb3feb9a95da40d750793e8948d696da",
    "Consensus.tla": "5b877e93b31c982e80b1443127bc2c50c090d7f8d729d468f3b42849315205bf",
}
LIBRARY = Path(__file__).resolve().parents[3] / "crates/dive-sema/bundled/tlaps"
FINITE_SETS = "484bf0f9ab6a69ef45f7282f7f92dcf1e6ae139e44117b0d5a4427635818e773"

# Use the same fresh value in BPConProof and its PConProof instance.
FRESH_NONE = r"""CONSTANT FreshNone
None == FreshNone
ASSUME NoneOutside == None \notin Value
USE NoneOutside"""

P_NEXT_PREMISES = """<1>3. P!NoneOutside
  BY NoneOutside DEF None, P!None, P!NoneOutside
<1>4. QED
  BY P!NextDef, <1>1, <1>2, <1>3, NoSetContainsEverything"""

# Expose the two cases of the function update before asking SMT for the bound.
PHASE2B_ORIGINAL = r"""        <5> [type |-> "1c", bal |-> r.bal, val |-> r.val] \in msgs'
          BY <3>3, MsgsLemma DEF Inv, accInv
        <5> QED
          BY <3>3 DEF Phase2b, Inv, Ballot, TypeOK, accInv"""

PHASE2B = r"""        <5>1. [type |-> "1c", bal |-> r.bal, val |-> r.val] \in msgs'
          BY <3>3, MsgsLemma DEF Inv, accInv
        <5>2. r.bal =< maxBal[a]
          BY DEF Inv, accInv
        <5>3. /\ r.bal \in Int /\ maxBal[a] \in Int /\ b \in Int
          BY DEF Inv, TypeOK, Ballot
        <5>4. /\ maxBal[self] =< b /\ maxBal' = [maxBal EXCEPT ![self] = b]
          BY <3>3 DEF Phase2b
        <5>5. maxBal'[a] = IF a = self THEN b ELSE maxBal[a]
          BY <5>4 DEF Inv, TypeOK
        <5>6. r.bal =< maxBal'[a]
          BY <5>2, <5>3, <5>4, <5>5, SMT
        <5> QED BY <5>1, <5>6"""

# Relate the definitions in the specialized proof to the concrete TLC check.
CONFIG_MAPPING = r"""---- MODULE ConfigMapping ----
EXTENDS BPConProof
THEOREM /\ Ballot = 0..2
        /\ P!Ballot = Ballot
        /\ P!None = None
        /\ None = FreshNone
        /\ None \notin Value
  BY NoneOutside, SMT DEF Ballot, P!Ballot, None, P!None
====
"""

# TLC evaluates the published constant assignments against the original source.
# One stuttering state suffices to exercise startup and the initial invariant.
CONFIG_CHECK = r"""---- MODULE ConfigCheck ----
EXTENDS BPConProof
CONSTANT ConfiguredNone
ASSUME Sentinel == None = ConfiguredNone /\ None \notin Value
ASSUME FiniteBallots == Ballot = 0..2
ASSUME RootPremises == BallotAssump /\ BQA /\ BQLA
ASSUME ProjectedQuorums == P!QA /\ Quorum = {Q \cap Acceptor : Q \in ByzQuorum}
ASSUME ProjectedBallots == (Ballot \cup {-1}) \cap Acceptor = {}
Stay == UNCHANGED vars
====
"""


def read_pinned(path, expected):
    data = path.read_bytes()
    if sha256(data).hexdigest() != expected:
        raise ValueError(f"{path.name} differs from the pinned input")
    return data.decode("utf-8")


def replace_once(source, old, new):
    if source.count(old) != 1:
        raise ValueError(f"expected one source match for {old!r}")
    return source.replace(old, new)


def through(source, theorem, closing):
    start = source.index(theorem)
    end = source.index(closing, start) + len(closing)
    return source[:end] + "\n====\n"


def prepare(examples, output):
    sources = {name: read_pinned(examples / name, digest) for name, digest in EXAMPLES.items()}
    config = sources.pop("BPConProof.cfg")
    sources["FiniteSetTheorems.tla"] = read_pinned(LIBRARY / "FiniteSetTheorems.tla", FINITE_SETS)
    proof = sources.copy()
    for name in ("BPConProof.tla", "PConProof.tla", "VoteProof.tla"):
        proof[name] = replace_once(proof[name], "Ballot == Nat", r"Ballot == {b \in Nat : b <= 2}")
    for name in ("BPConProof.tla", "PConProof.tla"):
        proof[name] = replace_once(proof[name], r"None == CHOOSE v : v \notin Value", FRESH_NONE)
    root = through(
        proof["BPConProof.tla"], "THEOREM Invariance == Spec => []Inv", "  BY <1>1, <1>2, PTL DEF Spec"
    )
    root = replace_once(root, "<1>3. QED\n  BY P!NextDef, <1>1, <1>2, NoSetContainsEverything",
                        P_NEXT_PREMISES)
    proof["BPConProof.tla"] = replace_once(root, PHASE2B_ORIGINAL, PHASE2B)
    imported = through(proof["PConProof.tla"], "THEOREM NextDef == (Next <=> TLANext)",
                       "  BY <1>2, <1>3 DEF Next, TLANext")
    config = replace_once(config, "  None = None\n", "  None = None\n  ConfiguredNone = None\n")
    config = replace_once(config, "SPECIFICATION Spec\n", "INIT Init\nNEXT Stay\n")
    output.mkdir()
    for dirname, files in (
        ("proof", {**proof, "ConfigMapping.tla": CONFIG_MAPPING}),
        ("nextdef", {"PConProof.tla": imported}),
        ("config", {**sources, "ConfigCheck.tla": CONFIG_CHECK, "ConfigCheck.cfg": config}),
    ):
        directory = output / dirname
        directory.mkdir()
        for name, source in files.items():
            (directory / name).write_bytes(source.encode("utf-8"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("examples", type=Path, help="pinned Examples/specifications/byzpaxos directory")
    parser.add_argument("output", type=Path, help="new directory for the proofs and configuration control")
    args = parser.parse_args()
    try:
        prepare(args.examples, args.output)
    except (OSError, ValueError) as error:
        parser.exit(1, f"{parser.prog}: {error}\n")


if __name__ == "__main__":
    main()
