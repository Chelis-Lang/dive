#!/usr/bin/env python3
"""Infer the finite TwoPhase invariant from original action observations and certify it."""

import argparse
from collections import defaultdict
from copy import deepcopy
from hashlib import sha256
import json
from pathlib import Path
import re
from shutil import copyfile
import subprocess
import sys
from tempfile import TemporaryDirectory
from time import monotonic

from discover_induction import literal
from scimitar_controls import finite

SOURCE = "787afe81e60270d4b1489527504a4fb42c471d26a516bc0562126b57f23f0b87"
TARGET = "H_TCConsistent"


def digest(path):
    return sha256(path.read_bytes()).hexdigest()


def measured(command, root, seconds=60):
    stamp = root / "rss"
    start = monotonic()
    result = subprocess.run(["/usr/bin/time", "-f", "%M", "-o", stamp, *command],
                            cwd=root, capture_output=True, text=True, timeout=seconds)
    usage = {"seconds": round(monotonic() - start, 6),
             "peak_rss_kib": int(stamp.read_text().splitlines()[-1])}
    return result, usage


def run_json(command, root, exit_code=0, seconds=60):
    result, usage = measured(command, root, seconds)
    if result.returncode != exit_code:
        raise ValueError((result.returncode, result.stdout[-1500:], result.stderr[-1800:]))
    return json.loads(result.stdout), usage


def tuple_expr(items):
    return "<<" + ", ".join(items) + ">>"


def finite_set(items):
    return "{" + ", ".join(items) + "}"


def learned(rows, names):
    """Project shared coordinator state and the local fields of one arbitrary manager."""
    shared = r'<<tmState, [type |-> "Commit"] \in msgsCommit, [type |-> "Abort"] \in msgsAbort>>'
    local = r'<<rmState[r], r \in tmPrepared, [type |-> "Prepared", rm |-> r] \in msgsPrepared>>'
    patterns = defaultdict(set)
    for values in rows:
        state = dict(zip(names, values, strict=True))
        key = (state["tmState"], bool(state["msgsCommit"]["set"]), bool(state["msgsAbort"]["set"]))
        delivered = {item["model"] for item in state["tmPrepared"]["set"]}
        sent = {item["record"]["rm"]["model"] for item in state["msgsPrepared"]["set"]}
        for manager, stage in state["rmState"]["fcn"]:
            name = manager["model"]
            patterns[key].add((stage, name in delivered, name in sent))

    def t(values):
        return tuple_expr(literal(value) for value in values)

    clauses = [f"{shared} \\in {finite_set(t(key) for key in sorted(patterns))}"]
    for key, value in sorted(patterns.items()):
        opts = finite_set(t(items) for items in sorted(value))
        clauses.append(f"{shared} = {t(key)} => (\\A r \\in RM : {local} \\in {opts})")
    return clauses


def config_with(cfg, init, next_action, invariant, extra=""):
    constants = [line for line in cfg.splitlines() if line.startswith("CONSTANT RM")]
    if len(constants) != 1:
        raise ValueError("expected a single released RM binding")
    return (f"{constants[0]}\n{extra}INIT {init}\nNEXT {next_action}\n"
            f"INVARIANT {invariant}\nCHECK_DEADLOCK FALSE\n")


def write_trial(root, original_cfg, clauses, *, start="ProofStart", next_action="ProofNext",
                invariant="ProofGoal", weak=False, extra="", extra_cfg=""):
    module = root / "FiniteTrial.tla"
    cfg = module.with_suffix(".cfg")
    conjunction = " /\\ ".join(f"({formula})" for formula in clauses)
    module.write_text(fr"""---- MODULE FiniteTrial ----
EXTENDS TwoPhase
VARIABLE proofStep
{extra}
ProofInvariant == TypeOK /\ {conjunction}
ProofStart == /\ TypeOK /\ proofStep = FALSE /\ ProofInvariant {" /\\ " + TARGET if weak else ""}
ProofInit == /\ Init /\ proofStep = FALSE
ProofNext == /\ ~proofStep /\ Next /\ proofStep' = TRUE
ProofHold == UNCHANGED <<vars, proofStep>>
ProofGoal == ProofInvariant /\ {TARGET}
ProofDeadlock == proofStep \/ ENABLED Next
====
""")
    cfg.write_text(config_with(original_cfg, start, next_action, invariant, extra_cfg))
    return module, cfg


