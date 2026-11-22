#!/usr/bin/env python3
"""Certify the pinned AsyncRaft election invariant through original Next."""

import argparse
from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import re
from shutil import copyfile
from tempfile import TemporaryDirectory

from certify_twophase import digest, finite_set, measured, replay, run_json, tlc, tuple_expr
from discover_induction import literal

SOURCE = "7d739ad5f8637e974b57fabd83bd18f3b9a739c03c254af4fb087dd8ab5f2322"
TARGET = "H_OnePrimaryPerTerm"
VOTE_GUARD = r"/\ votedFor[i] \in {Nil, j}"
HEAD = "---- MODULE ElectionTrial ----\nEXTENDS AsyncRaft\n"
CUTOFF = "check-target-before-filter"


def cfg_for(path, servers):
    published = path.read_text()
    block = published.split("CONSTANTS\n", 1)[1].split("\nCONSTRAINT", 1)[0]
    constants = "\n".join(dict.fromkeys(line.strip() for line in block.splitlines()
                                        if line.strip() and not line.lstrip().startswith(r"\*")))
    if servers == 2:
        constants = constants.replace("{n1, n2, n3}", "{n1, n2}")
    constants = (constants.replace("MaxTerm = 3", f"MaxTerm = {2 if servers == 2 else 1}")
                 .replace("MaxLogLen = 3", "MaxLogLen = 1")
                 .replace("MaxMsgCount = 1000", "MaxMsgCount = 3"))
    return "CONSTANTS\n" + constants + "\n"


def encode_table(rows):
    dictionaries = [{} for _ in rows[0]]
    codes = []
    for row in rows:
        encoded = [json.dumps(value, sort_keys=True, separators=(",", ":")) for value in row]
        codes.append([column.setdefault(value, len(column)) + 1
                      for column, value in zip(dictionaries, encoded, strict=True)])
    return [[json.loads(value) for value in column] for column in dictionaries], codes


def names_of(value):
    if isinstance(value, list):
        for item in value:
            yield from names_of(item)
    elif isinstance(value, dict):
        if "model" in value:
            yield value["model"]
        else:
            for item in value.values():
                yield from names_of(item)


def write_certificate_trial(root, constants, native):
    names = native["variables"]
    columns, codes = encode_table(native["states"])
    aliases = {name: f"ProofAtom{i}" for i, name in enumerate(sorted(set(names_of(columns))), 1)}
    src = [HEAD, "VARIABLE proofStep\n"]
    if aliases:
        src += ["CONSTANTS " + ", ".join(aliases.values()) + "\n"]
    lookup, tests, assign, encoded_terms = [], [], [], []
    for i, (name, cells) in enumerate(zip(names, columns, strict=True), 1):
        dictionary = f"ProofValues{i}"
        inverse = f"ProofIndex{i}"
        src.append(f"{dictionary} == {tuple_expr(literal(cell, aliases) for cell in cells)}\n")
        src.append(fr"{inverse} == [v \in {{ {dictionary}[k] : k \in DOMAIN {dictionary} }} |-> "
                   fr"CHOOSE k \in DOMAIN {dictionary} : {dictionary}[k] = v]" + "\n")
        assign.append(f"{name} = {dictionary}[entry[{i}]]")
        tests.append(fr"{name} \in DOMAIN {inverse}")
        encoded_terms.append(f"{inverse}[{name}]")
    for i in range(0, len(codes), 128):
        part = f"ProofBlock{i // 128}"
        lookup.append(part)
        rows = (tuple_expr(map(str, code)) for code in codes[i:i+128])
        src.append(f"{part} == {{\n" + ",\n".join(rows) + "}\n")
    joined = " /\\ ".join(assign)
    all_keys = " /\\ ".join(tests)
    src.append(f"ProofRows == UNION {finite_set(lookup)}\n")
    src.append(fr"ProofCandidate == IF ({all_keys}) THEN {tuple_expr(encoded_terms)} \in ProofRows ELSE FALSE" + "\n")
    src.append(fr"ProofStart == /\ proofStep = FALSE /\ \E entry \in ProofRows : {joined}" + "\n")
    src.append(r"ProofInit == /\ Init /\ proofStep = FALSE" + "\n")
    src.append(r"ProofNext == /\ ~proofStep /\ Next /\ proofStep' = TRUE" + "\n")
    src.append(r"ProofHold == UNCHANGED <<vars, proofStep>>" + "\n")
    src.append(fr"ProofGoal == /\ {TARGET} /\ (IF StateConstraint THEN ProofCandidate ELSE proofStep)" + "\n")
    src.append(r"ProofPrestates == StateConstraint" + "\n====\n")
    wrapper = root / "ElectionTrial.tla"
    wrapper.write_text("".join(src))
    cfg = wrapper.with_suffix(".cfg")
    binding = "".join(f"CONSTANT {alias} = {name}\n" for name, alias in aliases.items())
    return wrapper, cfg, constants + binding


