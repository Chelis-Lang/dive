#!/usr/bin/env python3
"""Check induction, finite progress, damaged specifications and small full graphs."""

import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import subprocess
import tempfile

import project


PINS = {
    "c1cs.tla": "ee222e8a5a7defc2d449ec30ee24053036560b6b7f1107ab2ab115427fb53af0",
    "c1cs.cfg": "0ef50a8cc706e60f6285efe627f2b34ff9a52d8dfdf9287bed2b358157c4d233",
}
PROOF = Path(__file__).with_name("C1CSProgress.tla")


def checked(cmd, directory, expected=0, marker=None):
    result = subprocess.run(cmd, cwd=directory, text=True, capture_output=True, timeout=300)
    out = result.stdout + result.stderr
    if result.returncode != expected or (marker and marker not in out):
        raise ValueError(f"{cmd} returned {result.returncode}\n{out[-4000:]}")
    return out


def replace_once(source, before, after):
    if source.count(before) != 1:
        raise ValueError(f"source control is ambiguous: {before}")
    return source.replace(before, after)


def prepare(folder, source, apalache):
    folder.mkdir()
    (folder / "c1cs.tla").write_text(source)
    checked([apalache, "parse", "--output=raw.json", "c1cs.tla"], folder)
    enabled = project.module(json.loads((folder / "raw.json").read_text()), source)
    (folder / "C1CSEnabled.tla").write_text(enabled)
    (folder / "C1CSProgress.tla").write_bytes(PROOF.read_bytes())


def symbolic(folder, apalache, init, action, length, property_names, expected=0):
    marker = "The outcome is: NoError" if expected == 0 else "Checker has found an error"
    checked([
        apalache, "check", f"--init={init}", f"--next={action}",
        f"--length={length}", f"--inv={property_names}", "--no-deadlock", PROOF.name,
    ], folder, expected, marker)
    return dict(init=init, next=action, length=length, invariants=property_names,
                result="NoError" if expected == 0 else "countermodel")


def small_graph(root, source, cfg, n, args):
    folder = root / f"small-{n}"
    folder.mkdir()
    (folder / "c1cs.tla").write_text(source)
    small = replace_once(replace_once(replace_once(cfg, "N = 4", f"N = {n}"),
                                      "T = 1", "T = 0"), "F = 1", "F = 0")
    (folder / "c1cs.cfg").write_text(small)
    out = checked([args.java, "-XX:ActiveProcessorCount=1", "-Xmx1g", "-cp", args.tlc_jar,
                   "tlc2.TLC", "-workers", "1", "-fp", "0", "-seed", "0",
                   "-config", "c1cs.cfg", "c1cs.tla"], folder, marker="No error has been found.")
    counts = re.search(r"([\d,]+) states generated, ([\d,]+) distinct states found, "
                       r"([\d,]+) states left on queue", out)
    depth = re.search(r"depth of the complete state graph search is ([\d,]+)", out)
    assert counts and depth
    golden = dict(zip(("generated", "distinct", "left_on_queue", "depth"),
                      [int(x.replace(",", "")) for x in (*counts.groups(), depth.group(1))]))
    # Independently audit the derived enabling guard against TLC's own
    # ENABLED operator in every reachable state of both smaller full models.
    (folder / "C1CSEnabled.tla").write_bytes((root / "published" / "C1CSEnabled.tla").read_bytes())
    (folder / "Oracle.tla").write_text(
        "---- MODULE Oracle ----\nEXTENDS C1CSEnabled\n"
        "FairGuardMatches == FairEnabled <=> ENABLED <<Fair>>_vars\n====\n"
    )
    (folder / "Oracle.cfg").write_text(small + "\nINVARIANT FairGuardMatches\n")
    comparison = checked([args.java, "-XX:ActiveProcessorCount=1", "-Xmx1g", "-cp", args.tlc_jar,
                          "tlc2.TLC", "-workers", "1", "-fp", "0", "-seed", "0",
                          "-config", "Oracle.cfg", "Oracle.tla"], folder,
                         marker="No error has been found.")
    assert counts.group() in comparison and depth.group() in comparison
    actual = json.loads(checked([args.dive, "check", "c1cs.tla", "--config", "c1cs.cfg",
                                 "--workers", "1", "--seed", "0", "--fp", "0",
                                 "--time-limit", "90s", "--max-memory", "1g", "--output", "json"],
                                folder))
    assert actual["verdict"] == "success"
    assert all(actual["statistics"][key] == val for key, val in golden.items())
    return dict(N=n, T=0, F=0, verdict="success", tlc_enabled_agreement=True, **golden)