def tlc(args, module, cfg, root, expected="success"):
    result, usage = measured([args.java, "-Xmx1g", "-cp", args.tlc_jar, "tlc2.TLC",
                              "-workers", "1", "-fp", "0", "-seed", "0", "-metadir", "states",
                              "-config", cfg.name, module.name], root)
    output = result.stdout + result.stderr
    if expected == "success":
        ok = result.returncode == 0 and "Model checking completed. No error has been found." in output
    else:
        ok = result.returncode != 0 and "Invariant ProofGoal is violated" in output
    if not ok:
        raise ValueError(output[-4000:])
    counts = re.search(r"([\d,]+) states generated, ([\d,]+) distinct states found", output)
    if not counts:
        raise ValueError("pinned TLC did not return state counts")
    return dict(usage, **dict(zip(("generated", "distinct"),
                (int(x.replace(",", "")) for x in counts.groups()), strict=True)))


def replay(dive, module, cfg, trace, root):
    path = root / "witness.json"
    path.write_text(json.dumps(trace))
    r, _ = measured([dive, "validate-trace", module, "--config", cfg, "--trace", path, "--output", "json"], root)
    return json.loads(r.stdout)["result"]


def action_partition(args, program, actions, root):
    doc, _ = run_json([args.exporter, *program], root)
    ir = doc["ir"]

    def node(index):
        data = ir["exprs"][index]["expr"]
        return next(iter(data.items())) if isinstance(data, dict) else (data, None)

    top = {definition["name"]: definition for definition in ir["defs"] if definition["scope"] == "TopLevel"}
    roles = {}
    for name in actions:
        tag, body = node(top[name]["body"])
        if tag != "Junction" or body["kind"] != "And" or len(body["items"]) != 2:
            raise ValueError("unexpected original action wrapper")
        first, inner = map(node, body["items"])
        if first != ("Bool", True):
            raise ValueError("original action prefix is not TRUE")
        if inner[0] != "Quant":
            if inner[0] != "Def":
                raise ValueError("expected a coordinator action")
            roles[name] = None
            continue
        quant = inner[1]
        bound, = quant["bounds"]
        tag, call = node(quant["body"])
        if (quant["kind"] != "Exists" or bound["tuple"] or node(bound["domain"]) != ("Const", 0)
                or tag != "Apply" or set(call["callee"]) != {"Def"} or len(call["args"]) != 1):
            raise ValueError("expected the original single-manager action")
        slot, = bound["vars"]
        if node(call["args"][0]["Expr"]["expr"]) != ("Local", {"up": 0, "slot": slot}):
            raise ValueError("manager action uses a different binding")
        roles[name] = ir["defs"][call["callee"]["Def"]]["name"]
    return roles, [value["model"] for value in doc["constants"][0]["set"]]


def verify_manifest(artifact, actual, module, cfg):
    names = [action["name"] for action in actual["actions"]]
    expected = {"source": digest(module), "config": digest(cfg), "actions": names,
                "variables": actual["variables"], "target": TARGET, "boundary": actual["boundary"]}
    if artifact["manifest"] != expected:
        raise ValueError("certificate does not match the original source, carrier or action inventory")


