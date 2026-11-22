#!/usr/bin/env python3
"""Independently discharge the original AsyncRaft finite log induction obligations."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import z3

from representation import representation
from source import Model
from values import con, dis, neg, require

TARGET = "H_PrimaryHasEntriesItCreated"
ORIGINAL_SOURCE = "7d739ad5f8637e974b57fabd83bd18f3b9a739c03c254af4fb087dd8ab5f2322"
FOLLOWER_MUTANT = "5dc0736072b5e660a3b3a01dafa5f4052d37f5618397677cfe8e45ea15c56767"
GRAPH_SHA = "c71a7d7e4f036d97025c0c6091c9fe0f6bfec146b9eceeca7c0511870d7d5d16"
UPSTREAM_CFG = "d1602e6bb793b9ea4968587e58ceedffffd6bd765a84b2eb349bf818ab0ad284"
GRAPH = "AsyncRaft_ind-proof-tree-sd4.proofgraph.json"
ACTIONS = (
    "RequestVoteAction",
    "UpdateTermAction",
    "HandleRequestVoteRequestAction",
    "HandleRequestVoteResponseAction",
    "BecomeLeaderAction",
    "ClientRequestAction",
    "AppendEntriesAction",
    "AcceptAppendEntriesRequestAppendAction",
    "AcceptAppendEntriesRequestLearnCommitAction",
    "HandleAppendEntriesResponseAction",
    "AdvanceCommitIndexAction",
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(obj):
    return json.dumps(obj, sort_keys=True, separators=(",", ":")).encode()


def graph_path(root):
    path = root / GRAPH
    return (
        path
        if path.is_file()
        else root / "saved_results/AsyncRaft_PrimaryHasEntriesItCreated" / GRAPH
    )


def config(path, servers):
    # Preserve all bindings from the pinned configuration except the explicit study bounds.
    text = path.read_text().split("CONSTANTS\n", 1)[1].split("\nCONSTRAINT", 1)[0]
    lines = list(
        dict.fromkeys(
            line.strip()
            for line in text.splitlines()
            if line.strip() and not line.lstrip().startswith(r"\*")
        )
    )
    for source, new in (
        ("MaxTerm = 3", "MaxTerm = 2"),
        ("MaxLogLen = 3", "MaxLogLen = 2"),
        ("MaxMsgCount = 1000", "MaxMsgCount = 2"),
        (
            "Server = {n1, n2, n3}",
            "Server = {n1, n2}" if servers == 2 else "Server = {n1, n2, n3}",
        ),
    ):
        require(source in lines, f"unexpected upstream setting {source}")
        lines[lines.index(source)] = new
    return (
        "CONSTANTS\n"
        + "\n".join(lines)
        + f"\nINIT Init\nNEXT Next\nCONSTRAINT StateConstraint\nINVARIANT {TARGET}\nCHECK_DEADLOCK FALSE\n"
    )


def bind(source, exporter, size, stage):
    graph = graph_path(source)
    pairs = json.loads(graph.read_bytes())["strengthening_conjuncts"]
    names = [name for name, _, _ in pairs]
    require(
        names and len(set([TARGET, *names])) == 1 + len(names),
        "duplicate or empty strengthening",
    )
    require(
        all(name.isidentifier() and name.isascii() for name in names),
        "invalid formula name",
    )
    shutil.copy(source / "AsyncRaft.tla", stage / "AsyncRaft.tla")
    trial = stage / "RaftTrial.tla"
    trial.write_text(
        "---- MODULE RaftTrial ----\nEXTENDS AsyncRaft\n"
        + "".join(f"{name} == {formula}\n" for name, formula, _ in pairs)
        + "CandidatePackage ==\n"
        + "".join("  /\\ " + name + "\n" for name in [*names, TARGET])
        + "====\n"
    )
    cfg = trial.with_suffix(".cfg")
    cfg.write_text(config(source / "AsyncRaft.cfg", size))
    run = subprocess.run([exporter, trial, cfg], capture_output=True, timeout=30)
    require(run.returncode == 0, run.stderr.decode()[-1200:])
    doc = json.loads(run.stdout)
    require(
        len(doc["constraints"]) == 1
        and len(doc["invariants"]) == 1
        and doc["invariants"][0][0] == TARGET,
        "expected one source constraint and the named invariant",
    )
    require(
        not any(
            doc[k] for k in ("assumptions", "overrides", "symmetry", "specification")
        ),
        "unsupported bound configuration",
    )
    source_files = [
        {k: v for k, v in f.items() if k != "display"} for f in doc["ir"]["files"]
    ]
    bound = {**doc, "ir": {**doc["ir"], "files": source_files}}
    identity = dict(
        bound_ir=digest(canonical(bound)),
        graph=digest(graph.read_bytes()),
        upstream_cfg=digest((source / "AsyncRaft.cfg").read_bytes()),
        config=digest(cfg.read_bytes()),
        source=digest((stage / "AsyncRaft.tla").read_bytes()),
        servers=size,
    )
    return doc, [TARGET, *names], identity


def simplified(value):
    return z3.simplify(z3.BoolVal(value) if isinstance(value, bool) else value)


def satisfiable(solver, condition):
    condition = simplified(condition)
    if z3.is_false(condition):
        return False
    solver.push()
    try:
        solver.add(condition)
        verdict = solver.check()
        if verdict == z3.unknown:
            raise ValueError("solver returned unknown: " + solver.reason_unknown())
        return verdict == z3.sat
    finally:
        solver.pop()


def discharge(solver, failure, description):
    require(not satisfiable(solver, failure), description)


def prove(doc, names):
    model = Model(doc)
    before, domain = representation(model)
    defs = {
        d["name"]: d["body"]
        for d in model.defs
        if d["scope"] == "TopLevel" and d["source"] is None
    }
    require(
        defs[names[0]] == doc["invariants"][0][1], "configured safety operator mismatch"
    )
    require(
        [model.exprs[a][1] for a in model.exprs[defs["CandidatePackage"]][1]["items"]]
        == [
            next(
                i
                for i, d in enumerate(model.defs)
                if d["name"] == n and d["body"] == defs[n]
            )
            for n in [*names[1:], names[0]]
        ],
        "candidate wrapper differs from selected graph formulas",
    )
    constraint = doc["constraints"][0]
    solver = z3.Solver()
    solver.set(timeout=30000)
    model.state = before
    predicates = {name: simplified(model.eval(defs[name], {})) for name in names}
    keep = model.eval(constraint, {})
    # Definedness is a conjunct of the inductive strengthening. Initiation and
    # all retained images below must establish it, independent of this assumption.
    solver.add(domain(before), keep, neg(dis(*model.errors)), *predicates.values())
    require(satisfiable(solver, True), "empty constrained induction hypothesis")
    model.state = {}
    model.errors = []
    initial = list(model.branches(doc["init"], {}))
    require(
        initial and all(g is True and st.keys() == before.keys() for g, st in initial),
        "unsupported initial relation",
    )
    for _, state in initial:
        model.state = state
        checks = [
            domain(state),
            model.eval(constraint, {}),
            *(model.eval(defs[n], {}) for n in names),
        ]
        require(
            z3.is_true(simplified(con(*checks, neg(dis(*model.errors))))),
            "initial inclusion or definedness",
        )
    tag, body = model.exprs[doc["next"]]
    require(tag == "Junction" and body["kind"] == "Or", "unsupported Next")
    actions = []
    for index in body["items"]:
        kind, ref = model.exprs[index]
        require(kind == "Def", "unsupported action root")
        actions.append((model.defs[ref]["name"], index))
    require(
        tuple(n for n, _ in actions) == ACTIONS, "incomplete original action coverage"
    )
    rows = []
    for action, index in actions:
        model.state = before
        model.errors = []
        branches = list(model.branches(index, {}))
        discharge(solver, dis(*model.errors), action + " undefined source evaluation")
        feasible = boundary = 0
        for branch, (guard, after) in enumerate(branches):
            if not satisfiable(solver, guard):
                continue
            feasible += 1
            require(after.keys() == before.keys(), action + " incomplete destination")
            solver.push()
            solver.add(guard)
            model.state = after
            model.errors = []
            keep = model.eval(constraint, {})
            boundary += int(satisfiable(solver, neg(keep)))
            label = f"{action} alternative {branch}"
            discharge(
                solver,
                neg(model.eval(defs[names[0]], {})),
                label + " destination safety",
            )
            discharge(
                solver, con(keep, neg(domain(after))), label + " finite representation"
            )
            for name in names[1:]:
                dest = simplified(model.eval(defs[name], {}))
                if not predicates[name].eq(dest):
                    discharge(solver, con(keep, neg(dest)), label + " fails " + name)
            discharge(
                solver, dis(*model.errors), label + " undefined destination evaluation"
            )
            solver.pop()
        rows.append(
            dict(
                action=action,
                alternatives=len(branches),
                feasible=feasible,
                crossing_constraint=boundary,
            )
        )
    return dict(
        property=TARGET, predicates=len(names), initial=len(initial), actions=rows
    )


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("--exporter", required=True, type=Path)
    p.add_argument("--servers", required=True, type=int, choices=(2, 3))
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--save", type=Path)
    mode.add_argument("--verify", type=Path)
    args = p.parse_args()
    try:
        with tempfile.TemporaryDirectory() as tmp:
            doc, names, identity = bind(
                args.source, args.exporter.resolve(), args.servers, Path(tmp)
            )
            prior = json.loads(args.verify.read_bytes()) if args.verify else None
            if args.verify:
                require(
                    prior.get("input") == identity,
                    "different source, configuration, or proof graph",
                )
            # Model atoms use negative integer tags. Only the reviewed, source-typed
            # expressions may use this translation. New input needs a type audit.
            require(
                identity["source"] in (ORIGINAL_SOURCE, FOLLOWER_MUTANT)
                and identity["graph"] == GRAPH_SHA
                and identity["upstream_cfg"] == UPSTREAM_CFG,
                "unsupported source, configuration or proof graph for this finite representation",
            )
            result = dict(
                format="asyncraft-source-induction-v1",
                input=identity,
                **prove(doc, names),
                result="proved",
            )
            if args.verify:
                require(
                    prior == result, "receipt differs from independent action discharge"
                )
            else:
                args.save.write_bytes(canonical(result) + b"\n")
            print(json.dumps(result))
            return 0
    except (
        ValueError,
        KeyError,
        TypeError,
        IndexError,
        OSError,
        z3.Z3Exception,
        subprocess.TimeoutExpired,
    ) as err:
        print(json.dumps({"result": "inconclusive", "reason": str(err)}))
        return 2


if __name__ == "__main__":
    sys.exit(main())