def verify(args, root):
    for name, pin in PINS.items():
        if sha256((args.model / name).read_bytes()).hexdigest() != pin:
            raise ValueError(f"unexpected published input: {name}")
    source = (args.model / "c1cs.tla").read_text()
    cfg = (args.model / "c1cs.cfg").read_text()
    original = root / "published"
    prepare(original, source, args.apalache)
    calls = [
        ("Init", "Next", 0, "Inv"),
        ("Inv", "Next", 1, "Inv,RankStep"),
        ("Inv", "Next", 0, "Progress,ChoicesDefined"),
        ("Inv", "Fair", 1, "FairNonstutter"),
        ("Init", "Next", 0, "StrongSafety"),
        ("StrongSafety", "Next", 1, "StrongSafety"),
    ]
    result = {"input_sha256": PINS, "obligations": [
        symbolic(original, args.apalache, *call) for call in calls
    ]}
    widened = root / "all_vote_choices"
    widened.mkdir()
    raw = json.loads((original / "raw.json").read_text())
    (widened / "c1cs.tla").write_text(project.all_choices(raw, source))
    for name in ("C1CSEnabled.tla", PROOF.name):
        (widened / name).write_bytes((original / name).read_bytes())
    result["all_satisfying_vote_choices"] = [
        symbolic(widened, args.apalache, "Inv", "Next", 1, "Inv,RankStep"),
        symbolic(widened, args.apalache, "StrongSafety", "Next", 1, "StrongSafety"),
    ]
    result["without_quorum_support"] = symbolic(original, args.apalache, "SafetyAll", "Next",
                                                1, "SafetyAll", 12)
    fairness_start = source.index("/\\ WF_vars")
    proposal = "\\/ Propose(i)"
    no_fair_propose = source[:fairness_start] + replace_once(
        source[fairness_start:], proposal, "")
    mutants = [
        ("omit_propose_from_fairness", no_fair_propose, ("Init", "Next", 0, "Progress")),
        ("count_crashed_for_crash_guard",
         replace_once(source, 'pc[p] # "CRASH"', 'pc[p] = "CRASH"'),
         ("Init", "Next", 1, "Inv")),
        ("discard_previous_deliveries",
         replace_once(source, "rcvdMsg[i] \\cup { msg }", "{ msg }"),
         ("Inv", "Next", 1, "RankStep")),
        ("broadcast_bottom_on_T2",
         replace_once(source, "![i] = msg.value ]", "![i] = Bottom ]"),
         ("StrongSafety", "Next", 1, "StrongSafety")),
    ]
    result["damaged_source"] = {}
    for key, damaged, call in mutants:
        folder = root / key
        prepare(folder, damaged, args.apalache)
        result["damaged_source"][key] = symbolic(folder, args.apalache, *call, 12)
    # A successor equation whose RHS depends on itself is not a total update.
    cyclic = replace_once(source, "[ rcvdMsg EXCEPT", "[ rcvdMsg' EXCEPT")
    try:
        prepare(root / "cyclic", cyclic, args.apalache)
    except AssertionError as err:
        assert str(err) == "cyclic successor assignments"
    else:
        raise AssertionError("cyclic transition accepted")
    result["cyclic_update"] = "rejected"
    result["small_full_graphs"] = [small_graph(root, source, cfg, n, args) for n in (1, 2)]
    return result


if __name__ == "__main__":
    if not __debug__:
        raise RuntimeError("run without Python -O so all proof checks execute")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    for name in ("apalache", "java", "tlc-jar", "dive"):
        parser.add_argument(f"--{name}", required=True, type=lambda p: Path(p).resolve())
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as tmp:
        print(json.dumps(verify(args, Path(tmp)), indent=2))
