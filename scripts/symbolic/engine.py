#!/usr/bin/env python3
"""Compile Dive's bound safety IR to CUDD using finite types declared in the source."""

import argparse
from collections import Counter
from contextlib import suppress
from dataclasses import dataclass
from enum import Enum, auto
import errno
import gc
import itertools
import json
import math
import operator
from pathlib import Path
import resource
import subprocess
import sys
import tempfile
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


class Grow(Exception):
    """The current finite coordinate system is missing reachable set members."""


class ChangedRelation(Exception):
    """Rebind the edited model in a fresh manager using its surviving coordinates."""


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


class Enumeration(Enum):
    # Certify the raw enumeration used by Init membership, independently of
    # the characteristic function. Reducible sets (literals and intervals)
    # also perform eager set operations with other reducible sets.
    UNPROVED = auto()
    UNIQUE = auto()
    REDUCIBLE = auto()


class Members(dict):
    def __init__(self, items=(), *, enumeration=Enumeration.UNPROVED):
        super().__init__(items)
        self.enumeration = enumeration


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
    def __init__(self, doc, type_invariant, manager=None, order_by_path=False, single_partition=False, learned=None):
        require(doc["format"] == "dive-symbolic-input-v2", "incompatible bound IR exporter")
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
        for index, (name, expr) in enumerate(doc["assumptions"], 1):
            # Only ground operators implemented by this interpreter can discharge
            # an assumption. A false or unevaluable premise never yields Holds.
            holds = self.boolean(self.eval(expr, {}, {})) == self.b.true
            require(holds, f"ASSUME {name or index} did not evaluate to TRUE")
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
        self.refining = learned is not None
        self.whole_domains = {}
        self.missing = {}
        self.initial_missing = set()
        if self.refining:
            for name, typ in list(types.items()):
                if isinstance(typ, PowerSet) and len(typ.domain) > 256:
                    require(not self.symmetry, "wide set-domain refinement with symmetry")
                    path = (name,)
                    self.whole_domains[path] = typ.domain
                    types[name] = PowerSet(Members(
                        (k, bit) for k, bit in typ.domain.items() if (path, k) in learned
                    ))
        self.types = types
        self.allocate(types, atoms, order_by_path)

        # TLC counts every enabled Init witness before symmetry or duplicate
        # elimination. BFS retains their union; induction checks the same union.
        self.invalid_sources = self.b.false
        self.initial = self.b.false
        self.initial_generated = 0
        for index, (guard, assignments) in enumerate(self.branches(doc["init"], {}, True, self.b.true, {})):
            require(index < 100000, "Init exceeds 100000 expanded witnesses")
            require(guard in (self.b.true, self.b.false), "nonconstant Init guard")
            if guard == self.b.false:
                continue
            bits = self.encode(assignments)
            require(all(v in (self.b.true, self.b.false) for v in bits.values()), "nonground Init")
            self.initial |= self.b.cube({name: value == self.b.true for name, value in bits.items()})
            self.initial_generated += 1
        require(self.initial_generated > 0, "empty Init")
        if self.initial_missing:
            raise Grow(self.initial_missing)
        self.reading_init = False
        self.constraint = self.conjunction(self.boolean(self.eval(expr, {}, {})) for expr in doc["constraints"])
        self.properties = [(name, self.boolean(self.eval(expr, {}, {}))) for name, expr in doc["invariants"]]
        updates = []
        for index, (guard, assigns) in enumerate(self.branches(doc["next"], {}, False, self.b.true, {})):
            if self.refining and index and index % 64 == 0:
                gc.collect()
            require(index < 100000, "transition exceeds 100000 expanded action witnesses")
            change = {n: v for n, v in self.encode(assigns, guard).items() if v != self.b.var(n)}
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
        self.effect_parts = {
            frozenset(change): self.partitions(dict(change), single_partition) for change in merged
        }
        self.actions = [
            (guard, self.effect_parts[frozenset(change)])
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
            return sorted(keys, key=repr)

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
                require(planned_bits <= 512, f"finite layout exceeds 512 state bits at {path[0]} ({planned_bits} bits so far)")
                for k in keys:
                    leaves.append(((*path, k), None))
                domains[tuple(keys)] += len(keys)
                return
            choices = tuple(self.static_set(bound))
            require(choices, "empty scalar domain")
            require(not set(choices) & permuted, "permuted atoms in a scalar field")
            planned_bits += max(1, (len(choices) - 1).bit_length())
            require(planned_bits <= 512, f"finite layout exceeds 512 state bits at {path[0]} ({planned_bits} bits so far)")
            domains[inner_domain] += 1
            leaves.append((path, choices))

        for name in self.names:
            describe(types[name], (name,), ())
        clusters = max((d for d in domains if d), key=domains.get, default=())
        atoms = tuple(sorted(group_atoms))

        # In a sparse network, messages often contain the same participants
        # that index other local-state fields. Keep related bits near one
        # another instead of separating the entire network from its endpoints.
        if self.whole_domains:
            axes = [domain for domain in domains if domain and all(isinstance(k, str) for k in domain)]
            clusters = max(axes, key=domains.get, default=())

        def model_values(key):
            if isinstance(key, str):
                yield key
            elif key[0] == "record":
                for _, field in key[1]:
                    yield from model_values(field)

        def order(leaf):
            path, _ = leaf
            group = next((clusters.index(k) for k in reversed(path[1:]) if k in clusters), -1)
            if self.whole_domains:
                group = next((clusters.index(atom) for key in reversed(path[1:])
                              for atom in model_values(key) if atom in clusters), -1)
            roles = [atoms.index(k) for k in path[1:] if k in atoms]
            require(len(roles) <= 1, "multiple permuted indices in one path")
            return group, (roles[0] if roles else -1), self.names.index(path[0]), tuple(map(repr, path))

        # Record relative path identities as well as bit positions. This lets a
        # permutation of outer OR inner function indices move the right fields.
        role_fields = {a: {} for a in atoms}
        for path, choices in sorted(
            leaves, key=(lambda leaf: (self.names.index(leaf[0][0]), tuple(map(repr, leaf[0])))) if order_by_path else order
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
                permute(self.initial) == self.initial
                and permute(self.constraint) == self.constraint
                and all(permute(p) == p for _, p in self.properties),
                "asymmetric Init, constraint or invariant",
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
        if isinstance(value, bool):
            return self.b.true if value else self.b.false
        if type(value) is int:
            return self.atom(self.integer(value))
        if isinstance(value, str):
            return self.atom(("string", value))
        require(isinstance(value, dict), "unsupported configuration value")
        if set(value) == {"model"}:
            require("_" not in value["model"], "typed model values")
            return self.atom(value["model"])
        if set(value) == {"set"}:
            return Members(((self.key(self.literal(v)), self.b.true) for v in value["set"]),
                           enumeration=Enumeration.REDUCIBLE)
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
        self.comparable(value)
        return sorted(value, key=repr)

    def boolean(self, value):
        require(isinstance(value, cudd.Function), "expected a Boolean expression")
        return value

    def any(self, terms):
        return ~self.conjunction(~self.boolean(t) for t in terms)

    def comparable(self, *groups):
        """Refuse a mixed scalar comparison that the nominal TLC profile can reject."""
        kinds = {}

        def visit(key, path):
            if isinstance(key, str) or self.padding(key):
                return  # Untyped model values compare with every class.
            kind, value = key
            require(kinds.setdefault(path, kind) == kind, "incomparable finite values")
            if kind == "record":
                for field, member in value:
                    visit(member, (*path, field))

        for group in groups:
            for key in group:
                visit(key, ())

    def same(self, a, b):
        require(type(a) is type(b), "mixed-type equality")
        if isinstance(a, (Atom, Members)):
            self.comparable(a, b)
            return self.conjunction(
                a.get(k, self.b.false).equiv(b.get(k, self.b.false)) for k in a.keys() | b.keys()
            )
        if isinstance(a, Function):
            self.comparable(a, b)
            if a.keys() != b.keys():
                return self.b.false
            return self.conjunction(self.same(v, b[k]) for k, v in a.items())
        return self.boolean(a).equiv(self.boolean(b))

    def subset(self, a, b):
        require(isinstance(a, Members) and isinstance(b, Members), "subset of non-set")
        self.comparable(a, b)
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
        self.comparable(value, space)
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

    def bindings(self, q, env, assigned, *, multiple=False, known_domains=None):
        slots = []
        domains = []
        for index, bound in enumerate(q["bounds"]):
            require(not bound["tuple"] and bound["domain"] is not None, "unbounded or tuple binder")
            domain = self.eval(bound["domain"], env, assigned) if known_domains is None else known_domains[index]
            require(isinstance(domain, Members), "binder over a non-atomic set")
            for slot in bound["vars"]:
                slots.append(slot)
                domains.append(domain)
        require(slots and (len(slots) == 1 or multiple), "expected a single bound variable")
        require(math.prod(map(len, domains)) <= 100000, "quantifier exceeds 100000 bindings")
        for values in itertools.product(*(domain.items() for domain in domains)):
            keys = tuple(k for k, _ in values)
            scope = {**env, **{slot: self.bound_value(k) for slot, k in zip(slots, keys)}}
            yield keys[0] if len(keys) == 1 else keys, self.conjunction(g for _, g in values), scope

    def bound_value(self, key):
        if isinstance(key, tuple) and key[0] == "boolean":
            return self.b.true if key[1] else self.b.false
        return self.atom(key)

    def let_scope(self, expr, env, assigned):
        local = dict(env)
        for idx in expr["defs"]:
            definition = self.ir["defs"][idx]
            scope = definition["scope"]
            require(isinstance(scope, dict) and "LetValue" in scope
                    and not definition["params"] and not definition["recursive"],
                    "non-value or recursive LET")
            local[scope["LetValue"]["slot"]] = self.eval(definition["body"], local, assigned)
        return local

    @staticmethod
    def padding(key):
        return isinstance(key, tuple) and key[0] == "padding"

    @staticmethod
    def integer(value):
        require(-(2**31) <= value < 2**31, "integer outside the nominal TLC range")
        return ("int", value)

    def cardinality_bound(self, members, bound):
        """Whether at most bound membership bits hold; omit larger partial sums."""
        require(isinstance(members, Members), "Cardinality requires a finite set")
        if bound < 0:
            return self.b.false
        if bound >= len(members):
            return self.b.true
        # more[i] means at least i+1 of the processed members are present.
        more = [self.b.false] * (bound + 1)
        for present in reversed(list(members.values())):
            more = [self.b.ite(present, more[i - 1] if i else self.b.true, old)
                    for i, old in enumerate(more)]
        return ~more[-1]

    def numeric(self, op, args):
        if op == "FiniteSetsCardinality":
            members, = args
            require(isinstance(members, Members), "Cardinality requires a finite set")
            counts = [self.b.true]
            for present in members.values():
                counts = [self.b.ite(present, counts[n - 1] if n else self.b.false,
                                     counts[n] if n < len(counts) else self.b.false)
                          for n in range(len(counts) + 1)]
            return Atom((self.integer(n), possible) for n, possible in enumerate(counts)
                        if possible != self.b.false)
        if op.startswith("Integers"):
            op = "Naturals" + op.removeprefix("Integers")
        arithmetic = {"NaturalsPlus": operator.add, "NaturalsMinus": operator.sub, "NaturalsTimes": operator.mul}
        compares = {"NaturalsLeq": operator.le, "NaturalsGeq": operator.ge,
                    "NaturalsLt": operator.lt, "NaturalsGt": operator.gt}
        require(op in arithmetic or op in compares or op == "NaturalsDotDot", "IR standard operator " + op)
        a, b = args
        require(isinstance(a, Atom) and isinstance(b, Atom), "integer operands for " + op)
        result = Atom()
        truth = self.b.false
        for left, lg in a.items():
            for right, rg in b.items():
                guard = lg & rg
                if guard == self.b.false:
                    continue
                # Spare bit patterns cannot be produced from a ground Init or
                # a valid assignment. Their arithmetic value is immaterial.
                if self.padding(left) or self.padding(right):
                    continue
                require(isinstance(left, tuple) and left[0] == "int"
                        and isinstance(right, tuple) and right[0] == "int", "integer operands for " + op)
                x, y = left[1], right[1]
                if op == "NaturalsDotDot":
                    require(guard == self.b.true and y - x < 16384, "nonconstant or oversized integer interval")
                    return Members(((self.integer(n), self.b.true) for n in range(x, y + 1)),
                                   enumeration=Enumeration.REDUCIBLE)
                if op in compares:
                    if compares[op](x, y):
                        truth |= guard
                else:
                    k = self.integer(arithmetic[op](x, y))
                    result[k] = result.get(k, self.b.false) | guard
        return truth if op in compares else result

    def eval(self, i, env, assigned):
        tag, x = self.node(i)

        def sub(expr):
            return self.eval(expr, env, assigned)

        if tag == "Let":
            return self.eval(x["body"], self.let_scope(x, env, assigned), assigned)
        if tag in ("SetFilter", "SetMap"):
            q = x if tag == "SetMap" else {"bounds": [x["bound"]]}
            domains = None
            enumeration = Enumeration.REDUCIBLE  # A mapped set is an explicit enumeration.
            if tag == "SetFilter":
                require(x["bound"]["domain"] is not None, "unbounded set filter")
                domain = sub(x["bound"]["domain"])
                require(isinstance(domain, Members), "filter over a non-atomic set")
                domains = [domain]
                enumeration = domain.enumeration
            result = Members(enumeration=enumeration)
            for key, active, scope in self.bindings(q, env, assigned, known_domains=domains):
                if active == self.b.false:
                    continue
                if tag == "SetFilter":
                    result[key] = active & self.boolean(self.eval(x["pred"], scope, assigned))
                else:
                    elem = self.eval(x["expr"], scope, assigned)
                    require(isinstance(elem, Atom), "set map of compound values")
                    for k, g in elem.items():
                        result[k] = result.get(k, self.b.false) | (active & g)
            return result
        if tag == "Num":
            return self.atom(self.integer(int(x)))
        if tag == "Apply" and "Std" in x["callee"]:
            if x["callee"]["Std"] in ("NaturalsLeq", "IntegersLeq"):
                lhs, rhs = [arg["Expr"]["expr"] for arg in x["args"]]
                ltag, left = self.node(lhs)
                if ltag == "Apply" and left["callee"] == {"Std": "FiniteSetsCardinality"}:
                    upper = sub(rhs)
                    if isinstance(upper, Atom) and len(upper) == 1:
                        key, guard = next(iter(upper.items()))
                        if guard == self.b.true and isinstance(key, tuple) and key[0] == "int":
                            return self.cardinality_bound(sub(left["args"][0]["Expr"]["expr"]), key[1])
            return self.numeric(x["callee"]["Std"], [sub(arg["Expr"]["expr"]) for arg in x["args"]])
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
        if tag in ("Record", "RecordSet"):
            require(not self.doc["symmetry"], "symmetry on record-valued finite domains")
            fields = sorted(x, key=lambda field: field["name"])
            names = [f["name"] for f in fields]
            require(len(names) == len(set(names)), "duplicate record field")
            values = [sub(f["value"]) for f in fields]
            kind = Atom if tag == "Record" else Members
            require(all(isinstance(v, kind) for v in values), "record fields must have finite scalar values")
            total = math.prod(len(v) for v in values)
            require(total <= 16384, "record domain exceeds 16384 alternatives")
            result = kind()
            if tag == "RecordSet" and all(v.enumeration != Enumeration.UNPROVED for v in values):
                result.enumeration = Enumeration.UNIQUE
            for choices in itertools.product(*(v.items() for v in values)):
                guard = self.conjunction(bit for _, bit in choices)
                if guard != self.b.false:
                    fields_key = tuple((n, k) for n, (k, _) in zip(names, choices))
                    result[("record", fields_key)] = guard
            return result
        if tag == "RecordSelect":
            records = sub(x["record"])
            require(isinstance(records, Atom), "field selection on a non-record")
            result = Atom()
            for record, guard in records.items():
                if self.padding(record):
                    continue
                require(isinstance(record, tuple) and record[0] == "record",
                        "field selection outside a finite record domain")
                fields = dict(record[1])
                require(x["field"] in fields, "record lacks field " + x["field"])
                key = fields[x["field"]]
                result[key] = result.get(key, self.b.false) | guard
            return result
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
            out = Members(enumeration=Enumeration.REDUCIBLE)
            for expr in x:
                atom = sub(expr)
                if isinstance(atom, cudd.Function):
                    atom = Atom({("boolean", False): ~atom, ("boolean", True): atom})
                require(isinstance(atom, Atom), "set of nonscalars")
                for k, g in atom.items():
                    out[k] = out.get(k, self.b.false) | g
            self.comparable(out)
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
            cond = self.boolean(sub(x["cond"]))
            if cond == self.b.true:
                return sub(x["then"])
            if cond == self.b.false:
                return sub(x["other"])
            return self.select(cond, sub(x["then"]), sub(x["other"]))
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

                self.comparable(a, b)

                def merge(k):
                    u, v = a.get(k, self.b.false), b.get(k, self.b.false)
                    return u | v if op == "Cup" else (u & v if op == "Cap" else u & ~v)

                result = Members((k, merge(k)) for k in a.keys() | b.keys())
                if self.reading_init:
                    result.enumeration = self.set_enumeration(op, a, b)
                return result
            raise Unsupported("IR builtin " + op)
        raise Unsupported("IR expression " + tag)

    def set_enumeration(self, op, a, b):
        # Tool.getInitStates uses elements(), which can retain duplicates.
        # A lazy union concatenates its operands; a filter or difference
        # walks its source. Prior normalization can remove any duplicates.
        left, right = a.enumeration, b.enumeration
        if op == "SetMinus":
            return left
        if op == "Cap":
            if Enumeration.REDUCIBLE in (left, right):
                return Enumeration.REDUCIBLE
            return left
        if left == right == Enumeration.REDUCIBLE:
            return Enumeration.REDUCIBLE
        if Enumeration.UNPROVED not in (left, right) and all(
            (a[k] & b[k]) == self.b.false for k in a.keys() & b.keys()
        ):
            return Enumeration.UNIQUE
        return Enumeration.UNPROVED

    def branches(self, i, env, init, guard, assigned, *, action_conjunct=False):
        if guard == self.b.false:
            return
        tag, x = self.node(i)
        if tag == "Let":
            yield from self.branches(x["body"], self.let_scope(x, env, assigned), init, guard, assigned)
        elif tag == "Apply" and "Std" in x["callee"]:
            require(x["callee"]["Std"] in ("NaturalsLeq", "NaturalsGeq", "NaturalsLt", "NaturalsGt"), "nondeterministic standard action guard")
            yield guard & self.boolean(self.eval(i, env, assigned)), assigned
        elif tag in ("Def", "Apply") and not (tag == "Def" and x in self.overrides):
            body, args = self.call(tag, x, env, assigned)
            yield from self.branches(body, args, init, guard, assigned)
        elif tag == "Junction":
            items = x["items"]
            if not init and x["kind"] == "Or" and self.ir["exprs"][i]["level"] in ("State", "Constant"):
                # A literal disjunction inside an action conjunction enumerates
                # its arms until all variables are bound. A completed assignment
                # can instead evaluate a pending state predicate as one Boolean.
                can_count_arms = (action_conjunct and len(assigned) < len(self.names)
                                  and self.ir["exprs"][i]["level"] == "State")
                alternatives = []
                overlapping = False
                combined = self.b.false
                for item in items:
                    tag, v = self.node(item)
                    require(tag in ("Const", "Bool") or (tag == "Builtin" and v["op"] in ("Eq", "Neq", "In", "NotIn", "Subseteq")),
                            "unsupported action guard: " + tag)
                    alt = self.boolean(self.eval(item, env, assigned))
                    if alt & combined & guard != self.b.false:
                        require(can_count_arms, "unsupported action guard: overlapping boolean alternatives")
                        overlapping = True
                    alternatives.append(alt)
                    combined |= alt
                if overlapping:
                    for alt in alternatives:
                        if guard & alt != self.b.false:
                            yield guard & alt, assigned
                else:
                    yield guard & combined, assigned
            elif x["kind"] == "Or":
                for item in items:
                    yield from self.action_alternatives(item, env, init, guard, assigned)
            else:
                require(x["kind"] == "And", "unknown action junction")

                def conjunction(index, active, values):
                    if index == len(items):
                        yield active, values
                    else:
                        for g, a in self.branches(items[index], env, init, active, values,
                                                  action_conjunct=self.ir["exprs"][i]["level"] == "Action"):
                            yield from conjunction(index + 1, g, a)

                yield from conjunction(0, guard, assigned)
        elif tag == "Quant" and x["kind"] == "Exists":
            for _, g, scope in self.bindings(x, env, assigned, multiple=True):
                yield from self.action_alternatives(x["body"], scope, init, guard & g, assigned)
        elif tag == "Unchanged":
            require(not init, "UNCHANGED in Init")
            kind, items = self.node(x)
            while kind == "Def" and items not in self.overrides:
                x, _ = self.call(kind, items, env, assigned)
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
            init and tag == "Builtin" and x["op"] == "In"
            and self.node(x["args"][0])[0] == "Var"
            and not self.node(x["args"][0])[1]["primed"]
            and (name := self.names[self.node(x["args"][0])[1]["var"]]) not in assigned
        ):
            domain = self.eval(x["args"][1], env, assigned)
            require(isinstance(domain, Members), "Init choice over a non-atomic set")
            require(domain.enumeration != Enumeration.UNPROVED,
                    "Init membership may enumerate duplicate witnesses in a lazy set")
            self.comparable(domain)
            for key, included in domain.items():
                yield guard & included, {**assigned, name: self.atom(key)}
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
                tag in ("Bool", "Not", "Const")
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

    def encode(self, assigned, guard=None):
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
                require(isinstance(value, Members), "expected a finite set")
                invalid = self.b.false
                for k in value.keys() - schema.keys():
                    bit = value[k] if guard is None or not self.refining else value[k] & guard
                    if bit == self.b.false:
                        continue
                    if k in self.whole_domains.get(path, ()):
                        if guard is None:
                            self.initial_missing.add((path, k))
                        else:
                            ident = (path, k)
                            self.missing[ident] = self.missing.get(ident, self.b.false) | bit
                    else:
                        invalid |= bit
                if guard is None or not self.refining:
                    require(invalid == self.b.false, "set assignment outside finite layout")
                else:
                    self.invalid_sources |= guard & invalid
                for k in schema:
                    fields, _ = self.layout[(*path, k)]
                    out[fields[0].var] = value.get(k, self.b.false)
            else:
                fields, alphabet = self.layout[path]
                require(isinstance(value, Atom), "scalar assignment outside finite layout")
                invalid = self.any(v for k, v in value.items() if k not in alphabet)
                if guard is None or not self.refining:
                    require(invalid == self.b.false, "scalar assignment outside finite layout")
                else:
                    self.invalid_sources |= guard & invalid
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
            if key[0] == "boolean":
                return key[1]
            if key[0] == "record":
                return {"record": {n: scalar(v) for n, v in key[1]}}
            require(key[0] in ("string", "int"), "invalid encoded scalar in witness")
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
                    target = layers[depth - 1] & self.constraint & guard & pred
                    if target != self.b.false:
                        break
                require(target != self.b.false, "missing predecessor")
        return dict(format="dive-trace", version=1, variables=self.names, states=path[::-1], ending=ending)

    def same_encoding_and_admission(self, other):
        # The same manager makes equality of Boolean functions exact. Layout
        # includes concrete paths and enum encodings, even on unused fields.
        return (
            self.b is other.b
            and self.types == other.types
            and self.layout == other.layout
            and self.names == other.names
            and self.invalid_sources == other.invalid_sources
            and self.constraint == other.constraint
            and self.initial == other.initial
            and self.roles == other.roles
            and self.symmetry == other.symmetry
        )

    def same_relation(self, other):
        return (self.same_encoding_and_admission(other)
                and self.initial_generated == other.initial_generated
                and self.effects == other.effects)

    def recheck_effects(self, previous, reached, generated):
        """Preserve distances when changed edges stutter or land in an admitted root."""
        generated += self.initial_generated - previous.initial_generated
        if self.effects == previous.effects:
            return generated

        # An effect is a deterministic simultaneous assignment. For each edited
        # assignment, compare the union of enabling guards on reachable sources.
        # Self-loops and edges into Init never precede a shortest path to a new state.
        removed = previous.effects - self.effects
        added = self.effects - previous.effects
        for update, removed_guard, added_guard in self.changed_guards(previous):
            changed = reached & (removed_guard | added_guard)
            if changed == self.b.false or not any(
                changed & ~self.b.var(bit).equiv(value) != self.b.false for bit, value in update
            ):
                continue
            # Substitute simultaneously into the root predicate. Every changed
            # edge from the remaining sources must leave its own source fixed.
            roots = self.initial & self.constraint
            nonroot = changed & ~self.b.let(dict(update), roots)
            if any(nonroot & ~self.b.var(bit).equiv(value) != self.b.false for bit, value in update):
                raise ChangedRelation

        # Duplicate witnesses can change the generated count even when every
        # destination is identical. Symmetry counts representative outdegrees.
        if self.symmetry:
            return self.counts(reached, True, self.initial_generated)[1]
        for sign, effects in ((-1, removed), (1, added)):
            for (guard, _), multiplicity in effects.items():
                generated += sign * multiplicity * self.state_count(reached & guard)
        return generated

    def changed_guards(self, previous):
        """Compare enabling sets per deterministic update, independently of witness count."""
        different = self.effects.keys() ^ previous.effects.keys()
        affected = {update for _, update in different}

        def enabled(effects):
            guards = dict.fromkeys(affected, self.b.false)
            for guard, update in effects:
                if update in guards:
                    guards[update] |= guard
            return guards

        before, after = enabled(previous.effects), enabled(self.effects)
        for update in affected:
            yield update, before[update] & ~after[update], after[update] & ~before[update]

    def layer_delta(self, previous):
        removed, added = [], []
        for update, old, new in self.changed_guards(previous):
            if old != self.b.false:
                removed.append((old, previous.effect_parts[update]))
            if new != self.b.false:
                added.append((new, self.effect_parts[update]))
        return removed, added

    def violations(self, states):
        admitted = states & self.constraint
        missing = {key for key, predicate in self.missing.items() if (admitted & predicate) != self.b.false}
        if missing:
            raise Grow(missing)
        require(admitted & self.invalid_sources == self.b.false,
                "reachable source emits an assignment outside finite layout")
        for name, prop in self.properties:
            if states & ~prop != self.b.false:
                yield dict(kind="invariant", name=name), ~prop
        if self.doc["check_deadlock"]:
            bad = self.constraint & ~self.any(g for g, _ in self.actions)
            if states & bad != self.b.false:
                yield dict(kind="deadlock"), bad

    def image(self, states, guard, updates):
        result = states & guard
        for relation, old, rename in updates:
            result = self.b.let(rename, cudd.and_exists(result, relation, old))
        return result

    def recheck_refined(self, previous, keep_layers=False):
        if previous is None:
            if keep_layers:
                return self.check_refined_layers(None)
            return self.refined_reachability()
        if (not hasattr(previous, "refined_result") or not self.same_encoding_and_admission(previous)
                or self.whole_domains != previous.whole_domains
                or self.missing != previous.missing):
            # Release the old closure before exploring a different relation.
            raise ChangedRelation

        reached, depth, distinct, generated = previous.refined_result
        try:
            generated = self.recheck_effects(previous, reached, generated)
        except ChangedRelation:
            if keep_layers and getattr(previous, "complete", None) is not None:
                return self.check_refined_layers(previous)
            raise
        proved = {prop for _, prop in previous.properties}
        added = [prop for _, prop in self.properties if prop not in proved]
        if added:
            # Excluded Init and emissions are subject to invariants, even though
            # they do not enter the reached set or contribute outgoing actions.
            bad = ~self.conjunction(added)
            if ((self.initial | reached) & bad != self.b.false
                    or any(self.image(reached, guard, parts) & bad != self.b.false
                           for guard, parts in self.actions)):
                return self.check()
        if self.doc["check_deadlock"] and (not previous.doc["check_deadlock"] or self.effects != previous.effects):
            enabled = self.any(guard for guard, _ in self.actions)
            if reached & ~enabled != self.b.false:
                return self.check()
        self.refined_result = reached, depth, distinct, generated
        if keep_layers and previous.complete is not None:
            all_states, layers, _, _ = previous.complete
            self.complete = all_states, layers, distinct, generated
        return self.success(depth, reached, distinct, generated, True), None

    def check_refined_layers(self, previous):
        row, trace = self.check(previous, collect_cycles=True)
        if trace is None:
            all_states, _, distinct, generated = self.complete
            self.refined_result = all_states & self.constraint, row["depth"], distinct, generated
        return row, trace

    def refined_reachability(self):
        """Check all emissions, retaining just admitted states during the first pass."""
        reached = frontier = self.initial & self.constraint
        if any(self.violations(self.initial)):
            return self.check()
        invariant = self.conjunction(prop for _, prop in self.properties)
        depth = 0
        while frontier != self.b.false:
            gc.collect()
            depth += 1
            if any(self.violations(frontier)):
                return self.check()
            successors = self.b.false
            for guard, parts in self.actions:
                emitted = self.image(frontier, guard, parts)
                if emitted & ~invariant != self.b.false:
                    # Repeat with ordered layers for an independently replayed shortest witness.
                    return self.check()
                successors |= emitted & self.constraint
            frontier = successors & ~reached
            reached |= frontier
        distinct, generated = self.counts(reached, self.symmetry, self.initial_generated)
        depth = max(depth, 1)
        self.refined_result = reached, depth, distinct, generated
        return self.success(depth, reached, distinct, generated, False), None

    def check(self, previous=None, collect_cycles=False):
        self.complete = None
        if previous is not None and previous.complete is not None and self.same_relation(previous):
            reached, layers, distinct, generated = previous.complete
            if any(self.violations(reached)):
                for depth, layer in enumerate(layers, 1):
                    for ending, bad in self.violations(layer):
                        return self.failure(layers[:depth], layer & bad, ending, reused=True)
            self.complete = previous.complete
            return self.success(self.depth(layers), reached & self.constraint, distinct, generated, True), None

        old_layers = ()
        if previous is not None and previous.complete is not None and self.same_encoding_and_admission(previous):
            old_layers = previous.complete[1]
            removed, added = self.layer_delta(previous)

        reached = frontier = self.initial
        layers = []
        reused_images = 0
        while frontier != self.b.false:
            if collect_cycles:
                gc.collect()
            layers.append(frontier)
            for ending, bad in self.violations(frontier):
                row, trace = self.failure(layers, frontier & bad, ending, reused=False)
                if reused_images:
                    row["reused_layer_images"] = reused_images
                return row, trace
            # Include excluded emissions in the witness layers but never expand them.
            sources = frontier & self.constraint
            if old_layers:
                # A deleted edge into an earlier layer cannot remove a shortest path.
                if all(self.image(sources, guard, parts) & ~reached == self.b.false
                       for guard, parts in removed):
                    next_old = old_layers[len(layers)] if len(layers) < len(old_layers) else self.b.false
                    extra = self.any(self.image(sources, guard, parts) for guard, parts in added)
                    if not removed:
                        # Every old layer is reachable by its old depth. Earlier
                        # arrivals and new states need their full outgoing image;
                        # the other old edges already lead into reached or next_old.
                        early = sources & ~old_layers[len(layers) - 1]
                        if early != self.b.false:
                            extra |= self.any(self.image(early, guard, parts) for guard, parts in self.actions)
                    frontier = (next_old | extra) & ~reached
                    reached |= frontier
                    reused_images += 1
                    if len(layers) == len(old_layers) or (removed and frontier != next_old):
                        old_layers = ()
                    continue
                old_layers = ()
            # All earlier admitted layers have been expanded under this relation.
            # Their successors are already in reached. Either BDD therefore
            # yields exactly the same next frontier after subtracting reached.
            admitted = reached & self.constraint
            if admitted.dag_size < sources.dag_size:
                sources = admitted
            successors = self.b.false
            for guard, updates in self.actions:
                successors |= self.image(sources, guard, updates)
            frontier = successors & ~reached
            reached |= frontier
        distinct, generated = self.counts(reached & self.constraint, self.symmetry, self.initial_generated)
        self.complete = reached, layers, distinct, generated
        row = self.success(self.depth(layers), reached & self.constraint, distinct, generated, False)
        if reused_images:
            row["reused_layer_images"] = reused_images
        return row, None

    def depth(self, layers):
        return max(1, len(layers) - int(layers[-1] & self.constraint == self.b.false))

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


class RefinementWorker:
    """One request at a time; only a completed successful pass keeps its manager."""

    def __init__(self, args):
        flags = ["-Es"] if sys.flags.ignore_environment else []
        options = {**vars(args), "keep_refined_layers": bool(args.recheck), "recheck": [], "trace": None, "domain_pass": True}
        self.command = [sys.executable, *flags, __file__, json.dumps(options)]
        self.child = None

    def close(self):
        if self.child is not None:
            # EOF releases any saved proof. The global supervisor owns cancellation.
            with suppress(BrokenPipeError):
                self.child.stdin.close()
            self.child.stdout.close()
            status = self.child.wait()
            size = self.errors.seek(0, 2)
            self.errors.seek(max(size - 500, 0))
            diagnostic = self.errors.read().decode(errors="replace").strip()
            self.errors.close()
            self.child = None
            return status, diagnostic
        return 0, ""

    def request(self, module, config, learned):
        if self.child is None:
            self.errors = tempfile.TemporaryFile()
            try:
                self.child = subprocess.Popen(self.command, stdin=subprocess.PIPE,
                                              stdout=subprocess.PIPE, stderr=self.errors, text=True)
            except BaseException:
                self.errors.close()
                raise
        with suppress(BrokenPipeError):
            self.child.stdin.write(json.dumps([module, config, list(learned.values())]) + "\n")
            self.child.stdin.flush()
        line = self.child.stdout.readline()
        reply = None
        if line.endswith("\n"):
            with suppress(json.JSONDecodeError):
                reply = json.loads(line)
        if isinstance(reply, dict) and reply.get("verdict") == "success" and isinstance(reply.get("domain_members"), list):
            return 0, reply, ""
        # Growth, relation changes, validated failures and errors terminate the pass.
        status, diagnostic = self.close()
        return status, reply, diagnostic


def refine_one(args, worker, module, config, hints, trace, last):
    started = time.monotonic()
    learned = hints.copy()
    discarded_hints = False
    for attempt in itertools.count(1):
        resident = worker.child is not None
        status, row, diagnostic = worker.request(module, config, learned)
        if status == 0 and last:
            status, diagnostic = worker.close()
            if status != 0:
                row = None
        if status == 7 and resident:
            # Retry the edited relation with hints but without the old manager.
            continue
        native_oom = status == 1 and args.max_memory and "out of memory" in diagnostic.lower()
        if (hints or resident) and not discarded_hints and (status < 0 or status in (2, 4) or native_oom):
            learned.clear()
            discarded_hints = True
            continue
        if status < 0:
            raise Stopped(f"refinement worker terminated by signal {-status}: {diagnostic}")
        if status == 6:
            if not isinstance(row, list) or not row:
                raise RuntimeError("invalid domain growth response from refinement worker")
            added = False
            for item in row:
                key = json.dumps(item)
                if key not in learned:
                    learned[key] = item
                    added = True
            require(added, "no progress refining finite set domains")
            if len(learned) > 512 and hints and not discarded_hints:
                learned.clear()
                discarded_hints = True
                continue
            require(len(learned) <= 512, "refined set domains exceed 512 state bits")
            continue
        if status == 1:
            if native_oom:
                raise Stopped("refinement worker exhausted memory during native allocation")
            raise RuntimeError("native refinement worker exited before returning a check result")
        if status == 4 and not isinstance(row, dict):
            raise Stopped("refinement worker stopped before reporting its result")
        if status not in (0, 2, 3, 4, 11) or not isinstance(row, dict):
            raise RuntimeError(f"refinement worker ended without a result (exit {status})")
        if status == 0 and (row.get("verdict") != "success" or not isinstance(row.get("domain_members"), list)):
            raise RuntimeError("refinement worker ended without a completed closure")
        if status == 11:
            if row.get("trace_validated") is not True or not isinstance(row.get("trace"), dict):
                raise RuntimeError("refinement worker ended without a validated witness")
            # Publish only after the validating worker exited successfully. A partial
            # response or failed warm attempt must never overwrite a user's trace.
            if trace:
                induction.write_atomic(Path(trace), (json.dumps(row.pop("trace")) + "\n").encode())
                row["trace_file"] = trace
        selected = row.pop("domain_members", None)
        current = {} if selected is None else {json.dumps(item): item for item in selected}
        row.update(module=module, config=config, refinement_rounds=attempt,
                   learned_members=len(learned) if selected is None else len(current),
                   initial_hint_members=0 if discarded_hints else len(hints.keys() & current.keys()),
                   seconds=round(time.monotonic() - started, 6))
        print(json.dumps(row), flush=True)
        return (1 if status == 11 else status), current


def refined_search(args):
    require(not (args.verify_predicate or args.prove_inductive or args.write_predicate),
            "set-domain refinement supports reachability and validated traces")
    hints = {}
    violated = False
    worker = RefinementWorker(args)
    try:
        revisions = [(args.module, args.config), *args.recheck]
        for index, (module, config) in enumerate(revisions, 1):
            status, members = refine_one(args, worker, module, config, hints,
                                         None if violated else args.trace, index == len(revisions))
            if status not in (0, 1):
                return status
            violated |= status == 1
            hints = members if status == 0 else {}
    finally:
        worker.close()
    return int(violated)


def run(args):
    if args.max_memory:
        _, hard = resource.getrlimit(resource.RLIMIT_AS)
        limit = args.max_memory << 20
        if hard != resource.RLIM_INFINITY:
            limit = min(limit, hard)
        resource.setrlimit(resource.RLIMIT_AS, (limit, hard))
    if args.refine_domains and not getattr(args, "domain_pass", False):
        return refined_search(args)
    if args.refine_domains:
        # CUDD returns an unreferenced result before dd allocates its Python
        # wrapper. Collect cyclic compiler objects only between native calls.
        # Refcounting stays active throughout this single-pass worker.
        gc.disable()

        def key(obj):
            return tuple(map(key, obj)) if isinstance(obj, list) else obj

        def requests():
            for line in sys.stdin:
                module, config, coordinates = json.loads(line)
                yield module, config, set(map(key, coordinates))

        return check_in_manager(args, requests())
    revisions = [(module, config, None) for module, config in [(args.module, args.config), *args.recheck]]
    if args.verify_predicate or args.prove_inductive:
        for revision in revisions:
            status = check_in_manager(args, [revision])
            if status:
                return status
        return 0
    return check_in_manager(args, revisions)


def check_in_manager(args, revisions):
    # Release cyclic Python node wrappers while their native manager is still alive.
    # Bind the first input before allocating CUDD so a frontend refusal survives
    # small resource limits. This owner outlives every Python node wrapper.
    owner = []
    try:
        return check_revisions(args, revisions, owner)
    finally:
        gc.collect()


def check_revisions(args, revisions, owner):
    previous = None
    violated = False
    for module, config, learned in revisions:
        start = time.monotonic()
        exported = run_helper([args.exporter, module, config], "Dive frontend", args.max_memory)
        require(exported.returncode == 0, "Dive frontend: " + exported.stderr.strip())
        doc = json.loads(exported.stdout)
        if not owner:
            owner.append(cudd.BDD(memory_estimate=512 << 20))
        model = SourceModel(
            doc,
            args.type_invariant,
            owner[0],
            args.order_by_path,
            args.single_partition,
            learned,
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
        row, trace = (model.recheck_refined(previous, getattr(args, "keep_refined_layers", False))
                      if args.refine_domains else model.check(previous))
        if args.refine_domains:
            # Private child-to-parent payload. Retire stale coordinates on each edit.
            row["domain_members"] = sorted(
                ((path, k) for path in model.whole_domains for k in model.types[path[0]].domain),
                key=repr,
            )
        if args.write_predicate and model.complete is not None:
            induction.store(Path(args.write_predicate), model, model.complete[0])
        if trace is not None:
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
            if args.trace and not violated:
                induction.write_atomic(Path(args.trace), (raw + "\n").encode("utf-8"))
                row["trace_file"] = str(args.trace)
            else:
                row["trace"] = trace
            violated = True
        row.update(format="dive-symbolic-report-v1", module=str(module), config=str(config))
        row["seconds"] = round(time.monotonic() - start, 6)
        print(json.dumps(row), flush=True)
        if args.refine_domains and trace is not None:
            return 1
        previous = model
        if args.refine_domains:
            gc.collect()
    return int(violated)


def main():
    try:
        return run(argparse.Namespace(**json.loads(sys.argv[1])))
    except ChangedRelation:
        print("null", flush=True)
        return 7
    except Grow as growth:
        extra, = growth.args
        print(json.dumps(sorted(extra, key=repr)), flush=True)
        return 6
    except Stopped as err:
        print(json.dumps(dict(verdict="stopped", reason=str(err))), flush=True)
        return 4
    except MemoryError:
        print(json.dumps(dict(verdict="stopped", reason="memory exhausted")), flush=True)
        return 4
    except (Unsupported, ValueError, KeyError, IndexError, TypeError, OSError, RecursionError) as err:
        print(json.dumps(dict(verdict="unsupported", reason=str(err))), flush=True)
        return 2
    except Exception as err:
        # An internal or CUDD failure can never publish a safety verdict.
        print(json.dumps(dict(verdict="error", reason=f"{type(err).__name__}: {err}")), flush=True)
        return 3


if __name__ == "__main__":
    status = main()
    # CUDD can call exit(1) on allocation failure, bypassing Python. Reserve a
    # private process status for a validated violation that reached main().
    sys.exit(11 if status == 1 else status)
