"""Finite alphabets for whole record, set, string and integer state values."""

import itertools
from math import prod

from dd import cudd
from engine import SourceModel, Unsupported, require, unpack


def as_set(value):
    require(isinstance(value, frozenset), "expected a finite set")
    return value


def union_members(sets):
    return frozenset().union(*(as_set(member) for member in as_set(sets)))


def powerset(domain):
    require(isinstance(domain, frozenset), "SUBSET of a non-set")
    require(len(domain) <= 16, "whole-value powerset exceeds 65536 members")
    return frozenset(
        frozenset(row)
        for size in range(len(domain) + 1)
        for row in itertools.combinations(domain, size)
    )


def order(value):
    if isinstance(value, frozenset):
        return ("set", tuple(sorted(map(order, value))))
    if isinstance(value, tuple):
        return ("record", tuple((field, order(item)) for field, item in value))
    return (type(value).__name__, value)


def supported_configuration(document):
    return document["format"] == "dive-symbolic-input-v2" and not any(
        document[key]
        for key in ("specification", "constraints", "overrides", "assumptions", "symmetry", "constants")
    )


def whole_record_layout(document, expr):
    """Select only a layout that binds an unprimed variable to a whole record set."""

    if not supported_configuration(document):
        return False
    ir = document["ir"]

    def node(index):
        return unpack(ir["exprs"][index]["expr"])

    pending = [expr]
    visited = set()
    while pending:
        index = pending.pop()
        if index in visited:
            continue
        visited.add(index)
        tag, term = node(index)
        if tag == "Def":
            pending.append(ir["defs"][term]["body"])
        elif tag == "Junction" and term["kind"] == "And":
            pending.extend(term["items"])
        elif tag == "Builtin" and term["op"] == "In":
            left, right = map(node, term["args"])
            if left[0] == "Var" and not left[1]["primed"] and right[0] == "RecordSet":
                return True
    return False


