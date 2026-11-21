#!/usr/bin/env python3
"""Prepare an Isabelle session for the pinned three-process, two-disk Disk Synod model."""

import argparse
from hashlib import sha256
from pathlib import Path
import sys

EXAMPLES = {
    "DiskSynod.tla": "c3924d65bfedaf991a43f50b1739822543e8e25b39c15854f9bf80cab09902da",
    "HDiskSynod.tla": "1758028ad1a579b045189ed305392361a5d154101ed8f9773bc3955f7646713a",
    "MC_HDiskSynod.tla": "f70a04902aaa20b175daddeead891e2f687128b8fe43b30cee3049bbb46af041",
    "MC_HDiskSynod.cfg": "76994aae4c323b971b05cc6604f92213f7bf6f147cf40074c1b03fecf5d1005c",
    "Synod.tla": "4bd2a69cc03151158cb422a0ee670470ad62b0dd2b56bc1d49a67cf77c3cc008"
}
AFP = {
    "DiskPaxos.thy": "23cc827aa77016eb9caa645e4cb783b080130d867e0f4d2e397431adbc5a460d",
    "DiskPaxos_Chosen.thy": "c825608b3440e0f2638dc26f4201179c46ea0f03e690637326617497a2672b5c",
    "DiskPaxos_Inv1.thy": "b054dfe697d4fee1386fef92f38abb32531d7fc130ac881c8e6883f785301d56",
    "DiskPaxos_Inv2.thy": "55c1ef3aeaedfebab9b1be2cc0ee4a76380a5a06365987a1021a479f8778f141",
    "DiskPaxos_Inv3.thy": "ce4aef1e244dc437e8eb283f16cc7805c05c8a237252abb89217428bc8227e0a",
    "DiskPaxos_Inv4.thy": "aabcc45e591e2a35356f7e95d1e36a061641a9b8a7be1078db893cac469f1962",
    "DiskPaxos_Inv5.thy": "f06ec2c963e4cf07f10b065f4c2c47491a4d77656add3fc796ddda5e5e2b4efa",
    "DiskPaxos_Inv6.thy": "326d017e50cd4e3f6eb2419569da7e26b007ce989c8642764388cf6b4fb27e4a",
    "DiskPaxos_Invariant.thy": "2246daa98b6e1f3b3e41b43a8cbf3d90f7364c23e799eb8eac56c3f863c17b36",
    "DiskPaxos_Model.thy": "fb6fdc376d02e424884c34fa3f19483b672edef47d4072c284c45ac72987f866"
}
# Concrete values for the constants and all six discharged AFP axioms.
CONFIGURATION = r"""
datatype InputsOrNi = I1 | I2 | NoInput
datatype Disk = D1 | D2
datatype Proc = P1 | P2 | P3

definition Inputs :: "InputsOrNi set" where "Inputs = {I1,I2}"
definition NotAnInput :: InputsOrNi where "NotAnInput = NoInput"
fun Ballot :: "Proc \<Rightarrow> nat set" where
  "Ballot P1 = {2,3}" | "Ballot P2 = {4,5}" | "Ballot P3 = {6,7}"
definition IsMajority :: "Disk set \<Rightarrow> bool" where "IsMajority s = (s = {D1,D2})"

lemma NotAnInput: "NotAnInput \<notin> Inputs" by (simp add: Inputs_def NotAnInput_def)
lemma InputsOrNi: "(UNIV :: InputsOrNi set) = Inputs \<union> {NotAnInput}"
  by (auto simp: Inputs_def NotAnInput_def; metis InputsOrNi.exhaust)
lemma Ballot_nzero: "\<forall>p. 0 \<notin> Ballot p"
  by (intro allI; rename_tac p; case_tac p; simp)
lemma Ballot_disj: "\<forall>p q. p \<noteq> q \<longrightarrow> Ballot p \<inter> Ballot q = {}"
  by (intro allI; rename_tac p q; case_tac p; case_tac q; simp)
lemma Disk_universe: "(UNIV :: Disk set) = {D1,D2}" by (auto; metis Disk.exhaust)
lemma Disk_isMajority: "IsMajority(UNIV)" by (simp add: IsMajority_def Disk_universe)
lemma majorities_intersect: "\<forall>S T. IsMajority(S) \<and> IsMajority(T) \<longrightarrow> S \<inter> T \<noteq> {}"
  by (simp add: IsMajority_def)
lemma concrete_quorum: "IsMajority s = (card s * 2 > 3)"
proof -
  have "s \<subseteq> {D1,D2}" using Disk_universe by blast
  then have "s = {} \<or> s = {D1} \<or> s = {D2} \<or> s = {D1,D2}" by blast
  then show ?thesis by (auto simp: IsMajority_def)
qed

"""


def checked(directory, manifest):
    files = {}
    for name, digest in manifest.items():
        data = (directory / name).read_bytes()
        if sha256(data).hexdigest() != digest:
            raise ValueError(f"{name} differs from the pinned input")
        files[name] = data
    return files


def prepare(examples, afp, output):
    checked(examples, EXAMPLES)
    theories = checked(afp, AFP)
    name = "DiskPaxos_Model.thy"
    model = theories[name].decode()
    start = model.index("typedecl InputsOrNi")
    end = model.index("lemma ballots_not_zero", start)
    theories[name] = (model[:start] + CONFIGURATION + model[end:]).encode()
    companion = Path(__file__).with_name("DiskPaxos_Configured.thy")
    theories[companion.name] = companion.read_bytes()
    output.mkdir()
    for name, data in theories.items():
        (output / name).write_bytes(data)
    (output / "ROOT").write_text(
        "session Disk_Source = HOL + theories DiskPaxos DiskPaxos_Configured\n"
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("examples", type=Path, help="pinned Examples specifications/diskpaxos")
    parser.add_argument("afp", type=Path, help="pinned AFP thys/DiskPaxos")
    parser.add_argument("output", type=Path, help="new directory for the Isabelle session")
    args = parser.parse_args()
    try:
        prepare(args.examples, args.afp, args.output)
    except (OSError, ValueError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
