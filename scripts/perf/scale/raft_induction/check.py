#!/usr/bin/env python3
"""Independently discharge the original AsyncRaft finite log induction obligations."""

import argparse
from functools import cache
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

import z3

from representation import representation
from source import Model
from values import con, dis, neg, require

sys.path.append(str(Path(__file__).resolve().parents[3] / "symbolic"))
from induction import write_atomic


TARGET = "H_PrimaryHasEntriesItCreated"
ORIGINAL_SOURCE = "7d739ad5f8637e974b57fabd83bd18f3b9a739c03c254af4fb087dd8ab5f2322"
FOLLOWER_MUTANT = "5dc0736072b5e660a3b3a01dafa5f4052d37f5618397677cfe8e45ea15c56767"
THREE_PENDING_MUTANT = (
    "190bc83fc3060c06bb94176e1b7e7ae2c80fda9d8c1cd7d557b8914324b9a775"
)
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
    return json.dumps(
        obj, sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()


def graph_path(root):
    path = root / GRAPH
    return (
        path
        if path.is_file()
        else root / "saved_results/AsyncRaft_PrimaryHasEntriesItCreated" / GRAPH
    )


def config(path, servers, max_messages=2, max_term=2, max_log=2):
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
        ("MaxTerm = 3", f"MaxTerm = {max_term}"),
        ("MaxLogLen = 3", f"MaxLogLen = {max_log}"),
        ("MaxMsgCount = 1000", f"MaxMsgCount = {max_messages}"),
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


def bind(
    source,
    exporter,
    size,
    stage,
    max_messages,
    max_term=2,
    max_log=2,
    supplied_cfg=None,
):
    graph = graph_path(source)
    graph_bytes = graph.read_bytes()
    pairs = json.loads(graph_bytes)["strengthening_conjuncts"]
    names = [name for name, _, _ in pairs]
    require(
        names and len(set([TARGET, *names])) == 1 + len(names),
        "duplicate or empty strengthening",
    )
    require(
        all(name.isidentifier() and name.isascii() for name in names),
        "invalid formula name",
    )
    source_bytes = (source / "AsyncRaft.tla").read_bytes()
    (stage / "AsyncRaft.tla").write_bytes(source_bytes)
    trial = stage / "RaftTrial.tla"
    trial.write_text(
        "---- MODULE RaftTrial ----\nEXTENDS AsyncRaft\n"
        + "".join(f"{name} == {formula}\n" for name, formula, _ in pairs)
        + "CandidatePackage ==\n"
        + "".join("  /\\ " + name + "\n" for name in [*names, TARGET])
        + "====\n"
    )
    cfg = trial.with_suffix(".cfg")
    if supplied_cfg is None:
        cfg.write_text(
            config(source / "AsyncRaft.cfg", size, max_messages, max_term, max_log)
        )
    else:
        cfg.write_bytes(supplied_cfg.read_bytes())
    run = subprocess.run([exporter, trial, cfg], capture_output=True, timeout=30)
    require(run.returncode == 0, run.stderr.decode()[-1200:])
    doc = json.loads(run.stdout)
    custom = {module["name"] for module in doc["ir"]["modules"] if not module["standard"]}
    require(custom == {"RaftTrial", "AsyncRaft"}, "unexpected module in the bound finite proof")
    for module in doc["ir"]["modules"]:
        if module["standard"]:
            local = source / (module["name"] + ".tla")
            require(not local.exists() and not local.is_symlink(),
                    "finite AsyncRaft proof refuses a local shadow of " + module["name"])
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
    constants = {
        d["name"]: doc["constants"][d["constant"]]
        for d in doc["ir"]["decls"]
        if d["constant"] is not None
    }
    servers = constants["Server"]["set"]
    size = len(servers)
    require(
        size in (2, 3) and servers == [{"model": f"n{i}"} for i in range(1, size + 1)],
        "unsupported labeled server carrier",
    )
    for name, value in constants.items():
        if name in ("MaxTerm", "MaxLogLen", "MaxMsgCount"):
            require(type(value) is int and value >= 0, "expected a nonnegative bound")
        elif name != "Server":
            require(value == {"model": name}, "unsupported symbolic constant binding")
    max_messages = constants["MaxMsgCount"]
    identity = dict(
        bound_ir=digest(canonical(bound)),
        graph=digest(graph_bytes),
        upstream_cfg=digest((source / "AsyncRaft.cfg").read_bytes()),
        config=digest(cfg.read_bytes()),
        source=digest(source_bytes),
        servers=size,
        max_messages=max_messages,
    )
    doc["checked_source_action_edits"] = action_source(source_bytes)
    return doc, [TARGET, *names], identity


def simplified(value):
    return z3.simplify(z3.BoolVal(value) if isinstance(value, bool) else value)


@cache
def term_identity(term):
    # Only this finite worker's first-order Int/Bool terms are admitted.
    # Shared-expression names emitted by sexpr() and fresh AST allocation
    # order are irrelevant. Reordering children is allowed only for the
    # indicated commutative operators over these exact sorts.
    require(z3.is_app(term) and term.sort().kind() in (z3.Z3_BOOL_SORT, z3.Z3_INT_SORT),
            "unsupported Z3 receipt atom")
    decl = term.decl()
    children = [term_identity(child) for child in term.children()]
    if decl.kind() in (z3.Z3_OP_AND, z3.Z3_OP_OR, z3.Z3_OP_EQ, z3.Z3_OP_ADD, z3.Z3_OP_MUL):
        children.sort()
    domain = [decl.domain(i).sexpr() for i in range(decl.arity())]
    native = z3.z3core
    symbol = native.Z3_get_decl_name(decl.ctx_ref(), decl.ast)
    kind = native.Z3_get_symbol_kind(decl.ctx_ref(), symbol)
    name = (native.Z3_get_symbol_int(decl.ctx_ref(), symbol) if kind == z3.Z3_INT_SYMBOL else
            native.Z3_get_symbol_string(decl.ctx_ref(), symbol))
    # Some internal names are reused for other signatures. Bind declarations,
    # argument and result sorts, and all native declaration parameters.
    return digest(canonical([decl.kind(), kind, name, domain, term.sort().sexpr(),
                             [str(param) for param in decl.params()], children,
                             str(term.as_long()) if z3.is_int_value(term) else None]))


def satisfiable(solver, condition, obligations=None, scopes=()):
    condition = simplified(condition)
    if z3.is_false(condition):
        return False
    def solve():
        solver.push()
        try:
            solver.add(condition)
            verdict = solver.check()
            if verdict == z3.unknown:
                raise ValueError("solver returned unknown: " + solver.reason_unknown())
            return verdict == z3.unsat
        finally:
            solver.pop()
    if obligations is None:
        return not solve()
    return not obligations.unsat([*scopes, term_identity(condition)], solve, remember_sat=True)


def discharge(solver, failure, description, obligations=None, scopes=()):
    require(not satisfiable(solver, failure, obligations, scopes), description)


def prove(doc, names, obligations=None, action_edits=None):
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
    base = digest(canonical([term_identity(term) for term in solver.assertions()])) if obligations else None
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
        key = [action, action_edits.get(action), base] if action_edits is not None and obligations else None
        evidence = obligations.group(key) if key is not None else None
        if evidence is not None:
            require(evidence.get('action') == action, 'cached action row names a different bound role')
            rows.append(evidence)
            continue
        model.state = before
        model.errors = []
        branches = list(model.branches(index, {}))
        discharge(solver, dis(*model.errors), action + " undefined source evaluation", obligations, (base,))
        feasible = boundary = 0
        for branch, (guard, after) in enumerate(branches):
            if not satisfiable(solver, guard, obligations, (base,)):
                continue
            feasible += 1
            require(after.keys() == before.keys(), action + " incomplete destination")
            solver.push()
            solver.add(guard)
            scope = (base, term_identity(guard)) if obligations else ()
            model.state = after
            model.errors = []
            keep = model.eval(constraint, {})
            boundary += int(satisfiable(solver, neg(keep), obligations, scope))
            label = f"{action} alternative {branch}"
            discharge(
                solver,
                neg(model.eval(defs[names[0]], {})),
                label + " destination safety", obligations, scope,
            )
            discharge(
                solver, con(keep, neg(domain(after))), label + " finite representation", obligations, scope,
            )
            for name in names[1:]:
                dest = simplified(model.eval(defs[name], {}))
                if not predicates[name].eq(dest):
                    discharge(solver, con(keep, neg(dest)), label + " fails " + name, obligations, scope)
            discharge(
                solver, dis(*model.errors), label + " undefined destination evaluation", obligations, scope,
            )
            solver.pop()
        evidence = dict(action=action, alternatives=len(branches), feasible=feasible,
                        crossing_constraint=boundary)
        rows.append(evidence)
        if key is not None:
            obligations.group_proved(key, evidence)
    term_identity.cache_clear()
    return dict(
        property=TARGET, predicates=len(names), initial=len(initial), actions=rows
    )


def action_source(source):
    """Recognize the prior type-audited source and one guarded-action edit."""
    original = b'ClientRequestAction == TRUE /\\ \\E i \\in Server : ClientRequest(i)'
    edited = original.replace(b'TRUE', b'FALSE', 1)
    physical = digest(source)
    if physical == ORIGINAL_SOURCE:
        return {}
    prefix, marker, comment = source.rpartition(b'\n\\*')
    if marker and b'\n' not in comment.rstrip(b'\r\n') and ORIGINAL_SOURCE in (digest(prefix), digest(prefix + b'\n')):
        return {}
    if source.count(edited) == 1 and digest(source.replace(edited, original, 1)) == ORIGINAL_SOURCE:
        return {'ClientRequestAction': 'disabled'}
    return None


def native_z3():
    """Locate the library mapped by this Python binding, or decline reuse."""
    try:
        pointer = z3.z3core.Z3_get_full_version.__defaults__[0].f
        libraries = [Path(value._name) for value in pointer._objects.values()
                     if hasattr(value, '_name')]
        if len(libraries) == 1 and libraries[0].is_absolute() and libraries[0].is_file():
            return libraries[0].resolve()
    except (AttributeError, IndexError, TypeError, OSError):
        pass
    return None


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("--exporter", required=True, type=Path)
    p.add_argument("--servers", type=int, choices=(2, 3))
    p.add_argument("--max-messages", type=int)
    p.add_argument("--max-term", type=int, choices=(2, 3))
    p.add_argument("--max-log-length", type=int, choices=(2, 3))
    p.add_argument(
        "--config",
        type=Path,
        help="bind these exact CFG bytes instead of generating study bounds",
    )
    p.add_argument(
        "--message-sets",
        action="store_true",
        help="prove over arbitrary finite sets of typed messages",
    )
    p.add_argument(
        "--jobs",
        type=int,
        default=1,
        help="concurrent action groups for --message-sets",
    )
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--save", type=Path)
    mode.add_argument("--verify", type=Path)
    p.add_argument("--obligation-cache", type=Path)
    p.add_argument("--rebind", action="store_true", help=argparse.SUPPRESS)
    args = p.parse_args()
    try:
        bounds = (args.servers, args.max_messages, args.max_term, args.max_log_length)
        require(
            args.config is None
            or all(v is None for v in bounds),
            "an explicit CFG takes the place of generated bounds",
        )
        require(
            args.config is not None or args.servers is not None,
            "provide --servers or --config",
        )
        require(
            1 <= args.jobs <= len(ACTIONS) and (args.message_sets or args.jobs == 1),
            "unsupported action worker count",
        )
        capacity = 2 if args.max_messages is None else args.max_messages
        require(
            capacity >= 0 and (args.message_sets or capacity in (2, 3)),
            "unsupported message capacity",
        )
        with tempfile.TemporaryDirectory() as tmp:
            doc, names, identity = bind(
                args.source,
                args.exporter.resolve(),
                args.servers,
                Path(tmp),
                capacity,
                args.max_term or 2,
                args.max_log_length or 2,
                args.config,
            )
            if not args.message_sets:
                require(identity["max_messages"] in (2, 3), "unsupported message capacity")
            if args.verify:
                require(args.verify.stat().st_size <= 1 << 20, "oversize certificate")
                prior = json.loads(args.verify.read_bytes())
                require(isinstance(prior, dict), "expected a certificate object")
            if args.verify:
                require(
                    canonical(prior.get("input")) == canonical(identity),
                    "different source, configuration, or proof graph",
                )
            edits = doc["checked_source_action_edits"]
            # Model atoms use negative integer tags. Only the reviewed, source-typed
            # expressions may use this translation. New input needs a type audit.
            require(
                (edits is not None or identity["source"] in (FOLLOWER_MUTANT, THREE_PENDING_MUTANT))
                and identity["graph"] == GRAPH_SHA
                and identity["upstream_cfg"] == UPSTREAM_CFG,
                "unsupported source, configuration or proof graph for this finite representation",
            )
            obligations = None
            if args.obligation_cache:
                require(not args.message_sets, "obligation cache uses the finite Z3 representation")
                from verified_obligations import VerifiedObligations
                library = native_z3()
                bindings = [doc["constants"], doc["overrides"], doc["assumptions"]]
                obligations = VerifiedObligations(args.obligation_cache,
                    ["asyncraft-z3-v1", ORIGINAL_SOURCE, identity["config"], identity["upstream_cfg"],
                     identity["graph"], bindings, z3.get_full_version()],
                    [args.exporter, *sorted(Path(__file__).parent.glob("*.py")),
                     *sorted(Path(z3.__file__).parent.glob("*.py")), library]) if library is not None else None
            if args.message_sets:
                from channel_proof import prove_channels

                result = dict(
                    format="asyncraft-membership-induction-v1",
                    input=identity,
                    **prove_channels(doc, names, args.jobs),
                    result="proved",
                )
            else:
                result = dict(
                    format="asyncraft-source-induction-v1",
                    input=identity,
                    **prove(doc, names, obligations, edits),
                    result="proved",
                )
            metrics = {}
            if args.message_sets:
                # Solver simplification can change the operational AST/query
                # counts without changing the mathematical obligations. Report
                # these measurements separately from the portable certificate.
                metrics["hypothesis_ast_nodes"] = result.pop("hypothesis_ast_nodes")
                metrics["solver_queries"] = {
                    row["action"]: row.pop("solver_queries")
                    for row in result["actions"]
                }
            if args.obligation_cache or args.rebind:
                with tempfile.TemporaryDirectory() as repeat:
                    _, _, rebound = bind(args.source, args.exporter.resolve(), args.servers,
                                         Path(repeat), capacity, args.max_term or 2,
                                         args.max_log_length or 2, args.config)
                require(identity == rebound, "bound source closure or CFG changed during induction")
            if args.verify:
                require(
                    canonical(prior) == canonical(result),
                    "receipt differs from independent action discharge",
                )
            else:
                write_atomic(args.save, canonical(result) + b"\n")
            if obligations:
                metrics["obligation_reuse"] = obligations.finish()
            print(
                json.dumps({**result, "solver_metrics": metrics} if metrics else result)
            )
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
