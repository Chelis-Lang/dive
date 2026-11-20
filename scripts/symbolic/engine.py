#!/usr/bin/env python3
"""Compile Dive's bound safety IR to CUDD using finite types declared in the source."""

import argparse
from collections import Counter
from dataclasses import dataclass
import errno
import itertools
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

try:
    from dd import cudd
except ImportError:
    print(json.dumps(dict(verdict="unsupported", reason="install dd==0.6.0 with its CUDD extension")))
    sys.exit(2)
from finite_relation import FiniteRelation
import induction


class Unsupported(ValueError):
    """The finite fragment or this layout cannot represent the program."""


class Stopped(RuntimeError):
    """A subprocess stopped before it produced a semantic result."""


def run_helper(command, label, memory_limited, **options):
    try:
        result = subprocess.run(command, text=True, capture_output=True, **options)
    except OSError as error:
        if error.errno in (errno.ENOMEM, errno.EAGAIN):
            raise Stopped(label + " exhausted process resources") from error
        raise
    if result.returncode:
        # A failed loader, allocator or thread startup can terminate before
        # Rust main returns a diagnostic. Never classify that as a model refusal.
        if result.returncode < 0:
            raise Stopped(f"{label} terminated by signal {-result.returncode}")
        if "cannot start a thread: " in result.stderr and any(
            f"(os error {code})" in result.stderr for code in (errno.ENOMEM, errno.EAGAIN)
        ):
            raise Stopped(label + " could not allocate a binder thread")
        if not result.stderr.startswith("Error: ") and not result.stdout.strip():
            reason = result.stderr.strip()[-500:] or f"exit {result.returncode}"
            if memory_limited:
                raise Stopped(f"{label} stopped under the memory budget: {reason}")
            raise RuntimeError(f"{label} failed without a result: {reason}")
    return result


def require(condition, message):
    if not condition:
        raise Unsupported(message)


# Atom alternatives and set membership have Boolean characteristic functions.
# Different wrappers keep function domain, set equality and scalar equality distinct.
class Atom(dict):
    pass


class Members(dict):
    pass


class Function(dict):
    pass


@dataclass
class FunctionSpace:
    domain: Members
    codomain: object


@dataclass
class PowerSet:
    domain: Members


def unpack(expr):
    if isinstance(expr, str):
        return expr, None
    return next(iter(expr.items()))


