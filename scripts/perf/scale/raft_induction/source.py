"""Evaluate the finite source expressions admitted by the pinned Raft induction panel."""

import itertools
import operator

import z3 as z

from values import (
    require,
    Rec,
    Seq,
    Set,
    Tab,
    card,
    con,
    concrete,
    dis,
    eq,
    ite,
    member,
    neg,
    select,
)


class Model:
    def __init__(self, doc):
        self.doc = doc
        self.ir = doc["ir"]
        self.defs = self.ir["defs"]
        self.exprs = [
            list(node["expr"].items())[0]
            if isinstance(node["expr"], dict)
            else (node["expr"], None)
            for node in self.ir["exprs"]
        ]
        self.names = [self.ir["decls"][j]["name"] for j in self.ir["vars"]]
        self.symbols = {}
        self.constants = [self.lit(node) for node in doc["constants"]]
        self.errors = []
        self.state = {}

    def lit(self, node):
        if isinstance(node, (int, bool)):
            return node
        if "model" in node:
            return self.symbols.setdefault(node["model"], -1 - len(self.symbols))
        if "set" in node:
            return Set([(True, self.lit(v)) for v in node["set"]])
        raise ValueError(node)

    def valid(self, when, condition):
        if condition is not True and not z.is_true(condition):
            bad = con(when, neg(condition))
            if bad is not False:
                self.errors.append(bad)

    def scopes(self, bounds, environment, path_guard):
        """Expand finite binders and retain the membership guard of each binding."""
        items = [(path_guard, environment)]
        for b in bounds:
            require(
                not b["tuple"] and b["domain"] is not None,
                "bounded non-tuple binder required",
            )
            out = []
            for guard, env in items:
                domain = self.eval(b["domain"], env, guard)
                for choices in itertools.product(domain.cells, repeat=len(b["vars"])):
                    terms = [node[0] for node in choices]
                    variables = [node[1] for node in choices]
                    active = con(guard, *terms)
                    if active is not False:
                        out.append(
                            (
                                active,
                                {**env, **dict(zip(b["vars"], variables, strict=True))},
                            )
                        )
            items = out
        return items

    def call(self, tag, node, environment, path_guard):
        index = node if tag == "Def" else node["callee"]["Def"]
        definition = self.defs[index]
        if isinstance(definition["scope"], dict):
            raise ValueError(
                ("unexpected local call", definition["name"], definition["scope"])
            )
        require(
            definition["scope"] == "TopLevel"
            and definition["source"] is None
            and (not definition["recursive"])
            and (not definition["std"]),
            "expected a nonrecursive source operator",
        )
        args = [] if tag == "Def" else [a["Expr"]["expr"] for a in node["args"]]
        require(
            len(args) == len(definition["params"]),
            "operator arity mismatch",
        )
        return definition["body"], {
            j: self.eval(a, environment, path_guard) for j, a in enumerate(args)
        }

    def let(self, node, environment, path_guard):
        environment = dict(environment)
        for index in node["defs"]:
            definition = self.defs[index]
            binding = definition["scope"]["LetValue"]
            require(
                not definition["params"] and (not definition["recursive"]),
                "unsupported local function",
            )
            environment[binding["slot"]] = (self, definition["body"], dict(environment))
        return environment

    def eval(self, index, environment, path_guard=True):
        """Translate an expression and collect undefined reads on its evaluation path."""
        kind, node = self.exprs[index]
        evaluate = lambda a, when=path_guard: self.eval(a, environment, when)
        if kind == "Num":
            return int(node)
        if kind == "Bool":
            return node
        if kind == "Const":
            return self.constants[node]
        if kind == "Local":
            require(node["up"] == 0, "nested local capture")
            v = environment[node["slot"]]
            return (
                self.eval(v[1], v[2], path_guard)
                if isinstance(v, tuple) and v[0] is self
                else v
            )
        if kind == "Var":
            require(not node["primed"], "primed variable in value expression")
            return self.state[self.names[node["var"]]]
        if kind == "Def" or (kind == "Apply" and "Def" in node["callee"]):
            body, env = self.call(kind, node, environment, path_guard)
            return self.eval(body, env, path_guard)
        if kind == "Let":
            return self.eval(
                node["body"], self.let(node, environment, path_guard), path_guard
            )
        if kind == "Not":
            return neg(evaluate(node))
        if kind == "Implies":
            a = evaluate(node[0])
            return dis(neg(a), evaluate(node[1], con(path_guard, a)))
        if kind == "Junction":
            is_and = node["kind"] == "And"
            val = is_and
            for item in node["items"]:
                v = evaluate(item, con(path_guard, val if is_and else neg(val)))
                val = con(val, v) if is_and else dis(val, v)
            return val
        if kind == "If":
            cond = evaluate(node["cond"])
            return ite(
                cond,
                evaluate(node["then"], con(path_guard, cond)),
                evaluate(node["other"], con(path_guard, neg(cond))),
            )
        if kind == "Quant":
            choices = self.scopes(node["bounds"], environment, True)
            k = node["kind"]
            if k == "Choose":
                require(
                    len(node["bounds"]) == 1 and len(node["bounds"][0]["vars"]) == 1,
                    "single-variable CHOOSE required",
                )
                slot = node["bounds"][0]["vars"][0]
                result = 0
                found = False
                for active, scope in reversed(choices):
                    ok = con(
                        active, self.eval(node["body"], scope, con(path_guard, active))
                    )
                    self.valid(
                        path_guard, dis(neg(con(found, ok)), eq(result, scope[slot]))
                    )
                    result = ite(ok, scope[slot], result)
                    found = dis(found, ok)
                self.valid(path_guard, found)
                return result
            require(
                k in ("Forall", "Exists"),
                "unsupported quantifier",
            )
            cond = [
                (active, self.eval(node["body"], scope, con(path_guard, active)))
                for active, scope in choices
            ]
            return (
                con(*(dis(neg(h), v) for h, v in cond))
                if k == "Forall"
                else dis(*(con(h, v) for h, v in cond))
            )
        if kind == "FcnApply":
            f = evaluate(node["fcn"])
            k = evaluate(node["arg"])
            if isinstance(f, Tab):
                self.valid(path_guard, dis(*(eq(k, index) for index in f.keys)))
                return select(f.keys, f.cells, k)
            if isinstance(f, Seq):
                self.valid(path_guard, con(k >= 1, k <= f.length))
                return (
                    select(tuple(range(1, len(f.cells) + 1)), f.cells, k)
                    if f.cells
                    else 0
                )
            raise ValueError(("apply", type(f)))
        if kind == "RecordSelect":
            return evaluate(node["record"]).fields[node["field"]]
        if kind == "Record":
            return Rec({v["name"]: evaluate(v["value"]) for v in node})
        if kind == "Tuple":
            return Seq(len(node), [evaluate(a) for a in node])
        if kind == "SetEnum":
            return Set([(True, evaluate(a)) for a in node])
        if kind == "SetMap":
            return Set(
                [
                    (h, self.eval(node["expr"], env, con(path_guard, h)))
                    for h, env in self.scopes(node["bounds"], environment, True)
                ]
            )
        if kind == "SetFilter":
            b = node["bound"]
            require(
                len(b["vars"]) == 1 and (not b["tuple"]),
                "unsupported set filter binder",
            )
            return Set(
                [
                    (
                        con(
                            h,
                            self.eval(
                                node["pred"],
                                {**environment, b["vars"][0]: v},
                                con(path_guard, h),
                            ),
                        ),
                        v,
                    )
                    for h, v in evaluate(b["domain"]).cells
                ]
            )
        if kind == "FcnCons":
            (b,) = node["bounds"]
            (slot,) = b["vars"]
            keys = concrete(evaluate(b["domain"]))
            return Tab(
                keys,
                tuple(
                    self.eval(node["body"], {**environment, slot: key}, path_guard)
                    for key in keys
                ),
            )
        if kind == "Except":
            f = evaluate(node["base"])
            (upd,) = node["updates"]
            ks = [evaluate(a) for a in upd["path"]]
            val = evaluate(upd["value"])

            def replace(f, level):
                if level == len(ks):
                    return val
                require(isinstance(f, Tab), "EXCEPT requires a finite function")
                # EXCEPT on missing keys has no effect.
                return Tab(
                    f.keys,
                    tuple(
                        ite(eq(ks[level], k), replace(old, level + 1), old)
                        for k, old in zip(f.keys, f.cells, strict=True)
                    ),
                )

            return replace(f, 0)
        if kind == "Builtin":
            op = node["op"]
            a, *more = map(evaluate, node["args"])
            b = more[0] if more else None
            if op in ("Eq", "Neq"):
                return eq(a, b) if op == "Eq" else neg(eq(a, b))
            if op in ("In", "NotIn"):
                return member(a, b) if op == "In" else neg(member(a, b))
            if op == "Domain":
                if isinstance(a, Seq):
                    return Set(
                        [
                            (a.length >= index, index)
                            for index in range(1, len(a.cells) + 1)
                        ]
                    )
                require(isinstance(a, Tab), "DOMAIN requires a function or sequence")
                return Set([(True, k) for k in a.keys])
            if op == "Cup":
                return Set(a.cells + b.cells)
            if op in ("Cap", "SetMinus"):
                return Set(
                    [
                        (con(h, member(v, b) if op == "Cap" else neg(member(v, b))), v)
                        for h, v in a.cells
                    ]
                )
            if op == "PowerSet":
                keys = concrete(a)
                return Set(
                    [
                        (True, Set([(True, v) for v in variables]))
                        for index in range(len(keys) + 1)
                        for variables in itertools.combinations(keys, index)
                    ]
                )
            raise ValueError(("builtin", op))
        if kind == "Apply":
            op = node["callee"]["Std"]
            args = [evaluate(a["Expr"]["expr"]) for a in node["args"]]
            if op == "SequencesLen":
                return args[0].length
            if op == "SequencesAppend":
                s, v = args
                return Seq(
                    s.length + 1,
                    [
                        ite(s.length == index, v, old)
                        for index, old in enumerate(s.cells)
                    ]
                    + [v],
                )
            if op == "SequencesSubSeq":
                s, lo, hi = args
                length = ite(lo > hi, 0, hi - lo + 1)
                self.valid(path_guard, dis(lo > hi, con(lo >= 1, hi <= s.length)))
                return Seq(
                    length,
                    [
                        select(tuple(range(1, len(s.cells) + 1)), s.cells, lo + index)
                        for index in range(len(s.cells))
                    ],
                )
            if op == "FiniteSetsCardinality":
                return card(args[0])
            op = op.replace("Integers", "Naturals")
            a, b = args
            if op == "NaturalsDotDot":
                require(
                    type(a) is int and type(b) is int,
                    ("interval", a, b, self.ir["exprs"][index]["origin"]),
                )
                return Set([(True, index) for index in range(a, b + 1)])
            return {
                "NaturalsPlus": operator.add,
                "NaturalsMinus": operator.evaluate,
                "NaturalsTimes": operator.mul,
                "NaturalsGeq": operator.ge,
                "NaturalsGt": operator.gt,
                "NaturalsLeq": operator.le,
                "NaturalsLt": operator.lt,
            }[op](a, b)
        raise ValueError(("expression", kind, node))

    def branches(self, index, environment, path_guard=True, writes=None):
        """Expand original quantifiers/disjunctions into guarded simultaneous writes."""
        writes = {} if writes is None else writes
        kind, node = self.exprs[index]
        if kind == "Def" or kind == "Apply" and "Def" in node["callee"]:
            a, env = self.call(kind, node, environment, path_guard)
            yield from self.branches(a, env, path_guard, writes)
        elif kind == "Quant" and node["kind"] == "Exists":
            for h, env in self.scopes(node["bounds"], environment, path_guard):
                yield from self.branches(node["body"], env, h, writes)
        elif kind == "Let":
            yield from self.branches(
                node["body"],
                self.let(node, environment, path_guard),
                path_guard,
                writes,
            )
        elif kind == "Junction" and node["kind"] == "Or":
            for a in node["items"]:
                yield from self.branches(a, environment, path_guard, writes)
        elif kind == "Junction":
            rows = [(path_guard, writes)]
            for a in node["items"]:
                rows = [
                    updated
                    for prev, writes in rows
                    for updated in self.branches(a, environment, prev, writes)
                    if updated[0] is not False
                ]
            yield from rows
        elif kind == "Unchanged":

            def variables(a):
                k, v = self.exprs[a]
                if k == "Def":
                    yield from variables(self.defs[v]["body"])
                elif k == "Tuple":
                    for m in v:
                        yield from variables(m)
                else:
                    require(
                        k == "Var" and (not v["primed"]),
                        "UNCHANGED expects source variables",
                    )
                    yield self.names[v["var"]]

            updated = dict(writes)
            for name in variables(node):
                if name in updated:
                    path_guard = con(path_guard, eq(updated[name], self.state[name]))
                updated[name] = self.state[name]
            yield path_guard, updated
        elif (
            kind == "Builtin"
            and node["op"] == "Eq"
            and self.exprs[node["args"][0]][0] == "Var"
            and (self.exprs[node["args"][0]][1]["primed"] or not self.state)
        ):
            name = self.names[self.exprs[node["args"][0]][1]["var"]]
            v = self.eval(node["args"][1], environment, path_guard)
            yield (
                (
                    con(path_guard, eq(writes[name], v))
                    if name in writes
                    else path_guard
                ),
                {**writes, name: v},
            )
        else:
            yield con(path_guard, self.eval(index, environment, path_guard)), writes
