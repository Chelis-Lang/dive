"""Check configured AsyncRaft by stronger induction over arbitrary finite message sets."""

from concurrent.futures import ThreadPoolExecutor
import json
from pathlib import Path
import subprocess
import sys
import z3

from channels import ChannelModel, Channel
from check import ACTIONS, TARGET, simplified
from ground import Ground, visit
from representation import representation, sets
from values import con, dis, neg, require, Seq


def typed_wire(channel, record):
    """A generated record must be observable even outside the state constraint."""
    fields = channel.fields
    require(record.fields.keys() == fields.keys(), "unrepresentable raw record fields")
    tests = []
    for key, shape in fields.items():
        value = record.fields[key]
        if key == "mtype":
            expected = channel.field(shape, "wire_tag")
            tests.append(value == expected)
        elif shape == "bool":
            require(
                isinstance(value, bool) or z3.is_bool(value), "non-Boolean wire field"
            )
        elif shape == "entries":
            require(isinstance(value, Seq), "non-sequence payload")
            tests.extend((value.length >= 0, value.length <= min(1, len(value.cells))))
            require(
                all(type(v) is int or z3.is_int(v) for v in value.cells),
                "non-integer log entry",
            )
        else:
            require(type(value) is int or z3.is_int(value), "non-integer wire field")
    return con(*tests)


def state_constraints(model, doc, defs):
    require(
        doc["init"] == defs["Init"]
        and doc["next"] == defs["Next"]
        and doc["constraints"] == [defs["StateConstraint"]],
        "expected original Init, Next and StateConstraint",
    )
    require(
        doc["invariants"] == [[TARGET, defs[TARGET]]], "expected original log property"
    )
    tag, body = model.exprs[doc["constraints"][0]]
    require(
        tag == "Junction" and body["kind"] == "And" and len(body["items"]) == 6,
        "unsupported state constraint",
    )
    (limit,) = [
        d["constant"]
        for d in model.ir["decls"]
        if d["name"] == "MaxMsgCount" and d["constant"] is not None
    ]
    removed = []
    for index in body["items"][2:]:

        def standard(ref, name):
            tag, call = model.exprs[ref]
            require(
                tag == "Apply" and call["callee"] == {"Std": name}, "expected " + name
            )
            return [arg["Expr"]["expr"] for arg in call["args"]]

        lhs, rhs = standard(index, "NaturalsLeq")
        require(
            model.exprs[rhs] == ("Const", limit),
            "channel upper bound is not MaxMsgCount",
        )
        (ref,) = standard(lhs, "FiniteSetsCardinality")
        tag, var = model.exprs[ref]
        require(tag == "Var" and not var["primed"], "expected channel state variable")
        removed.append(model.names[var["var"]])
    require(tuple(removed) == sets, "unexpected constrained message sets")
    # The original six-way conjunction implies its first two conjuncts.
    # Each removed cardinality exists: the admitted record carrier is finite
    # and each raw image adds/deletes finite sets of records.
    return body["items"][:2]


def prepare(doc, names):
    model = ChannelModel(doc)
    before, domain = representation(model, Channel)
    defs = {
        d["name"]: d["body"]
        for d in model.defs
        if d["scope"] == "TopLevel" and d["source"] is None
    }
    bounds = state_constraints(model, doc, defs)

    def retained():
        return con(*(model.eval(index, {}) for index in bounds))

    model.state = {}
    initial = list(model.branches(doc["init"], {}))
    require(
        initial and all(g is True and st.keys() == before.keys() for g, st in initial),
        "incomplete original Init",
    )
    for _, st in initial:
        model.state = st
        init = [
            domain(st),
            model.eval(doc["constraints"][0], {}),
            *(model.eval(defs[n], {}) for n in names),
        ]
        require(
            z3.is_true(simplified(con(*init, neg(dis(*model.errors))))),
            "initiation or initial definedness",
        )
    model.state = before
    model.errors = []
    predicates = {n: simplified(model.eval(defs[n], {})) for n in names}
    # Dropping definedness from the hypothesis also weakens it. Definedness
    # still has to be proved for every guarded source and every raw image.
    base = [domain(before), retained(), *predicates.values()]
    tag, body = model.exprs[doc["next"]]
    require(tag == "Junction" and body["kind"] == "Or", "unsupported original Next")
    actions = {}
    for index in body["items"]:
        kind, ref = model.exprs[index]
        require(kind == "Def", "unsupported Next component")
        actions[model.defs[ref]["name"]] = index
    require(
        tuple(actions) == ACTIONS and len(body["items"]) == len(ACTIONS),
        "incomplete original Next",
    )
    roots = list(map(simplified, base))
    stats = dict(
        initial=len(initial),
        predicates=len(names),
        hypothesis_ast_nodes=len({n.get_id() for expr in roots for n in visit(expr)}),
    )
    return model, before, domain, defs, retained, predicates, actions, base, stats