class SourceModel(FiniteRelation):
    def __init__(self, doc, type_invariant, manager=None, order_by_path=False, single_partition=False):
        require(doc["format"] == "dive-symbolic-input-v1", "incompatible bound IR exporter")
        self.doc = doc
        self.ir = doc["ir"]
        self.b = cudd.BDD(memory_estimate=512 << 20) if manager is None else manager
        self.b.configure(reordering=False)
        self.variables = []
        self.groups = {}
        self.layout = {}
        self.constants = [self.literal(v) for v in doc["constants"]]
        self.overrides = {int(i): self.literal(v) for i, v in doc["overrides"].items()}
        self.names = [self.ir["decls"][d]["name"] for d in self.ir["vars"]]
        require(len(set(self.names)) == len(self.names), "ambiguous state variable names")
        self.state = {}
        self.reading_init = True
        if doc["specification"] is not None:
            self.check_specification(doc["specification"])
        decls = [expr for name, expr in doc["invariants"] if name == type_invariant]
        require(len(decls) == 1, "expected a configured finite layout invariant named " + type_invariant)
        types = self.infer_types(decls[0])
        atoms = ()
        self.symmetry = doc["symmetry"] is not None
        if self.symmetry:
            declaration = doc["symmetry"]
            atoms = tuple(declaration["atoms"])
            indices = tuple(range(len(atoms)))
            require(
                1 < len(atoms) <= 6
                and {indices, *map(tuple, declaration["maps"])} == set(itertools.permutations(indices)),
                "expected a full finite permutation group",
            )
        self.types = types
        self.allocate(types, atoms, order_by_path)

        # Init is admitted only for a deterministic, total, ground assignment.
        initial = iter(self.branches(doc["init"], {}, True, self.b.true, {}))
        first = next(initial, None)
        require(first is not None and next(initial, None) is None, "expected one Init witness")
        guard, assigns = first
        require(guard == self.b.true, "nonconstant or disabled Init")
        init_bits = self.encode(assigns)
        require(all(v in (self.b.true, self.b.false) for v in init_bits.values()), "nonground Init")
        self.initial = self.b.cube({name: val == self.b.true for name, val in init_bits.items()})
        self.reading_init = False
        self.properties = [(name, self.boolean(self.eval(expr, {}, {}))) for name, expr in doc["invariants"]]
        updates = []
        for index, (guard, assigns) in enumerate(self.branches(doc["next"], {}, False, self.b.true, {})):
            require(index < 100000, "transition exceeds 100000 expanded action witnesses")
            change = {n: v for n, v in self.encode(assigns).items() if v != self.b.var(n)}
            if guard != self.b.false:
                updates.append((guard, change))
        if self.symmetry:
            self.check_equivariance(updates)
        # Disjoint witnesses with identical effect can share an image while
        # retaining exact generated multiplicity on every concrete source.
        merged = {}
        for guard, change in updates:
            group = merged.setdefault(tuple(sorted(change.items())), [])
            for i, other in enumerate(group):
                if (guard & other) == self.b.false:
                    group[i] |= guard
                    break
            else:
                group.append(guard)
        self.effects = Counter((g, frozenset(change)) for change, guards in merged.items() for g in guards)
        self.actions = [
            (guard, self.partitions(dict(change), single_partition))
            for change, guards in merged.items()
            for guard in guards
        ]

    def check_specification(self, name):
        # The implicit stutter in [Next]_v preserves every state invariant
        # only when v contains all state variables. Bind INIT/NEXT directly
        # or admit the common two-conjunct SPECIFICATION with this property.
        matching = [d for d in self.ir["defs"] if d["name"] == name and d["scope"] == "TopLevel"]
        require(len(matching) == 1, "ambiguous specification definition")
        tag, value = self.node(matching[0]["body"])
        require(
            tag == "Junction" and value["kind"] == "And" and len(value["items"]) == 2,
            "expected Init and a stuttering Next action",
        )
        tag, always = self.node(value["items"][1])
        require(tag == "Always", "expected always-action specification")
        tag, box = self.node(always)
        require(tag == "StepBox" and box["action"] == self.doc["next"], "unsupported stuttering action")

        def variables(i):
            tag, x = self.node(i)
            if tag == "Tuple":
                return {v for child in x for v in variables(child)}
            if tag == "Def" and x not in self.overrides:
                body, _ = self.call(tag, x, {}, {})
                return variables(body)
            require(tag == "Var" and not x["primed"], "unsupported stuttering subscript")
            return {x["var"]}

        require(variables(box["sub"]) == set(range(len(self.names))), "stuttering subscript omits variables")

    def infer_types(self, role):
        """Read finite bounds from a conjunction of state-variable type declarations."""
        found = {}

        def visit(expr):
            tag, x = self.node(expr)
            if tag == "Def":
                body, env = self.call(tag, x, {}, {})
                require(not env, "parameterized layout")
                visit(body)
            elif tag == "Junction" and x["kind"] == "And":
                for item in x["items"]:
                    visit(item)
            else:
                require(
                    tag == "Builtin" and x["op"] in ("In", "Subseteq"), "expected a finite type declaration"
                )
                lhs, rhs = x["args"]
                tag, var = self.node(lhs)
                require(tag == "Var" and not var["primed"], "expected a state variable in layout")
                name = self.names[var["var"]]
                require(name not in found, "duplicate type declaration " + name)
                bound = self.eval(rhs, {}, {})
                found[name] = PowerSet(bound) if x["op"] == "Subseteq" else bound

        visit(role)
        require(found.keys() == set(self.names), "need one finite bound for every state variable")
        return found

    def allocate(self, types, group_atoms, order_by_path):
        """Place bits of paths sharing the most common inner index near one another."""
        leaves = []
        domains = Counter()
        permuted = set(group_atoms)

        def indices(domain):
            keys = set(self.static_set(domain))
            require(not keys & permuted or permuted <= keys, "asymmetric finite index domain")
            return sorted(keys)

        # Check the size while traversing the proposed type. Nested functions
        # can expand overwhelmingly before the first BDD variable is allocated.
        planned_bits = 0
        fields_visited = 0

        def describe(bound, path, inner_domain):
            nonlocal planned_bits, fields_visited
            fields_visited += 1
            require(fields_visited <= 16384, "finite layout exceeds 16384 fields")
            if isinstance(bound, FunctionSpace):
                keys = indices(bound.domain)
                for k in keys:
                    describe(bound.codomain, (*path, k), tuple(keys))
                return
            if isinstance(bound, PowerSet):
                keys = indices(bound.domain)
                planned_bits += len(keys)
                require(planned_bits <= 512, "finite layout exceeds 512 state bits")
                for k in keys:
                    leaves.append(((*path, k), None))
                domains[tuple(keys)] += len(keys)
                return
            choices = tuple(sorted(self.static_set(bound)))
            require(choices, "empty scalar domain")
            require(not set(choices) & permuted, "permuted atoms in a scalar field")
            planned_bits += max(1, (len(choices) - 1).bit_length())
            require(planned_bits <= 512, "finite layout exceeds 512 state bits")
            domains[inner_domain] += 1
            leaves.append((path, choices))

        for name in self.names:
            describe(types[name], (name,), ())
        clusters = max((d for d in domains if d), key=domains.get, default=())
        atoms = tuple(sorted(group_atoms))

        def order(leaf):
            path, _ = leaf
            group = next((clusters.index(k) for k in reversed(path[1:]) if k in clusters), -1)
            roles = [atoms.index(k) for k in path[1:] if k in atoms]
            require(len(roles) <= 1, "multiple permuted indices in one path")
            return group, (roles[0] if roles else -1), self.names.index(path[0]), path

        # Record relative path identities as well as bit positions. This lets a
        # permutation of outer OR inner function indices move the right fields.
        role_fields = {a: {} for a in atoms}
        for path, choices in sorted(
            leaves, key=(lambda leaf: (self.names.index(leaf[0][0]), leaf[0])) if order_by_path else order
        ):
            width = max(1, (len(choices) - 1).bit_length()) if choices else 1
            group, role, _, _ = order((path, choices))
            fields = []
            for bit in range(width):
                name = f"b{len(self.variables)}"
                if name not in self.b.vars:
                    self.b.declare(name, name + "_next")
                self.variables.append(name)
                self.groups[name] = group
                if role >= 0:
                    ident = atoms[role]
                    relative = (path[0], *(None if k == ident else k for k in path[1:]), bit)
                    role_fields[ident][relative] = name
                fields.append(self.b.var(name))
            if choices:
                choices += tuple(("padding", i) for i in range(len(choices), 1 << width))
            self.layout[path] = (fields, choices)
        self.roles = []
        if atoms:
            # Preserve common slot order independently of how BDD variables are ordered.
            slots = list(role_fields[atoms[0]])
            for fields in role_fields.values():
                require(fields.keys() == set(slots), "asymmetric finite layout")
                self.roles.append([fields[k] for k in slots])

        def build(bound, path):
            if isinstance(bound, FunctionSpace):
                return Function((k, build(bound.codomain, (*path, k))) for k in self.static_set(bound.domain))
            if isinstance(bound, PowerSet):
                return Members((k, self.layout[(*path, k)][0][0]) for k in self.static_set(bound.domain))
            fields, choices = self.layout[path]
            return Atom((k, self.encoding(fields, i)) for i, k in enumerate(choices))

        self.state = {name: build(bound, (name,)) for name, bound in types.items()}

    def check_equivariance(self, updates):
        before = Counter((g, frozenset(v.items())) for g, v in updates)
        for left, right in zip(self.roles, self.roles[1:]):
            swap = dict(zip(left + right, right + left))

            def permute(value):
                return self.b.let(swap, value)

            require(
                permute(self.initial) == self.initial and all(permute(p) == p for _, p in self.properties),
                "asymmetric Init or invariant",
            )
            after = Counter(
                (permute(g), frozenset((swap.get(k, k), permute(rhs)) for k, rhs in v.items()))
                for g, v in updates
            )
            require(before == after, "asymmetric action witnesses")

    def partitions(self, updates, single_partition):
        groups = {}
        for name, rhs in updates.items():
            groups.setdefault(0 if single_partition else self.groups[name], {})[name] = rhs
        result = []
        while groups:
            # Consume readers before overwriting any bit they still need.
            reads = {g: set().union(*(e.support for e in part.values())) for g, part in groups.items()}
            ready = next(
                (
                    g
                    for g, part in groups.items()
                    if all(not part.keys() & read for h, read in reads.items() if h != g)
                ),
                None,
            )
            if ready is None:
                part = {k: v for part in groups.values() for k, v in part.items()}
                groups.clear()
            else:
                part = groups.pop(ready)
            result.append(self.assignment({self.b.var(n): v for n, v in part.items()}))
        return result

    def atom(self, key):
        return Atom({key: self.b.true})

    def literal(self, value):
        if isinstance(value, str):
            return self.atom(("string", value))
        if set(value) == {"model"}:
            require("_" not in value["model"], "typed model values")
            return self.atom(value["model"])
        if set(value) == {"set"}:
            return Members((self.key(self.literal(v)), self.b.true) for v in value["set"])
        raise Unsupported("configured value outside model-value or finite set fragment")

    def key(self, atom):
        require(isinstance(atom, Atom), "expected a ground model value")
        keys = [k for k, v in atom.items() if v != self.b.false]
        require(len(keys) == 1 and atom[keys[0]] == self.b.true, "symbolic function index or binder element")
        return keys[0]

    def static_set(self, value):
        require(
            isinstance(value, Members) and all(v == self.b.true for v in value.values()), "nonconstant domain"
        )
        return sorted(value, key=repr)

    def boolean(self, value):
        require(isinstance(value, cudd.Function), "expected a Boolean expression")
        return value

    def any(self, terms):
        return ~self.conjunction(~self.boolean(t) for t in terms)

    def same(self, a, b):
        require(type(a) is type(b), "mixed-type equality")
        if isinstance(a, (Atom, Members)):
            return self.conjunction(
                a.get(k, self.b.false).equiv(b.get(k, self.b.false)) for k in a.keys() | b.keys()
            )
        if isinstance(a, Function):
            if a.keys() != b.keys():
                return self.b.false
            return self.conjunction(self.same(v, b[k]) for k, v in a.items())
        return self.boolean(a).equiv(self.boolean(b))

    def subset(self, a, b):
        require(isinstance(a, Members) and isinstance(b, Members), "subset of non-set")
        return self.conjunction(~v | b.get(k, self.b.false) for k, v in a.items())

    def contains(self, value, space):
        if isinstance(space, FunctionSpace):
            require(isinstance(value, Function), "function-space element kind")
            domain = Members((k, self.b.true) for k in value)
            return self.same(domain, space.domain) & self.conjunction(
                self.contains(v, space.codomain) for v in value.values()
            )
        if isinstance(space, PowerSet):
            return self.subset(value, space.domain)
        require(isinstance(space, Members) and isinstance(value, Atom), "membership kind")
        return self.any(bit & space.get(k, self.b.false) for k, bit in value.items())

    def select(self, cond, a, b):
        require(type(a) is type(b), "mixed-type IF")
        if isinstance(a, (Atom, Members, Function)):
            require(not isinstance(a, Function) or a.keys() == b.keys(), "varying function domain")
            return type(a)(
                (k, self.select(cond, a.get(k, self.b.false), b.get(k, self.b.false)))
                for k in a.keys() | b.keys()
            )
        return self.b.ite(cond, self.boolean(a), self.boolean(b))

    def node(self, i):
        return unpack(self.ir["exprs"][i]["expr"])

    def call(self, tag, data, env, assigned):
        if tag == "Apply":
            require("Def" in data["callee"], "unsupported operator argument or dynamic callee")
        idx = data if tag == "Def" else data["callee"]["Def"]
        require(idx not in self.overrides, "operator or layout body replaced by a value")
        d = self.ir["defs"][idx]
        require(
            d["scope"] == "TopLevel" and d["source"] is None and not d["recursive"] and not d["std"],
            "local, instantiated, recursive or standard operator: " + d["name"]
        )
        args = [] if tag == "Def" else data["args"]
        require(len(args) == len(d["params"]), "operator arity")
        return d["body"], {i: self.eval(a["Expr"]["expr"], env, assigned) for i, a in enumerate(args)}

    def bindings(self, q, env, assigned, *, multiple=False):
        require(len(q["bounds"]) == 1, "multiple bound groups")
        (bound,) = q["bounds"]
        slots = bound["vars"]
        require(slots and (len(slots) == 1 or multiple), "expected a single bound variable")
        require(not bound["tuple"] and bound["domain"] is not None, "unbounded or tuple binder")
        domain = self.eval(bound["domain"], env, assigned)
        require(isinstance(domain, Members), "binder over a non-atomic set")
        if len(slots) == 1:
            for key in sorted(domain, key=repr):
                yield key, domain[key], {**env, slots[0]: self.atom(key)}
            return
        require(len(domain) ** len(slots) <= 100000, "quantifier exceeds 100000 bindings")
        for keys in itertools.product(domain, repeat=len(slots)):
            scope = {**env, **{slot: self.atom(key) for slot, key in zip(slots, keys)}}
            yield keys, self.conjunction(domain[key] for key in keys), scope

    def eval(self, i, env, assigned):
        tag, x = self.node(i)

        def sub(expr):
            return self.eval(expr, env, assigned)

        if tag == "Str":
            return self.atom(("string", x))
        if tag == "Bool":
            return self.b.true if x else self.b.false
        if tag == "Var":
            name = self.names[x["var"]]
            if self.reading_init:
                require(not x["primed"] and name in assigned, "unbound state read in Init or layout: " + name)
                return assigned[name]
            return (assigned if x["primed"] else self.state)[name]
        if tag == "Local":
            require(x["up"] == 0, "captured local")
            return env[x["slot"]]
        if tag == "Const":
            return self.constants[x]
        if tag == "Def" and x in self.overrides:
            return self.overrides[x]
        if tag in ("Def", "Apply"):
            body, args = self.call(tag, x, env, assigned)
            return self.eval(body, args, assigned)
        if tag == "FcnSet":
            return FunctionSpace(sub(x["domain"]), sub(x["range"]))
        if tag in ("FcnCons", "Quant"):
            bindings = self.bindings(x, env, assigned, multiple=(tag == "Quant"))
            if tag == "FcnCons":
                out = Function()
                for k, guard, scope in bindings:
                    require(guard == self.b.true, "symbolic function domain")
                    out[k] = self.eval(x["body"], scope, assigned)
                return out
            require(x["kind"] in ("Forall", "Exists"), "quantifier " + x["kind"])
            terms = ((g, self.boolean(self.eval(x["body"], scope, assigned))) for _, g, scope in bindings)
            return (
                self.conjunction(~g | v for g, v in terms)
                if x["kind"] == "Forall"
                else self.any(g & v for g, v in terms)
            )
        if tag == "FcnApply":
            f = sub(x["fcn"])
            require(isinstance(f, Function), "function application")
            key = self.key(sub(x["arg"]))
            require(key in f, "index outside finite function domain")
            return f[key]
        if tag == "SetEnum":
            out = Members()
            for expr in x:
                atom = sub(expr)
                require(isinstance(atom, Atom), "set of nonscalars")
                for k, g in atom.items():
                    out[k] = out.get(k, self.b.false) | g
            return out
        if tag == "ExceptAt":
            return env["@"]
        if tag == "Except":
            base = sub(x["base"])
            require(len(x["updates"]) == 1, "multiple EXCEPT clauses")
            update = x["updates"][0]
            path = [self.key(sub(e)) for e in update["path"]]

            def replace(value, depth):
                if depth == len(path):
                    return self.eval(update["value"], {**env, "@": value}, assigned)
                require(isinstance(value, Function), "EXCEPT through a nonfunction")
                k = path[depth]
                require(k in value, "EXCEPT index outside finite function domain")
                return Function({**value, k: replace(value[k], depth + 1)})

            base = replace(base, 0)
            return base
        if tag == "Junction":
            values = (self.boolean(sub(e)) for e in x["items"])
            return self.conjunction(values) if x["kind"] == "And" else self.any(values)
        if tag == "Not":
            return ~self.boolean(sub(x))
        if tag == "Implies":
            return ~self.boolean(sub(x[0])) | self.boolean(sub(x[1]))
        if tag == "If":
            return self.select(self.boolean(sub(x["cond"])), sub(x["then"]), sub(x["other"]))
        if tag == "Builtin":
            op = x["op"]
            a = sub(x["args"][0])
            if op == "PowerSet":
                return PowerSet(a)
            b = sub(x["args"][1])
            if op in ("Eq", "Neq"):
                eq = self.same(a, b)
                return eq if op == "Eq" else ~eq
            if op in ("In", "NotIn"):
                inside = self.contains(a, b)
                return inside if op == "In" else ~inside
            if op == "Subseteq":
                return self.subset(a, b)
            if op in ("Cup", "Cap", "SetMinus"):
                require(isinstance(a, Members) and isinstance(b, Members), "set operation kind")

                def merge(k):
                    u, v = a.get(k, self.b.false), b.get(k, self.b.false)
                    return u | v if op == "Cup" else (u & v if op == "Cap" else u & ~v)

                return Members((k, merge(k)) for k in a.keys() | b.keys())
            raise Unsupported("IR builtin " + op)
        raise Unsupported("IR expression " + tag)

    def branches(self, i, env, init, guard, assigned):
        tag, x = self.node(i)
        if tag in ("Def", "Apply") and not (tag == "Def" and x in self.overrides):
            body, args = self.call(tag, x, env, assigned)
            yield from self.branches(body, args, init, guard, assigned)
        elif tag == "Junction":
            items = x["items"]
            if x["kind"] == "Or":
                for item in items:
                    yield from self.action_alternatives(item, env, init, guard, assigned)
            else:
                require(x["kind"] == "And", "unknown action junction")

                def conjunction(index, active, values):
                    if index == len(items):
                        yield active, values
                    else:
                        for g, a in self.branches(items[index], env, init, active, values):
                            yield from conjunction(index + 1, g, a)

                yield from conjunction(0, guard, assigned)
        elif tag == "Quant" and x["kind"] == "Exists":
            for _, g, scope in self.bindings(x, env, assigned):
                yield from self.action_alternatives(x["body"], scope, init, guard & g, assigned)
        elif tag == "Unchanged":
            require(not init, "UNCHANGED in Init")
            kind, items = self.node(x)
            result = assigned.copy()
            for item in items if kind == "Tuple" else [x]:
                kind, v = self.node(item)
                require(kind == "Var" and not v["primed"], "UNCHANGED of non-variable")
                name = self.names[v["var"]]
                require(name not in result, "duplicate assignment " + name)
                result[name] = self.state[name]
            yield guard, result
        elif (
            tag == "Builtin"
            and x["op"] == "Eq"
            and self.node(x["args"][0])[0] == "Var"
            and (self.node(x["args"][0])[1]["primed"] != init)
        ):
            name = self.names[self.node(x["args"][0])[1]["var"]]
            require(name not in assigned, "duplicate assignment " + name)
            yield guard, {**assigned, name: self.eval(x["args"][1], env, assigned)}
        else:
            require(
                tag in ("Bool", "Not")
                or (
                    tag == "Quant" and x["kind"] == "Forall"
                    and self.ir["exprs"][i]["level"] in ("Constant", "State")
                    and self.node(x["body"])[0] == "Builtin"
                    and self.node(x["body"])[1]["op"] in ("Eq", "Neq", "In", "NotIn", "Subseteq")
                )
                or (tag == "Builtin" and x["op"] in ("Eq", "Neq", "In", "NotIn", "Subseteq")),
                "unsupported action guard: " + tag,
            )
            yield guard & self.boolean(self.eval(i, env, assigned)), assigned

    def action_alternatives(self, expr, env, init, guard, assigned):
        # Pure Boolean choices can be evaluated once or enumerated as multiple
        # witnesses depending on binding order and operator evaluation level.
        # Admit only alternatives that actually bind a state variable.
        for active, values in self.branches(expr, env, init, guard, assigned):
            require(len(values) > len(assigned), "choice without assignment in action guard")
            yield active, values

    def encode(self, assigned):
        require(set(assigned) == set(self.names), "incomplete state assignment")
        out = {}

        def visit(schema, value, path):
            if isinstance(schema, Function):
                require(
                    isinstance(value, Function) and schema.keys() == value.keys(), "function domain mismatch"
                )
                for k in schema:
                    visit(schema[k], value[k], (*path, k))
            elif isinstance(schema, Members):
                require(
                    isinstance(value, Members)
                    and all(v == self.b.false for k, v in value.items() if k not in schema),
                    "set assignment outside finite layout",
                )
                for k in schema:
                    fields, _ = self.layout[(*path, k)]
                    out[fields[0].var] = value.get(k, self.b.false)
            else:
                fields, alphabet = self.layout[path]
                require(
                    isinstance(value, Atom) and value.keys() <= set(alphabet),
                    "scalar assignment outside finite layout",
                )
                for bit, var in enumerate(fields):
                    out[var.var] = self.any(
                        value.get(k, self.b.false) for j, k in enumerate(alphabet) if j & (1 << bit)
                    )

        for name, value in assigned.items():
            visit(self.state[name], value, (name,))
        return out

    def decode(self, point):
        def scalar(key):
            if isinstance(key, str):
                return {"model": key}
            require(key[0] == "string", "invalid encoded scalar in witness")
            return key[1]

        def read(schema, path):
            if isinstance(schema, Function):
                return {"fcn": [[scalar(k), read(v, (*path, k))] for k, v in schema.items()]}
            if isinstance(schema, Members):
                return {"set": [scalar(k) for k in schema if point[self.layout[(*path, k)][0][0].var]]}
            fields, alphabet = self.layout[path]
            key = alphabet[sum(1 << i for i, bit in enumerate(fields) if point[bit.var])]
            return scalar(key)

        return {name: read(self.state[name], (name,)) for name in self.names}

    def witness(self, layers, target, ending):
        path = []
        for depth in reversed(range(len(layers))):
            point = self.b.pick(target, care_vars=set(self.variables))
            path.append({"values": self.decode(point)})
            if depth:
                cube = self.b.cube(point)
                for guard, updates in self.actions:
                    pred = cube
                    for rel, _, rename in reversed(updates):
                        pred = cudd.and_exists(
                            self.b.let({v: k for k, v in rename.items()}, pred), rel, list(rename)
                        )
                    target = layers[depth - 1] & guard & pred
                    if target != self.b.false:
                        break
                require(target != self.b.false, "missing predecessor")
        return dict(format="dive-trace", version=1, variables=self.names, states=path[::-1], ending=ending)

    def same_relation(self, other):
        # The same manager makes equality of Boolean functions exact. Layout
        # includes concrete paths and enum encodings, even on unused fields.
        return (
            self.b is other.b
            and self.types == other.types
            and self.layout == other.layout
            and self.names == other.names
            and self.initial == other.initial
            and self.effects == other.effects
            and self.roles == other.roles
            and self.symmetry == other.symmetry
        )

    def violations(self, states):
        for name, prop in self.properties:
            if states & ~prop != self.b.false:
                yield dict(kind="invariant", name=name), ~prop
        if self.doc["check_deadlock"]:
            bad = ~self.any(g for g, _ in self.actions)
            if states & bad != self.b.false:
                yield dict(kind="deadlock"), bad

    def image(self, states, guard, updates):
        result = states & guard
        for relation, old, rename in updates:
            result = self.b.let(rename, cudd.and_exists(result, relation, old))
        return result

    def check(self, previous=None):
        self.complete = None
        if previous is not None and previous.complete is not None and self.same_relation(previous):
            reached, layers, distinct, generated = previous.complete
            if any(self.violations(reached)):
                for depth, layer in enumerate(layers, 1):
                    for ending, bad in self.violations(layer):
                        return self.failure(layers[:depth], layer & bad, ending, reused=True)
            self.complete = previous.complete
            return self.success(len(layers), reached, distinct, generated, True), None

        reached = frontier = self.initial
        layers = []
        while frontier != self.b.false:
            layers.append(frontier)
            for ending, bad in self.violations(frontier):
                return self.failure(layers, frontier & bad, ending, reused=False)
            successors = self.b.false
            for guard, updates in self.actions:
                successors |= self.image(frontier, guard, updates)
            frontier = successors & ~reached
            reached |= frontier
        distinct, generated = self.counts(reached, self.symmetry)
        self.complete = reached, layers, distinct, generated
        return self.success(len(layers), reached, distinct, generated, False), None

    def failure(self, layers, target, ending, reused):
        verdict = "deadlock" if ending["kind"] == "deadlock" else "invariant-violation"
        return dict(verdict=verdict, depth=len(layers), ending=ending, reused=reused), self.witness(
            layers, target, ending
        )

    def success(self, depth, reached, distinct, generated, reused):
        return dict(
            verdict="success",
            distinct=distinct,
            generated=generated,
            depth=depth,
            reused=reused,
            reachable_bdd_nodes=reached.dag_size,
            invariants=[n for n, _ in self.properties],
        )