def ordinary_cfg(constants, invariant=TARGET, constrained=True):
    return (constants + "INIT Init\nNEXT Next\n" +
            ("CONSTRAINT StateConstraint\n" if constrained else "") +
            f"INVARIANT {invariant}\nCHECK_DEADLOCK FALSE\n")


def drive(args, module, cfg, work, returncode):
    return run_json([args.dive, "check", module, "--config", cfg, "--workers", "1", "--store", "exact",
                     "--seed", "0", "--fp", "0", "--time-limit", "60s", "--exit-codes", "native", "--output", "json"],
                    work, returncode)


def leaders(value):
    by_node = {name["model"]: item for name, item in value["currentTerm"]["fcn"]}
    return {name["model"]: by_node[name["model"]] for name, role in value["state"]["fcn"] if role["model"] == "Leader"}


def replay_word(args, module, cfg, trace, root):
    assert replay(args.dive, module, cfg, trace, root) == "valid"
    damaged = deepcopy(trace)
    damaged["states"][-1]["values"] = deepcopy(damaged["states"][0]["values"])
    assert replay(args.dive, module, cfg, damaged, root) == "invalid"
    return {"length": len(trace["states"]), "replay": "valid", "damaged_replay": "invalid",
            "actions": [(state.get("action") or {}).get("name") for state in trace["states"][1:]]}


def vote_control(args, root, constants, assertion):
    wrapper = root / "ElectionTrial.tla"
    wrapper.write_text(HEAD + f"ProofGoal == {assertion}\n====\n")
    cfg = wrapper.with_suffix(".cfg")
    cfg.write_text(ordinary_cfg(constants, "ProofGoal"))
    row, _ = drive(args, wrapper, cfg, root, 10)
    trace = row["counterexample"]["trace"]
    control = replay_word(args, wrapper, cfg, trace, root)
    return control, trace


def mutations(args, root, module, cfg, constants):
    wrapper = root / "broken"
    wrapper.mkdir()
    changed = wrapper / module.name
    original = module.read_text()
    if original.count(VOTE_GUARD) != 1:
        raise ValueError("vote uniqueness guard source changed")
    changed.write_text(original.replace(VOTE_GUARD, r"/\ TRUE"))
    copied = changed.with_suffix(".cfg")
    copyfile(cfg, copied)
    row, usage = drive(args, changed, copied, wrapper, 10)
    trace = row["counterexample"]["trace"]
    end = leaders(trace["states"][-1]["values"])
    assert len(set(end.values())) < len(end)
    assert replay(args.dive, module, cfg, trace, root) == "invalid"
    result = replay_word(args, changed, copied, trace, wrapper)
    # Check the reachable failure independently with the same constrained Init/Next.
    regression = wrapper / "ElectionTrial.tla"
    regression.write_text(HEAD + f"ProofGoal == {TARGET}\n====\n")
    rcfg = regression.with_suffix(".cfg")
    rcfg.write_text(ordinary_cfg(constants, "ProofGoal"))
    independent = tlc(args, regression, rcfg, wrapper, "violation")
    return dict(result, guard=VOTE_GUARD, same_term_leaders=end,
                original_replay="invalid", dive=usage, pinned_tlc=independent)


def verify_manifest(artifact, bound, module, cfg):
    expected = dict(source=digest(module), config=digest(cfg), actions=[row["name"] for row in bound["actions"]],
                    variables=bound["variables"], target=TARGET, boundary=CUTOFF)
    if artifact != expected:
        raise ValueError("missing or mismatched action, carrier or boundary proof")
    return expected