def group(doc, names, selected):
    model, before, domain, defs, retained, predicates, actions, base, stats = prepare(
        doc, names
    )
    channels = {name: before[name] for name in sets}
    checker = Ground(channels, base)
    rows = []
    for name in selected:
        model.state = before
        model.errors = []
        branches = list(model.branches(actions[name], {}))
        count = checker.total
        checker.reject(dis(*model.errors), name + " undefined source")
        images = 0
        for ordinal, (guard, after) in enumerate(branches):
            if z3.is_false(simplified(guard)):
                continue
            images += 1
            require(after.keys() == before.keys(), "incomplete original action image")
            model.state = after
            model.errors = []
            keep = retained()
            label = f"{name} alternative {ordinal}"
            wires = [
                dis(neg(h), typed_wire(channels[k], r))
                for k in sets
                for h, r in after[k].new_records
            ]
            checker.reject(neg(con(*wires)), label + " raw record coverage", guard)
            checker.reject(
                neg(model.eval(defs[TARGET], {})),
                label + " raw destination safety",
                guard,
            )
            checker.reject(
                con(keep, neg(domain(after))), label + " retained representation", guard
            )
            for predicate in names[1:]:
                destination = simplified(model.eval(defs[predicate], {}))
                if not destination.eq(predicates[predicate]):
                    checker.reject(
                        con(keep, neg(destination)),
                        label + " preservation of " + predicate,
                        guard,
                    )
            checker.reject(
                dis(*model.errors), label + " undefined raw destination", guard
            )
        rows.append(
            dict(
                action=name,
                guarded_images=images,
                preservation_obligations=images * (len(names) - 1),
                raw_safety_obligations=images,
                raw_record_shape_obligations=images,
                solver_queries=checker.total - count,
            )
        )
    return stats, rows


def prove_channels(doc, names, jobs):
    assignments = [ACTIONS[i::jobs] for i in range(jobs)]
    if jobs == 1:
        results = [group(doc, names, ACTIONS)]
    else:
        # Use independent interpreters and pipes: each Z3 context belongs to a
        # single process, including on job runners without persistent POSIX IPC.
        def run_worker(actions):
            process = subprocess.run(
                [sys.executable, Path(__file__).resolve()],
                input=json.dumps([doc, names, actions]),
                capture_output=True,
                text=True,
                timeout=240,
            )
            if process.returncode != 0:
                raise ValueError(process.stderr.strip() or "action worker failed")
            return json.loads(process.stdout)

        with ThreadPoolExecutor(max_workers=jobs) as pool:
            results = list(pool.map(run_worker, assignments))
    stats = results[0][0]
    require(all(stat == stats for stat, _ in results), "inconsistent bound hypotheses")
    rows = {r["action"]: r for _, group_rows in results for r in group_rows}
    require(set(rows) == set(ACTIONS), "missing original action result")
    constants = {
        d["name"]: doc["constants"][d["constant"]]
        for d in doc["ir"]["decls"]
        if d["constant"] is not None
    }
    return dict(
        property=TARGET,
        encoding="extensional-message-membership-v1",
        max_term=constants["MaxTerm"],
        max_log_length=constants["MaxLogLen"],
        omitted_cardinality_conjuncts=list(sets),
        actions=[rows[n] for n in ACTIONS],
        **stats,
    )


if __name__ == "__main__":
    # Internal projected obligations only. The user-facing command independently
    # binds TLA+/CFG and validates coverage before it can issue any receipt.
    try:
        print(json.dumps(group(*json.load(sys.stdin))))
    except (ValueError, TypeError, KeyError, IndexError, z3.Z3Exception) as error:
        print(error, file=sys.stderr)
        sys.exit(2)