def run(args):
    if args.max_memory:
        _, hard = resource.getrlimit(resource.RLIMIT_AS)
        limit = args.max_memory << 20
        if hard != resource.RLIM_INFINITY:
            limit = min(limit, hard)
        resource.setrlimit(resource.RLIMIT_AS, (limit, hard))
    previous = None
    violated = False
    for module, config in [(args.module, args.config), *args.recheck]:
        start = time.monotonic()
        exported = run_helper([args.exporter, module, config], "Dive frontend", args.max_memory)
        require(exported.returncode == 0, "Dive frontend: " + exported.stderr.strip())
        doc = json.loads(exported.stdout)
        model = SourceModel(
            doc,
            args.type_invariant,
            previous.b if previous is not None else None,
            args.order_by_path,
            args.single_partition,
        )
        if args.verify_predicate or args.prove_inductive:
            states = (
                induction.load(Path(args.verify_predicate), model)
                if args.verify_predicate
                else model.conjunction(prop for _, prop in model.properties)
            )
            failed = induction.verify(model, states)
            row = dict(
                format="dive-symbolic-report-v1",
                module=str(module),
                config=str(config),
                proof="inconclusive" if failed else "inductive-safety",
                failed_obligation=failed,
                seconds=round(time.monotonic() - start, 6),
            )
            print(json.dumps(row), flush=True)
            if failed:
                return 2
            continue
        row, trace = model.check(previous)
        if args.write_predicate and model.complete is not None:
            induction.store(Path(args.write_predicate), model, model.complete[0])
        if trace is not None:
            violated = True
            raw = json.dumps(trace)
            validated = run_helper(
                [
                    args.dive,
                    "validate-trace",
                    module,
                    "--trace",
                    "/dev/stdin",
                    "--config",
                    config,
                    "--compat=tlc",
                    "--seed",
                    "0",
                    "--fp",
                    "0",
                    "--output",
                    "json",
                ],
                "Dive trace validator",
                args.max_memory,
                input=raw,
            )
            require(
                validated.returncode == 0 and json.loads(validated.stdout)["result"] == "valid",
                "independent trace validation failed: " + validated.stdout[-1500:],
            )
            row["trace_validated"] = True
            if args.trace:
                induction.write_atomic(Path(args.trace), (raw + "\n").encode("utf-8"))
                row["trace_file"] = str(args.trace)
            else:
                row["trace"] = trace
        row.update(format="dive-symbolic-report-v1", module=str(module), config=str(config))
        row["seconds"] = round(time.monotonic() - start, 6)
        print(json.dumps(row), flush=True)
        previous = model
    return int(violated)


if __name__ == "__main__":
    try:
        sys.exit(run(argparse.Namespace(**json.loads(sys.argv[1]))))
    except Stopped as err:
        print(json.dumps(dict(verdict="stopped", reason=str(err))), flush=True)
        sys.exit(4)
    except MemoryError:
        print(json.dumps(dict(verdict="stopped", reason="memory exhausted")), flush=True)
        sys.exit(4)
    except (Unsupported, ValueError, KeyError, IndexError, TypeError, OSError, RecursionError) as err:
        print(json.dumps(dict(verdict="unsupported", reason=str(err))), flush=True)
        sys.exit(2)
    except Exception as err:
        # An internal or CUDD failure can never publish a safety verdict.
        print(json.dumps(dict(verdict="error", reason=f"{type(err).__name__}: {err}")), flush=True)
        sys.exit(3)