class FiniteValueModel(SourceModel):
    """Use one finite value alphabet per source variable, including whole records."""

    def __init__(self, document, layout_expr):
        require(supported_configuration(document), "expected finite INIT/NEXT safety configuration")
        self.doc = document
        self.ir = document["ir"]
        self.b = cudd.BDD(memory_estimate=1 << 30)
        self.b.configure(reordering=False)
        self.names = [self.ir["decls"][index]["name"] for index in self.ir["vars"]]
        require(
            len(set(self.names)) == len(self.names), "ambiguous state variable names"
        )
        require(
            not {
                "deadlock",
                "partition changed",
                "invalid or undefined successor value",
            }.intersection(name for name, _ in document["invariants"]),
            "ambiguous invariant name",
        )
        self.layout_expr = layout_expr
        self.bound_reads = None
        self.values = {}
        self.bits = {}
        self.groups = {}
        self.variables = []
        # Layout inference evaluates ground domains before any variables exist.
        self.valid = self.b.true
        self.domains = self.infer_alphabets()
        require(
            sum(max(1, (len(d) - 1).bit_length()) for d in self.domains.values())
            <= 512,
            "whole-value layout exceeds 512 bits",
        )
        for index, domain in sorted(self.domains.items()):
            bits = []
            for position in range(max(1, (len(domain) - 1).bit_length())):
                name = f"v{index}b{position}"
                self.b.declare(name, name + "_next")
                self.variables.append(name)
                self.groups[name] = index
                bits.append(self.b.var(name))
            self.bits[index] = bits
            self.values[index] = {
                value: self.encoding(bits, ordinal)
                for ordinal, value in enumerate(domain)
            }
        self.valid = self.conjunction(
            self.any(mapping.values()) for mapping in self.values.values()
        )

    def node(self, expr):
        node = unpack(self.ir["exprs"][expr]["expr"])
        if node[0] == "Def":
            definition = self.ir["defs"][node[1]]
            require(
                definition["scope"] == "TopLevel"
                and definition["source"] is None
                and not definition["recursive"]
                and not definition["std"]
                and not definition["params"],
                "unsupported finite-value definition " + definition["name"],
            )
        return node

    def variable(self, expr):
        tag, data = self.node(expr)
        require(tag == "Var" and not data["primed"], "expected an unprimed variable")
        return data["var"]

    def conjuncts(self, expr):
        yield expr
        tag, data = self.node(expr)
        if tag == "Def":
            yield from self.conjuncts(self.ir["defs"][data]["body"])
        if tag == "Junction" and data["kind"] == "And":
            for child in data["items"]:
                yield from self.conjuncts(child)

    def infer_alphabets(self):
        initial_terms = set(self.conjuncts(self.doc["init"]))
        layout = self.layout_expr
        require(layout in initial_terms, "finite layout is not required by Init")
        domains = {}
        dependent = {}
        for expr in self.conjuncts(layout):
            if self.node(expr)[0] in ("Junction", "Def"):
                kind, data = self.node(expr)
                require(
                    kind == "Def" or data["kind"] == "And",
                    "expected conjunctive finite layout",
                )
                continue
            tag, term = self.node(expr)
            if tag == "Quant":
                require(term["kind"] == "Exists", "unrecognized quantified domain")
                op, equality = self.node(term["body"])
                require(
                    op == "Builtin" and equality["op"] == "Eq",
                    "expected singleton equality",
                )
                left, right = equality["args"]
                domain = set()
                for guard, scope in self.bindings_for(term, {}):
                    require(guard == self.b.true, "nonconstant quantified domain")
                    domain.add(self.ground(right, scope))
                require(domain, "empty quantified layout")
                domains[self.variable(left)] = domain
                continue
            require(
                tag == "Builtin" and term["op"] in ("In", "Subseteq"),
                "unknown finite domain",
            )
            left, right = term["args"]
            index = self.variable(left)
            kind, children = self.node(right)
            if (
                kind == "SetEnum"
                and children
                and all(self.node(item)[0] == "Var" for item in children)
            ):
                require(term["op"] == "In", "unsupported dynamic subset domain")
                dependent[index] = [self.variable(item) for item in children]
            else:
                upper = self.ground(right, {})
                require(isinstance(upper, frozenset), "layout membership in a non-set")
                domains[index] = powerset(upper) if term["op"] == "Subseteq" else upper
        for index, sources in dependent.items():
            require(
                all(source in domains for source in sources),
                "unresolved dependent domain",
            )
            domains[index] = set().union(*(domains[source] for source in sources))
        # A mandatory literal equality bounds Init even if the layout omits this variable.
        # Literal assignments propose extra codes; reachable RHS closure checks every branch.
        missing = set(range(len(self.names))) - domains.keys()
        for index in missing:
            literals = set()
            anchored = False
            for expr, entry in enumerate(self.ir["exprs"]):
                tag, term = unpack(entry["expr"])
                if tag != "Builtin" or term["op"] != "Eq":
                    continue
                left, right = term["args"]
                kind, var = unpack(self.ir["exprs"][left]["expr"])
                if kind != "Var" or var["var"] != index:
                    continue
                kind, value = unpack(self.ir["exprs"][right]["expr"])
                if kind in ("Num", "Str") and (var["primed"] or expr in initial_terms):
                    literals.add(int(value) if kind == "Num" else value)
                    anchored |= not var["primed"] and expr in initial_terms
            require(anchored, "Init does not bound " + self.names[index])
            domains[index] = literals
        return {index: sorted(domain, key=order) for index, domain in domains.items()}

    def ground(self, expr, env):
        values = self.value(expr, env)
        require(
            len(values) == 1 and next(iter(values.values())) == self.b.true,
            "expected ground value",
        )
        return next(iter(values))

    def combine(self, operation, *operands):
        """Lift a concrete finite-value operation over guarded alternatives."""
        result = {}
        for choice in itertools.product(*(operand.items() for operand in operands)):
            guard = self.conjunction(active for _, active in choice)
            if guard != self.b.false:
                value = operation(*(value for value, _ in choice))
                result[value] = result.get(value, self.b.false) | guard
        return result

    def bindings_for(self, term, env):
        require(len(term["bounds"]) == 1, "expected a single existential domain")
        (bound,) = term["bounds"]
        require(len(bound["vars"]) == 1, "expected a single local variable")
        allowed = {}
        for domain, guard in self.total_value(bound["domain"], env).items():
            require(isinstance(domain, frozenset), "quantifier over a non-set")
            for value in domain:
                allowed[value] = allowed.get(value, self.b.false) | guard
        for value in sorted(allowed, key=order):
            yield allowed[value], env | {bound["vars"][0]: value}

    def value(self, expr, env):
        tag, term = self.node(expr)

        def sub(child):
            return self.value(child, env)

        if tag in ("Num", "Str"):
            return {int(term) if tag == "Num" else term: self.b.true}
        if tag == "Local":
            require(term["up"] == 0, "captured local")
            return {env[term["slot"]]: self.b.true}
        if tag == "Var":
            index = self.variable(expr)
            require(
                self.bound_reads is None or index in self.bound_reads,
                "Init reads unbound variable " + self.names[index],
            )
            require(index in self.values, "nonconstant layout expression")
            return self.values[index]
        if tag == "Def":
            return sub(self.ir["defs"][term]["body"])
        if tag == "SetEnum":
            return self.combine(lambda *items: frozenset(items), *map(sub, term))
        if tag == "Builtin":
            operators = {
                "PowerSet": powerset,
                "SetMinus": lambda left, right: as_set(left) - as_set(right),
                "Cap": lambda left, right: as_set(left) & as_set(right),
                "BigUnion": union_members,
            }
            require(
                term["op"] in operators,
                "unsupported finite-value operator " + term["op"],
            )
            return self.combine(operators[term["op"]], *map(sub, term["args"]))
        if tag in ("Record", "RecordSet"):
            fields = [field["name"] for field in term]

            def record(*values):
                return tuple(sorted(zip(fields, values, strict=True)))

            def space(*domains):
                require(
                    all(isinstance(d, frozenset) for d in domains),
                    "record field domain is not a set",
                )
                require(
                    prod(map(len, domains)) <= 65536,
                    "whole-value record domain exceeds 65536 members",
                )
                return frozenset(record(*row) for row in itertools.product(*domains))

            return self.combine(
                record if tag == "Record" else space,
                *(sub(field["value"]) for field in term),
            )
        if tag == "RecordSelect":

            def select(value):
                require(isinstance(value, tuple), "field selection from a non-record")
                require(term["field"] in dict(value), "missing record field")
                return dict(value)[term["field"]]

            return self.combine(select, sub(term["record"]))
        if tag == "Except":
            updates = term["updates"]
            require(
                all(len(update["path"]) == 1 for update in updates), "nested EXCEPT"
            )
            fields = [self.ground(update["path"][0], env) for update in updates]
            require(len(set(fields)) == len(fields), "overlapping EXCEPT paths")

            def replace(record, *values):
                require(isinstance(record, tuple), "EXCEPT on a non-record")
                original = dict(record)
                require(set(fields) <= original.keys(), "missing EXCEPT field")
                return tuple(
                    sorted((original | dict(zip(fields, values, strict=True))).items())
                )

            return self.combine(
                replace,
                sub(term["base"]),
                *(sub(update["value"]) for update in updates),
            )
        if tag == "Case":
            require(term["other"] is None, "unrecognized CASE default")
            result = {}
            covered = self.b.false
            for arm in term["arms"]:
                guard = self.predicate(arm["guard"], env)
                require(guard & covered == self.b.false, "overlapping CASE guards")
                covered |= guard
                for value, active in sub(arm["value"]).items():
                    result[value] = result.get(value, self.b.false) | (guard & active)
            # Missing CASE coverage is a missing RHS value. The action compiler
            # checks that each reachable enabled source has a valid destination.
            return result
        raise Unsupported("unsupported finite value " + tag)

    def equal_values(self, left, right):
        return self.any(
            guard & right[value] for value, guard in left.items() if value in right
        )

    def total_value(self, expr, env):
        """Reject undefined predicate operands and quantifier domains."""
        choices = self.value(expr, env)
        require(
            self.valid & ~self.any(choices.values()) == self.b.false,
            "partial finite value in predicate or quantifier domain",
        )
        return choices

    def predicate(self, expr, env):
        tag, term = self.node(expr)

        def pred(child):
            return self.predicate(child, env)

        if tag == "Bool":
            return self.b.true if term else self.b.false
        if tag == "Def":
            return pred(self.ir["defs"][term]["body"])
        if tag == "Not":
            return ~pred(term)
        if tag == "Implies":
            return ~pred(term[0]) | pred(term[1])
        if tag == "Junction":
            return (self.conjunction if term["kind"] == "And" else self.any)(
                map(pred, term["items"])
            )
        if tag == "Quant":
            require(term["kind"] == "Exists", "unsupported predicate quantifier")
            return self.any(
                guard & self.predicate(term["body"], scope)
                for guard, scope in self.bindings_for(term, env)
            )
        require(tag == "Builtin", "unsupported predicate " + tag)
        op = term["op"]
        lhs, rhs = term["args"]
        left = self.total_value(lhs, env)
        if op in ("Eq", "Neq"):
            equal = self.equal_values(left, self.total_value(rhs, env))
            return equal if op == "Eq" else ~equal
        if op in ("In", "NotIn") and self.node(rhs)[0] == "SetEnum":
            # Avoid enumerating the product of several variable-valued members.
            member = self.any(
                self.equal_values(left, self.total_value(item, env))
                for item in self.node(rhs)[1]
            )
            return member if op == "In" else ~member
        require(op in ("In", "NotIn", "Subseteq"), "unrecognized predicate " + op)
        right = self.total_value(rhs, env)
        require(
            all(isinstance(y, frozenset) for y in right),
            "membership or subset with a non-set",
        )
        if op == "Subseteq":
            require(
                all(isinstance(x, frozenset) for x in left), "subset with a non-set"
            )
        result = self.any(
            a & b
            for x, a in left.items()
            for y, b in right.items()
            if (x <= y if op == "Subseteq" else x in y)
        )
        return ~result if op == "NotIn" else result

    def branches(self, expr, env, guard, assigned, *, action_conjunct=False):
        if guard == self.b.false:
            return
        tag, term = self.node(expr)
        require(
            tag != "Implies",
            "implication in Next requires a witness multiplicity proof",
        )
        if tag == "Def":
            yield from self.branches(
                self.ir["defs"][term]["body"], env, guard, assigned
            )
        elif tag == "Junction":
            if term["kind"] == "Or":
                # TLC expands a literal state disjunction inside an action conjunction
                # while the next-state assignment is incomplete.
                enumerate_guards = (
                    action_conjunct
                    and len(assigned) < len(self.names)
                    and self.ir["exprs"][expr]["level"] == "State"
                )
                prior = self.b.false
                for child in term["items"]:
                    for active, updated in self.branches(child, env, guard, assigned):
                        if updated.keys() == assigned.keys():
                            require(
                                enumerate_guards or active & prior == self.b.false,
                                "overlapping unassigned action alternatives",
                            )
                        prior |= active
                        yield active, updated
            else:
                rows = [(guard, assigned)]
                for child in term["items"]:
                    rows = [
                        row
                        for active, values in rows
                        for row in self.branches(
                            child,
                            env,
                            active,
                            values,
                            action_conjunct=self.ir["exprs"][expr]["level"] == "Action",
                        )
                    ]
                yield from rows
        elif tag == "Quant":
            require(term["kind"] == "Exists", "unsupported action quantifier")
            seen_filters = self.b.false
            for active, scope in self.bindings_for(term, env):
                for enabled, updated in self.branches(
                    term["body"], scope, guard & active, assigned
                ):
                    if updated.keys() == assigned.keys():
                        require(
                            enabled & seen_filters == self.b.false,
                            "overlapping existential guard witnesses",
                        )
                        seen_filters |= enabled
                    yield enabled, updated
        elif tag == "Unchanged":
            kind, items = self.node(term)
            require(kind == "Tuple", "expected UNCHANGED tuple")
            indices = [self.variable(item) for item in items]
            require(not set(indices) & assigned.keys(), "repeated frame assignment")
            yield guard, assigned | dict.fromkeys(indices)
        elif (
            tag == "Builtin"
            and term["op"] == "Eq"
            and self.node(term["args"][0])[0] == "Var"
            and self.node(term["args"][0])[1]["primed"]
        ):
            left, right = term["args"]
            index = self.node(left)[1]["var"]
            require(index not in assigned, "repeated primed assignment")
            choices = self.value(right, env)
            self.partial_rhs |= guard & ~self.any(choices.values())
            yield guard, assigned | {index: choices}
        else:
            yield guard & self.predicate(expr, env), assigned

    def decode(self, point):
        def concrete(value):
            if isinstance(value, frozenset):
                return {"set": [concrete(item) for item in sorted(value, key=order)]}
            if isinstance(value, tuple):
                return {"record": {field: concrete(item) for field, item in value}}
            return value

        result = {}
        for index, name in enumerate(self.names):
            code = sum(
                1 << bit for bit, var in enumerate(self.bits[index]) if point[var.var]
            )
            result[name] = concrete(self.domains[index][code])
        return result

    def check_init_order(self, expr, assigned, env):
        """Prove each read follows its binding and every alternative assigns a full row."""
        tag, term = self.node(expr)
        require(
            tag != "Implies",
            "implication in Init requires a witness multiplicity proof",
        )
        if tag == "Def":
            return self.check_init_order(self.ir["defs"][term]["body"], assigned, env)
        if tag == "Junction":
            if term["kind"] == "And":
                for child in term["items"]:
                    assigned = self.check_init_order(child, assigned, env)
                return assigned
            alternatives = [
                self.check_init_order(child, assigned, env) for child in term["items"]
            ]
            require(alternatives, "empty initial disjunction")
            return frozenset.intersection(*alternatives)
        self.bound_reads = assigned
        if tag == "Quant":
            require(term["kind"] == "Exists", "unsupported Init quantifier")
            choices = list(self.bindings_for(term, env))
            require(choices, "empty initial quantifier")
            return frozenset.intersection(
                *(
                    self.check_init_order(term["body"], assigned, scope)
                    for _, scope in choices
                )
            )
        if tag == "Builtin" and term["op"] in ("In", "Eq"):
            left, right = term["args"]
            if self.node(left)[0] == "Var":
                choices = self.total_value(right, env)
                index = self.variable(left)
                if index not in assigned:
                    # A later conjunct can narrow successful states after an earlier
                    # out-of-layout assignment has already triggered an error.
                    initial_values = (
                        choices
                        if term["op"] == "Eq"
                        else (value for domain in choices for value in as_set(domain))
                    )
                    require(
                        all(value in self.values[index] for value in initial_values),
                        "initial binding outside finite alphabet: " + self.names[index],
                    )
                return assigned | {index}
        self.predicate(expr, env)
        return assigned

    def check_initial_choices(self, expr, env, states):
        """Disjuncts and quantified initial witnesses must be disjoint on admitted states."""
        tag, term = self.node(expr)
        if tag == "Def":
            self.check_initial_choices(self.ir["defs"][term]["body"], env, states)
        elif tag == "Junction" and term["kind"] == "And":
            for child in term["items"]:
                self.check_initial_choices(child, env, states)
        elif tag in ("Junction", "Quant"):
            if tag == "Quant":
                require(term["kind"] == "Exists", "unsupported Init quantifier")
                choices = (
                    (term["body"], scope, guard)
                    for guard, scope in self.bindings_for(term, env)
                )
            else:
                choices = ((child, env, self.b.true) for child in term["items"])
            seen = self.b.false
            for child, scope, guard in choices:
                active = states & guard & self.predicate(child, scope)
                require(seen & active == self.b.false, "overlapping initial witnesses")
                seen |= active
                self.check_initial_choices(child, scope, active)

    def check(self, previous):
        require(previous is None, "whole-value search requires fresh inputs")
        result = self.check_values(keep_trace=True)
        if result["closed"]:
            return dict(
                verdict="success",
                distinct=result["distinct"],
                generated=result["generated"],
                depth=result["depth"],
                reused=False,
                initial=result["initial"],
                reachable_bdd_nodes=result["bdd_nodes"],
                invariants=result["invariants"],
                encoding="whole-values",
            ), None
        ending = result["ending"]
        verdict = "deadlock" if ending["kind"] == "deadlock" else "invariant-violation"
        return dict(
            verdict=verdict,
            depth=result["depth"],
            ending=ending,
            reused=False,
            encoding="whole-values",
        ), result["trace"]

    def check_values(self, fixed_values=(), keep_trace=False):
        assigned = self.check_init_order(self.doc["init"], frozenset(), {})
        require(assigned == self.values.keys(), "incomplete initial state binding")
        self.bound_reads = None
        initial = self.valid & self.predicate(self.doc["init"], {})
        fixed = self.conjunction(
            self.values[index][value] for index, value in fixed_values
        )
        self.check_initial_choices(self.doc["init"], {}, initial & fixed)
        reached = frontier = initial & fixed
        require(frontier != self.b.false, "empty initial set")
        initial_count = self.state_count(frontier)
        invalid_rhs = self.b.false
        self.partial_rhs = self.b.false
        actions = []
        for guard, values in self.branches(self.doc["next"], {}, self.b.true, {}):
            if guard == self.b.false:
                continue
            require(values.keys() == self.values.keys(), "incomplete action")
            updates = {}
            for index, assignment in values.items():
                if assignment is None:
                    continue
                domain = self.domains[index]
                valid = self.any(
                    active
                    for value, active in assignment.items()
                    if value in self.values[index]
                )
                invalid_rhs |= guard & ~valid
                for position, bit in enumerate(self.bits[index]):
                    value = self.any(
                        assignment.get(item, self.b.false)
                        for ordinal, item in enumerate(domain)
                        if (ordinal >> position) & 1
                    )
                    if value != bit:
                        updates[bit.var] = value
            actions.append((guard, self.partitions(updates, False)))
        invalid_rhs |= self.partial_rhs
        checks = [("invalid or undefined successor value", ~invalid_rhs)]
        checks.extend(
            (name, self.predicate(expr, {})) for name, expr in self.doc["invariants"]
        )
        if self.doc["check_deadlock"]:
            checks.append(("deadlock", self.any(guard for guard, _ in actions)))
        checks.append(("partition changed", fixed))
        admissible = self.conjunction(test for _, test in checks)
        layers = []
        depth = 0
        while frontier != self.b.false:
            depth += 1
            if keep_trace:
                layers.append(frontier)
            if frontier & ~admissible != self.b.false:
                failed = [
                    name for name, test in checks if frontier & ~test != self.b.false
                ]
                if keep_trace and frontier & (~fixed | invalid_rhs) == self.b.false:
                    self.actions = actions
                    self.constraint = self.b.true
                    name = failed[0]
                    ending = (
                        {"kind": "deadlock"}
                        if name == "deadlock"
                        else {"kind": "invariant", "name": name}
                    )
                    trace = self.witness(layers, frontier & ~dict(checks)[name], ending)
                    return {
                        "closed": False,
                        "depth": depth,
                        "invariants": [name for name, _ in self.doc["invariants"]],
                        "checked_deadlock": self.doc["check_deadlock"],
                        "ending": ending,
                        "trace": trace,
                    }
                raise Unsupported(
                    f"obligation failed at BFS depth {depth}: {', '.join(failed)}"
                )
            sources = reached if reached.dag_size < frontier.dag_size else frontier
            successors = self.any(
                self.image(sources, guard, relation) for guard, relation in actions
            )
            frontier = successors & ~reached
            reached |= frontier
        return {
            "initial": initial_count,
            "distinct": self.state_count(reached),
            "generated": initial_count
            + sum(self.state_count(reached & guard) for guard, _ in actions),
            "depth": depth,
            "bdd_nodes": reached.dag_size,
            "encoding_bits": len(self.variables),
            "action_witnesses": len(actions),
            "invariants": [name for name, _ in self.doc["invariants"]],
            "checked_deadlock": self.doc["check_deadlock"],
            "closed": True,
        }