def ablation(args, root, cfg, clauses, report):
    for i in range(1, len(clauses)):
        module, trial_cfg = write_trial(root, cfg, clauses[:i] + clauses[i+1:], weak=True)
        command = [args.dive, "check", module, "--config", trial_cfg, "--store", "exact", "--workers", "1",
                   "--seed", "0", "--fp", "0", "--time-limit", "30s", "--exit-codes", "native", "--output", "json"]
        run, _ = measured(command, root)
        if run.returncode == 0:
            continue
        row = json.loads(run.stdout)
        if run.returncode != 10 or row["verdict"] != "invariant-violation":
            raise ValueError(("ablation failed outside the obligation", row))
        trace = row["counterexample"]["trace"]
        if len(trace["states"]) != 2:
            raise ValueError("ablation did not exhibit a closure CTI")
        assert replay(args.dive, module, trial_cfg, trace, root) == "valid"
        damaged = deepcopy(trace)
        damaged["states"][1]["values"]["proofStep"] = False
        assert replay(args.dive, module, trial_cfg, damaged, root) == "invalid"
        original = root / "OriginalInit.cfg"
        original.write_text(config_with(cfg, "ProofInit", "ProofNext", "ProofGoal"))
        assert replay(args.dive, module, original, trace, root) == "invalid"
        values = trace["states"][0]["values"]
        prestate = [values[name] for name in report["variables"]]
        assert prestate not in report["states"]
        external = tlc(args, module, trial_cfg, root, "violation")
        return {"removed": clauses[i], "action": trace["states"][1].get("action"),
                "hypothesis_replay": "valid", "original_init_replay": "invalid",
                "prestate_reachable": False, "damaged_replay": "invalid", "pinned_tlc": external}
    raise ValueError("no necessary learned clause was removed")


def prove(args, root, managers):
    module = root / "TwoPhase.tla"
    copyfile(args.benchmarks / module.name, module)
    if digest(module) != SOURCE:
        raise ValueError("unexpected source revision")
    original_cfg = (args.benchmarks / "TwoPhase.cfg").read_text()
    cfg = module.with_suffix(".cfg")
    cfg_text = original_cfg if managers == 3 else original_cfg.replace("{rm1,rm2,rm3}", "{rm1,rm2}")
    cfg.write_text(cfg_text)
    native, production = run_json([args.producer, module, cfg], root)
    names = [row["name"] for row in native["actions"]]
    partition, carrier = action_partition(args, (module, cfg), names, root)
    if len(names) != 7 or list(partition.values()).count(None) != 2 or len(carrier) != managers:
        raise ValueError("unexpected number of resolved branches")
    start = monotonic()
    clauses = learned(native["states"], native["variables"])
    synthesis_s = monotonic() - start
    artifact = {"manifest": {"source": digest(module), "config": digest(cfg), "actions": names,
                            "variables": native["variables"], "target": TARGET, "boundary": native["boundary"]},
                "conjunction": clauses, "carrier": managers}
    verify_manifest(artifact, native, module, cfg)
    deleted = deepcopy(artifact)
    deleted["manifest"]["actions"].pop()
    wrong = root / "WrongCarrier.cfg"
    wrong.write_text(original_cfg if managers == 2 else original_cfg.replace("{rm1,rm2,rm3}", "{rm1,rm2}"))
    opposite, _ = run_json([args.producer, module, wrong], root)
    for altered, bound, binding in ((deleted, native, cfg), (artifact, opposite, wrong)):
        try:
            verify_manifest(altered, bound, module, binding)
        except ValueError:
            pass
        else:
            raise ValueError("stale certificate accepted")

    original, core = run_json([args.dive, "check", module, "--config", cfg, "--store", "exact",
                               "--workers", "1", "--seed", "0", "--fp", "0", "--output", "json"], root)
    assert (native["expanded"], native["generated"]) == (
        original["statistics"]["distinct"], original["statistics"]["generated"])
    named, symbolic = run_json([args.dive, "check", module, "--config", cfg, "--strategy", "symbolic",
                               "--property-proof", TARGET, "--type-invariant", "TypeOK", "--output", "json"], root)
    assert named["proof"] == "inductive-safety"
    init = tlc(args, *write_trial(root, cfg_text, clauses, start="ProofInit", next_action="ProofHold"), root)
    implication = tlc(args, *write_trial(root, cfg_text, clauses, next_action="ProofHold"), root)
    closure = tlc(args, *write_trial(root, cfg_text, clauses), root)
    assert implication["distinct"] == native["expanded"]
    assert closure["generated"] == implication["distinct"] + native["generated"] - native["initial_generated"]
    deadlock = tlc(args, *write_trial(root, cfg_text, clauses, next_action="ProofHold", invariant="ProofDeadlock"), root)
    # The quantified branches cover all bindings of each RM under original Next.
    # Run each source disjunct in isolation as a second check on that partition.
    independent = []
    for action in native["actions"]:
        name = action["name"]
        application = partition[name]
        if application is None:
            selected = [(None, name, "", "")]
        else:
            selected = [(rm, f"{application}(ProofManager)", "CONSTANT ProofManager\n",
                         f"CONSTANT ProofManager = {rm}\n") for rm in carrier]
        counts = 0
        for manager, expression, declarations, binding in selected:
            definition = declarations + fr"Selected == /\ ~proofStep /\ {expression} /\ proofStep' = TRUE"
            row = tlc(args, *write_trial(root, cfg_text, clauses, next_action="Selected",
                                         extra=definition, extra_cfg=binding), root)
            counts += row["generated"] - implication["distinct"]
            independent.append(dict(name=name, binding=manager, **row))
        assert counts == action["generated"]
    control = ablation(args, root, cfg_text, clauses, native)

    # Read published helper definitions only after discovery and independent certification.
    pool_rows, pool_cost = run_json(
        [sys.executable, Path(__file__).resolve(), args.benchmarks, "--dive", args.dive,
         "--published-pool", str(managers)], root
    )
    named_support = {key: named[key] for key in ("transition_obligations", "closure_sweeps", "proof")}
    return {"cfg": cfg_text, "certificate": artifact, "discovery": dict(production, synthesis_s=synthesis_s),
            "native": {k: v for k, v in native.items() if k != "states"},
            "core": core, "symbolic": dict(symbolic, **named_support),
            "pinned_tlc": {"Init": init, "Target": implication, "Next": closure, "Deadlock": deadlock,
                           "actions": independent},
            "ablation": control, "wrong_carrier": "rejected", "missing_action": "rejected",
            "published_receive_abort_pool": dict(pool_cost, candidates=pool_rows)}