def prove(args, root, servers):
    module = root / "AsyncRaft.tla"
    copyfile(args.benchmarks / module.name, module)
    if digest(module) != SOURCE:
        raise ValueError("unexpected bound source revision")
    constants = cfg_for(args.benchmarks / "AsyncRaft.cfg", servers)
    cfg = module.with_suffix(".cfg")
    cfg.write_text(ordinary_cfg(constants))
    native, discovery = run_json([args.producer, module, cfg], root)
    assert native["constraints"] == ["StateConstraint"] and native["boundary"] == CUTOFF
    stored = root / "candidate.json"
    stored.write_text(json.dumps(native["states"], separators=(",", ":")))
    confirmation, checked = run_json([args.producer, module, cfg, stored], root)
    assert all(confirmation[name] == native[name] for name in ("actions", "generated", "expanded", "outside"))
    core, baseline = drive(args, module, cfg, root, 0)
    assert core["statistics"]["distinct"] == native["expanded"] and core["statistics"]["generated"] == native["generated"]

    wrapper, trial_cfg, trial_constants = write_certificate_trial(root, constants, native)

    def proof_check(initial, relation, goal="ProofGoal"):
        trial_cfg.write_text(trial_constants + f"INIT {initial}\nNEXT {relation}\nINVARIANT {goal}\nCHECK_DEADLOCK FALSE\n")
        return tlc(args, wrapper, trial_cfg, root)

    manifest = dict(source=digest(module), config=digest(cfg), actions=[row["name"] for row in native["actions"]],
                    variables=native["variables"], target=TARGET, boundary=CUTOFF)
    verify_manifest(manifest, confirmation, module, cfg)
    missing = dict(manifest, actions=manifest["actions"][:-1])
    try:
        verify_manifest(missing, confirmation, module, cfg)
    except ValueError:
        pass
    else:
        raise ValueError("missing action certificate accepted")

    init = proof_check("ProofInit", "ProofHold")
    implication = proof_check("ProofStart", "ProofHold")
    live = proof_check("ProofStart", "ProofHold", "ProofPrestates")
    closure = proof_check("ProofStart", "ProofNext")
    assert implication["distinct"] == live["distinct"] == native["expanded"]
    assert closure["generated"] == native["expanded"] + native["generated"] - native["initial_generated"]

    election, trace = vote_control(args, root, constants,
        r'\A i \in Server : state[i] = Leader => votesGranted[i] \subseteq {i}')
    required = {"RequestVoteAction", "UpdateTermAction", "HandleRequestVoteRequestAction",
                "HandleRequestVoteResponseAction", "BecomeLeaderAction"}
    # The action label may be that of the original existential wrapper.
    assert required <= set(election["actions"])
    final = trace["states"][-1]["values"]
    elected, = leaders(final)
    granted = next(v for i, v in final["votesGranted"]["fcn"] if i["model"] == elected)
    genuine = {name["model"] for name in granted["set"]} - {elected}
    assert genuine
    first_response = next(i for i, state in enumerate(trace["states"])
                          if (state.get("action") or {}).get("name") == "HandleRequestVoteResponseAction")
    before = trace["states"][first_response - 1]["values"]["requestVoteResponseMsgs"]["set"]
    after = trace["states"][first_response]["values"]["requestVoteResponseMsgs"]["set"]
    positive, = [entry["record"] for entry in before if entry not in after]
    assert positive["mvoteGranted"] and positive["msource"]["model"] in genuine
    assert positive["mdest"]["model"] == elected
    election["leader"] = elected
    election["non_self_voters"] = sorted(genuine)
    election["consumed_positive_vote"] = positive
    negative, rejected = vote_control(args, root, constants,
        r'\A m \in requestVoteResponseMsgs : m.mvoteGranted')
    responses = rejected["states"][-1]["values"]["requestVoteResponseMsgs"]["set"]
    negative["negative_responses"] = sum(not entry["record"]["mvoteGranted"] for entry in responses)
    assert negative["negative_responses"]
    mutation = mutations(args, root, module, cfg, constants)

    wrapper.write_text(HEAD + r"""EmptyVoteChannels == /\ StateConstraint
    /\ requestVoteRequestMsgs = {}
    /\ requestVoteResponseMsgs = {}
    /\ appendEntriesRequestMsgs = {}
    /\ appendEntriesResponseMsgs = {}
====
""")
    trial_cfg.write_text(ordinary_cfg(constants).replace("CONSTRAINT StateConstraint", "CONSTRAINT EmptyVoteChannels"))
    slice_proof, _ = run_json([args.producer, wrapper, trial_cfg], root)
    assert slice_proof["expanded"] == 1
    stored.write_text(json.dumps(slice_proof["states"]))
    incomplete, _ = measured([args.producer, module, cfg, stored], root)
    assert incomplete.returncode and "action closure" in incomplete.stderr
    return {
        "carrier": servers, "cfg": cfg.read_text(), "model_sha256": digest(module),
        "rowset_sha256": sha256(json.dumps(native["states"], separators=(",", ":")).encode()).hexdigest(),
        "native": {name: value for name, value in native.items() if name != "states"},
        "cold_discovery": discovery, "cold_core": baseline, "independent_fixed_row_native": checked,
        "pinned_tlc": {"Init": init, "implication": implication, "constraint": live, "all_Next": closure},
        "manifest": manifest, "election": election, "negative_vote": negative, "mutation": mutation,
        "missing_action": "rejected", "empty_channel_candidate": "closure rejected",
    }


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("benchmarks", type=lambda v: Path(v).resolve())
    for option in ("dive", "producer", "tlc-jar"):
        p.add_argument("--" + option, required=True, type=lambda v: Path(v).resolve())
    p.add_argument("--java", default="java")
    p.add_argument("--carrier", type=int, choices=(2, 3))
    args = p.parse_args()
    for n in ((args.carrier,) if args.carrier else (2, 3)):
        with TemporaryDirectory() as temporary:
            print(json.dumps(prove(args, Path(temporary), n)), flush=True)


if __name__ == "__main__":
    main()
