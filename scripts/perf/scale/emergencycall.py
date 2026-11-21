"""Count the finite reachable graph of the pinned EmergencyCall safety model.

Run with the symbolic worker's Python and PYTHONPATH. The positional inputs are
the original TLA+ module and a safety CFG. The exporter resolves both inputs.
"""

import argparse
import hashlib
import itertools
import json
from pathlib import Path
import subprocess
import sys

from dd import cudd
from induction import write_atomic
from engine import SourceModel, Unsupported, require

SOURCE_SHA = "76236976cf936bd81a8a30e57144ee9ed20b1a3498f9224a55e96147ab339a5e"
STANDARD_SOURCES = {
    "Naturals": "73f2a68412039aa86f37ad08f0d0fef50fa8327d0ad8806f515548c75e9d9d84",
    "Sequences": "f613cffe14133769f0972b6f6e0139aaf0773004a51bac88ada382426e3eb7f4",
    "FiniteSets": "d94cb545256539bf782fc1914842dfe8c19c1d3d4e48b8327dd0c47b74922e1e",
    "TLC": "944ece590eec81ace059881c2a62064c06041efa81539a788cd40fa7d1da41ce",
}
# Extra cases change a single finite set literal. Identifying them by the
# complete source digest keeps the initial-witness argument tied to those edits.
VARIANTS = {
    "published": SOURCE_SHA,
    "ordinary-1": "4f9be5695f4c713ca445ff765fe341d559cd70f350374252cd3e9d0b521d180a",
    "ordinary-2": "1c730ad1c2a34bc0de48767a482fdadf657a69a90a73f13fd068deb05ed47d36",
    "local-1": "d6c2291f6747d4614e829b9282cdc18a406741399a4cf8fa420a9624647dba8e",
    "local-2": "6a14b5eea015ac557b8267dab6eb02d10af410ce141aa64c3f9a85ae62232f77",
}
PARTITION = (
    "amf_0_normal_attach_accept_include_enl",
    "amf_1_normal_attach_accept_include_enl",
    "amf_0_emergency_attach_accept_include_enl",
    "amf_1_emergency_attach_accept_include_enl",
    "amf_home_reject_or_allow_unmarked_emergency_request",
    "amf_roam_reject_or_allow_unmarked_emergency_request",
)


def powerset(domain):
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