def published_pool(benchmarks, dive, carrier):
    source = benchmarks / "TwoPhase_IndProofs_nfm.tla"
    if digest(source) != "fbacfe5b8349100d4d59554205b0046f2fd4fb59b873dd7cbe4109fbfe20e23f":
        raise ValueError("unexpected upstream pool")
    pool = source.read_text().split("IndGlobal ==", 1)[1].split("\n\n", 1)[0]
    names = [name for name in re.findall(r"/\\ (H_\w+)", pool) if name != TARGET]
    if len(names) != 15:
        raise ValueError("unexpected upstream pool size")
    return [finite(dive, Path.cwd(), name, tuple(f"rm{i+1}" for i in range(carrier))) for name in names]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("benchmarks", type=lambda p: Path(p).resolve())
    parser.add_argument("--dive", type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--published-pool", type=int, choices=(2, 3))
    for flag in ("producer", "exporter", "tlc-jar"):
        parser.add_argument("--" + flag, type=lambda p: Path(p).resolve())
    parser.add_argument("--java", default="java")
    args = parser.parse_args()
    if args.published_pool:
        print(json.dumps(published_pool(args.benchmarks, args.dive, args.published_pool)))
        return
    if not all((args.producer, args.exporter, args.tlc_jar)):
        parser.error("certification requires --producer, --exporter and --tlc-jar")
    for managers in (2, 3):
        with TemporaryDirectory() as temporary:
            print(json.dumps(prove(args, Path(temporary), managers)), flush=True)


if __name__ == "__main__":
    main()
