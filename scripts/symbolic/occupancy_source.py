"""Admit bound source for exact labeled reachability of anonymous finite status tables."""

from collections import Counter
from itertools import product


class Unsupported(ValueError):
    """The selected source requires a feature outside the occupancy fragment."""


def require(condition, reason):
    if not condition:
        raise Unsupported(reason)


OWNER = -1


def holds(predicate, present, scope):
    kind, *args = predicate
    if kind == "bool":
        return args[0]
    if kind == "member":
        slot, allowed = args
        return scope[slot] in allowed
    if kind == "not":
        return not holds(args[0], present, scope)
    if kind in ("and", "or"):
        combine = all if kind == "and" else any
        return combine(holds(term, present, scope) for term in args)
    slots, body = args
    combine = all if kind == "every" else any
    return combine(holds(body, present, {**scope, **dict(zip(slots, values))})
                   for values in product(present, repeat=len(slots)))


class PopulationSource:
    """An admitted action changes one anonymous owner and observes only local statuses."""

    def __init__(self, doc, type_name):
        require(doc["format"] == "dive-symbolic-input-v2", "bound exporter format")
        require(doc["symmetry"] is None, "configured symmetry changes labeled count scope")
        require(not doc["constraints"], "state constraint")
        require(not doc["assumptions"], "ASSUME")
        require(not doc["overrides"], "operator override")
        self.ir = doc["ir"]
        self.visits = 0
        self.next_slot = 0
        require(len(self.ir["vars"]) == 1, "expected one anonymous state table")
        self.name = self.ir["decls"][self.ir["vars"][0]]["name"]
        configured = doc["invariants"]
        matches = [root for name, root in configured if name == type_name]
        require(len(matches) == 1, "expected a configured table type invariant named " + type_name)
        self.read_type(matches[0], doc["constants"])
        if doc["specification"] is not None:
            self.check_specification(doc)
        self.initial = self.initial_statuses(doc["init"])
        self.rules = self.actions(doc["next"], {}, False)
        require(self.rules and len(self.rules) <= 256, "expected at most 256 owner action branches")
        self.invariants = [name for name, _ in configured]
        predicates = [(name, self.predicate(root, {})) for name, root in configured if name != type_name]
        # Every admitted assignment preserves the table type by construction.
        self.tables = {}
        self.violations = {}
        for mask in range(1 << len(self.statuses)):
            present = tuple(i for i in range(len(self.statuses)) if mask & (1 << i))
            moves = Counter()
            for own, guards, target in self.rules:
                for src in present if own else (None,):
                    if all(holds(guard, present, {OWNER: src}) for guard in guards):
                        moves[src, src if target is None else target] += 1
            self.tables[mask] = [(a, b, count) for (a, b), count in moves.items()]
            bad = next((name for name, pred in predicates if not holds(pred, present, {})), None)
            if bad is not None:
                self.violations[mask] = {"kind": "invariant", "name": bad}
            elif doc["check_deadlock"] and not moves:
                self.violations[mask] = {"kind": "deadlock"}

    def node(self, index):
        self.visits += 1
        require(self.visits <= 100_000, "bound expression expansion limit")
        node = self.ir["exprs"][index]["expr"]
        return next(iter(node.items())) if isinstance(node, dict) else (node, None)

    def local(self, index, env):
        kind, local = self.node(index)
        require(kind == "Local" and local["up"] == 0 and local["slot"] in env,
                "owner identity must be a currently bound table index")
        return env[local["slot"]]

    def resolve(self, index, env):
        seen = set()
        while True:
            kind, value = self.node(index)
            if kind not in ("Def", "Apply"):
                return index, kind, value, env
            if kind == "Def":
                callee, args = value, []
            else:
                require("Def" in value["callee"], "standard or dynamic operator")
                callee, args = value["callee"]["Def"], value["args"]
            definition = self.ir["defs"][callee]
            require(callee not in seen and not definition["recursive"] and not definition["std"]
                    and definition["scope"] == "TopLevel" and definition["source"] is None,
                    "recursive, local, standard or instantiated operator " + definition["name"])
            seen.add(callee)
            require(len(args) == len(definition["params"])
                    and all(p["arity"] == 0 for p in definition["params"]), "operator parameter signature")
            new_env = {}
            for pos, arg in enumerate(args):
                require(set(arg) == {"Expr"} and arg["Expr"]["pass"] == "Strict", "non-value argument")
                new_env[pos] = self.local(arg["Expr"]["expr"], env)
            index, env = definition["body"], new_env

    def expect(self, index, kind):
        actual, value = self.node(index)
        require(actual == kind, f"expected {kind}, found {actual}")
        return value

    def variable(self, index, primed):
        return self.node(index) == ("Var", {"primed": primed, "var": 0})

    def same_domain(self, index):
        return self.node(index) == ("Const", self.domain)

    def read_type(self, root, constants):
        _, kind, expr, _ = self.resolve(root, {})
        require(kind == "Builtin" and expr["op"] == "In", "expected table type membership")
        table, space = expr["args"]
        require(self.variable(table, False), "expected the unprimed state table")
        space = self.expect(space, "FcnSet")
        self.domain = self.expect(space["domain"], "Const")
        ids = constants[self.domain]
        require(isinstance(ids, dict) and set(ids) == {"set"}, "expected a finite configured owner set")
        members = ids["set"]
        require(all(isinstance(x, dict) and set(x) == {"model"} and "_" not in x["model"] for x in members),
                "expected untyped model-value owners")
        self.ids = [x["model"] for x in members]
        require(len(self.ids) == len(set(self.ids)), "owner uniqueness")
        self.statuses = [self.expect(item, "Str") for item in self.expect(space["range"], "SetEnum")]
        require(1 <= len(self.statuses) <= 8 and len(self.statuses) == len(set(self.statuses)),
                "expected one to eight distinct literal string statuses")

    def status(self, index):
        value = self.expect(index, "Str")
        require(value in self.statuses, "literal outside the declared status domain")
        return self.statuses.index(value)

    def slots(self, bounds):
        require(len(bounds) == 1, "expected one group of owner binders")
        bound, = bounds
        require(not bound["tuple"] and bound["domain"] is not None and self.same_domain(bound["domain"]),
                "binder must use the complete owner domain")
        slots = bound["vars"]
        require(1 <= len(slots) <= 2 and len(set(slots)) == len(slots), "owner binder arity")
        return slots

    def check_specification(self, doc):
        defs = [d for d in self.ir["defs"] if d["name"] == doc["specification"] and d["scope"] == "TopLevel"]
        require(len(defs) == 1, "ambiguous specification")
        body = self.expect(defs[0]["body"], "Junction")
        require(body["kind"] == "And" and len(body["items"]) == 2, "expected Init and always Next")
        init, temporal = body["items"]
        require(self.resolve(init, {})[0] == self.resolve(doc["init"], {})[0], "specification Init")
        step = self.expect(self.expect(temporal, "Always"), "StepBox")
        require(self.resolve(step["action"], {})[0] == self.resolve(doc["next"], {})[0], "specification Next")
        sub, kind, value, _ = self.resolve(step["sub"], {})
        if kind == "Tuple" and len(value) == 1:
            sub, _, _, _ = self.resolve(value[0], {})
        require(self.variable(sub, False), "stuttering subscript must contain the state table")

    def initial_statuses(self, root):
        _, kind, value, _ = self.resolve(root, {})
        if kind == "Junction" and value["kind"] == "Or":
            return [status for term in value["items"] for status in self.initial_statuses(term)]
        require(kind == "Builtin" and value["op"] == "Eq", "expected a uniform initial assignment")
        left, right = value["args"]
        require(self.variable(left, False), "expected an initial table assignment")
        constructor = self.expect(right, "FcnCons")
        require(len(self.slots(constructor["bounds"])) == 1, "initial table constructor arity")
        return [self.status(constructor["body"])]

    def predicate(self, index, env, *, guard=False, nested=False):
        _, kind, expr, env = self.resolve(index, env)

        def child(e):
            return self.predicate(e, env, guard=guard, nested=nested)

        if kind == "Bool":
            return ("bool", expr)
        if kind == "Not":
            return ("not", child(expr))
        if kind == "Junction":
            require(expr["kind"] in ("And", "Or"), "Boolean junction")
            require(not guard or expr["kind"] != "Or", "disjunctive action guard can carry witnesses")
            return (expr["kind"].lower(), *(child(e) for e in expr["items"]))
        if kind == "Quant":
            require(not nested and expr["kind"] in ("Forall", "Exists"), "nested or unsupported quantifier")
            require(not guard or expr["kind"] == "Forall", "existential action guard can carry witnesses")
            declared = self.slots(expr["bounds"])
            tokens = tuple(range(self.next_slot, self.next_slot + len(declared)))
            self.next_slot += len(tokens)
            body = self.predicate(expr["body"], {**env, **dict(zip(declared, tokens))},
                                  guard=guard, nested=True)
            return ("every" if expr["kind"] == "Forall" else "some", tokens, body)
        require(kind == "Builtin" and expr["op"] in ("Eq", "Neq", "In", "NotIn"), "non-status predicate " + kind)
        left, right = expr["args"]
        op = expr["op"]
        if op in ("Eq", "Neq") and self.node(left)[0] == "Str":
            left, right = right, left
        field = self.expect(left, "FcnApply")
        require(self.variable(field["fcn"], False), "expected an unprimed status lookup")
        slot = self.local(field["arg"], env)
        terms = self.expect(right, "SetEnum") if op in ("In", "NotIn") else [right]
        pred = ("member", slot, frozenset(self.status(term) for term in terms))
        return ("not", pred) if op in ("Neq", "NotIn") else pred

    def actions(self, root, env, owner):
        index, kind, expr, env = self.resolve(root, env)
        if kind == "Quant":
            require(not owner and expr["kind"] == "Exists", "expected a single action owner")
            slots = self.slots(expr["bounds"])
            require(len(slots) == 1, "multiple action owners")
            return self.actions(expr["body"], {**env, slots[0]: OWNER}, True)
        if kind == "Junction" and expr["kind"] == "Or":
            return [rule for item in expr["items"] for rule in self.actions(item, env, owner)]
        guards = []
        targets = []

        def conjunct(node, scope):
            original, tag, term, scope = self.resolve(node, scope)
            if tag == "Junction" and term["kind"] == "And" and self.ir["exprs"][original]["level"] == "Action":
                for item in term["items"]:
                    conjunct(item, scope)
            elif tag == "Unchanged":
                if self.node(term)[0] == "Tuple":
                    fields = self.expect(term, "Tuple")
                    require(len(fields) == 1, "UNCHANGED subscript")
                    term, = fields
                require(self.variable(term, False), "UNCHANGED must name the status table")
                targets.append(None)
            elif tag == "Builtin" and term["op"] == "Eq" and self.variable(term["args"][0], True):
                right = term["args"][1]
                if self.variable(right, False):
                    targets.append(None)
                    return
                update = self.expect(right, "Except")
                require(owner and self.variable(update["base"], False) and len(update["updates"]) == 1,
                        "expected a single owner update to its original table")
                change, = update["updates"]
                require(len(change["path"]) == 1 and self.local(change["path"][0], scope) == OWNER,
                        "status update must affect the bound owner")
                targets.append(self.status(change["value"]))
            else:
                guards.append(self.predicate(original, scope, guard=True))

        conjunct(index, env)
        require(len(targets) == 1, "every action branch must assign the complete table once")
        return [(owner, guards, targets[0])]