class PhoneModel(SourceModel):
    """Use one finite value alphabet per source variable, including whole records."""

    def __init__(self, document):
        require(
            document["format"] == "dive-symbolic-input-v2", "unknown exporter format"
        )
        require(
            not any(
                document[key]
                for key in (
                    "specification",
                    "constraints",
                    "overrides",
                    "assumptions",
                    "symmetry",
                    "constants",
                )
            ),
            "expected finite INIT/NEXT safety configuration",
        )
        self.doc = document
        self.ir = document["ir"]
        self.b = cudd.BDD(memory_estimate=1 << 30)
        self.b.configure(reordering=False)
        self.names = [self.ir["decls"][index]["name"] for index in self.ir["vars"]]
        self.definitions = {
            definition["name"]: definition["body"] for definition in self.ir["defs"]
        }
        require(
            self.doc["init"]
            in {
                self.definitions[name]
                for name in ("Init", "Init0", "Init1", "Init2", "Init3", "Init4")
            }
            and self.doc["next"] == self.definitions["Next"],
            "unknown Init or Next",
        )
        self.values = {}
        self.bits = {}
        self.groups = {}
        self.variables = []
        # Layout inference evaluates ground domains before any variables exist.
        self.valid = self.b.true
        self.domains = self.infer_alphabets()
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
        return next(iter(self.ir["exprs"][expr]["expr"].items()))

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
        layout = self.definitions["TypeInvariant"]
        require(layout in initial_terms, "finite layout is not required by Init")
        domains = {}
        dependent = {}
        for expr in self.node(layout)[1]["items"]:
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
                domains[index] = powerset(upper) if term["op"] == "Subseteq" else upper
        for index, sources in dependent.items():
            domains[index] = set().union(*(domains[source] for source in sources))
        dial = self.names.index("ue_dial_emergency")
        require(
            set(range(len(self.names))) - domains.keys() == {dial},
            "unexpected untyped coordinate",
        )

        def starts_at_zero(expr):
            tag, term = self.node(expr)
            if tag != "Builtin" or term["op"] != "Eq":
                return False
            left, right = map(self.node, term["args"])
            return left == ("Var", {"var": dial, "primed": False}) and right == (
                "Num",
                "0",
            )

        require(any(map(starts_at_zero, initial_terms)), "missing initial dial value")
        domains[dial] = {0, 1}
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
            return self.values[self.variable(expr)]
        if tag == "Def":
            return sub(self.ir["defs"][term]["body"])
        if tag == "SetEnum":
            return self.combine(lambda *items: frozenset(items), *map(sub, term))
        if tag == "Builtin":
            operators = {
                "PowerSet": powerset,
                "SetMinus": frozenset.difference,
                "Cap": frozenset.intersection,
                "BigUnion": lambda sets: frozenset().union(*sets),
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
                return frozenset(record(*row) for row in itertools.product(*domains))

            return self.combine(
                record if tag == "Record" else space,
                *(sub(field["value"]) for field in term),
            )
        if tag == "RecordSelect":
            return self.combine(
                lambda record: dict(record)[term["field"]], sub(term["record"])
            )
        if tag == "Except":
            updates = term["updates"]
            require(
                all(len(update["path"]) == 1 for update in updates), "nested EXCEPT"
            )
            fields = [self.ground(update["path"][0], env) for update in updates]
            require(len(set(fields)) == len(fields), "overlapping EXCEPT paths")

            def replace(record, *values):
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
        result = self.any(
            a & b
            for x, a in left.items()
            for y, b in right.items()
            if (x <= y if op == "Subseteq" else x in y)
        )
        return ~result if op == "NotIn" else result

    def branches(self, expr, env, guard, assigned):
        if guard == self.b.false:
            return
        tag, term = self.node(expr)
        if tag == "Def":
            yield from self.branches(
                self.ir["defs"][term]["body"], env, guard, assigned
            )
        elif tag == "Junction":
            if term["kind"] == "Or":
                for child in term["items"]:
                    yield from self.branches(child, env, guard, assigned)
            else:
                rows = [(guard, assigned)]
                for child in term["items"]:
                    rows = [
                        row
                        for active, values in rows
                        for row in self.branches(child, env, active, values)
                    ]
                yield from rows
        elif tag == "Quant":
            require(term["kind"] == "Exists", "unsupported action quantifier")
            for active, scope in self.bindings_for(term, env):
                yield from self.branches(term["body"], scope, guard & active, assigned)
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
            yield guard, assigned | {index: self.value(right, env)}
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

    def check(self, cell, keep_trace=False):
        initial = self.valid & self.predicate(self.doc["init"], {})
        fixed = self.b.true
        if cell is not None:
            fixed = self.conjunction(
                self.values[self.names.index(name)][(cell >> bit) & 1]
                for bit, name in enumerate(PARTITION)
            )
        reached = frontier = initial & fixed
        require(frontier != self.b.false, "empty initial set")
        initial_count = self.state_count(frontier)
        invalid_rhs = self.b.false
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
                if keep_trace and not {
                    "partition changed",
                    "invalid or undefined successor value",
                }.intersection(failed):
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
                        "cell": cell,
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
            "cell": cell,
            "invariants": [name for name, _ in self.doc["invariants"]],
            "checked_deadlock": self.doc["check_deadlock"],
            "closed": True,
        }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--variant", choices=VARIANTS, default="published")
    parser.add_argument("--cell", type=int, choices=range(64))
    parser.add_argument(
        "--trace",
        type=Path,
        help="save a validated shortest counterexample from the selected initial set",
    )
    parser.add_argument(
        "--validator", type=Path, help="dive executable for validate-trace"
    )
    args = parser.parse_args()
    if bool(args.validator) != bool(args.trace):
        parser.error("supply --trace and --validator together")
    if args.trace and any(
        args.trace.resolve() == source.resolve()
        for source in (args.module, args.config, args.exporter, args.validator)
    ):
        parser.error("the trace destination must be separate from the inputs")
    require(
        hashlib.sha256(args.module.read_bytes()).hexdigest() == VARIANTS[args.variant],
        "unrecognized EmergencyCall source",
    )
    bound = json.loads(
        subprocess.check_output([args.exporter.resolve(), args.module, args.config])
    )
    expected = STANDARD_SOURCES | {"EmergencyCall_Model_auto": VARIANTS[args.variant]}
    files = bound["ir"]["files"]
    require(
        len(files) == len(expected)
        and all(item["standard"] == (item["module"] in STANDARD_SOURCES) for item in files)
        and {
            item["module"]: hashlib.sha256(item["source"].encode()).hexdigest()
            for item in files
        } == expected,
        "unrecognized bound EmergencyCall module or import",
    )
    model = PhoneModel(bound)
    row = model.check(args.cell, keep_trace=args.trace is not None)
    trace = row.pop("trace", None)
    if trace is not None:
        data = json.dumps(trace)
        checked = subprocess.run(
            [
                args.validator.resolve(),
                "validate-trace",
                args.module,
                "--config",
                args.config,
                "--trace",
                "/dev/stdin",
                "--compat=tlc",
                "--fp",
                "0",
                "--seed",
                "0",
                "--output",
                "json",
            ],
            input=data,
            text=True,
            capture_output=True,
        )
        require(
            checked.returncode == 0 and json.loads(checked.stdout)["result"] == "valid",
            "independent source trace validation failed",
        )
        write_atomic(args.trace, (data + "\n").encode())
        row.update(trace_validated=True, trace_file=str(args.trace))
    print(json.dumps(dict(variant=args.variant, **row)))
    return int(trace is not None)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (Unsupported, KeyError, ValueError, subprocess.CalledProcessError) as error:
        sys.exit(f"unsupported: {error}")
