"""Prove conjugacy of every finite safety witness on the bound labeled program."""
import argparse
from collections import Counter
import hashlib
import itertools
import json
from pathlib import Path
import sys

from dd import cudd
from engine import Atom, Function, Members, PermutedModel, Unsupported, finite_group, require
from worker_process import limit_memory


class FiniteProof(PermutedModel):
    """Use the finite compiler only for value shapes and operations whose demand is certified."""

    def __init__(self, doc, layout, atoms):
        require(doc["format"] == "dive-orbit-proof-input-v1" and doc["symmetry"] is None,
                "expected original bound finite safety roles without symmetry")
        require(not doc["constraints"] and not doc["overrides"], "constraints or redirected roles")
        self.initial_occurrences = Counter()
        self.unbound = {i for i, value in enumerate(doc["constants"])
                        if isinstance(value, dict) and "unbound" in value}
        declaration = {"atoms": atoms, "maps": list(itertools.permutations(range(len(atoms))))}
        labeled = {**doc, "format": "dive-symbolic-input-v2", "symmetry": declaration}
        super().__init__(labeled, layout, None, False, False, None)
        _, _, generators = finite_group(declaration)
        rename = {name: self.b.var(name + "_next") for name in self.variables}

        def prime(value):
            if isinstance(value, (Function, Atom, Members)):
                return type(value)((key, prime(member)) for key, member in value.items())
            return self.b.let(rename, value) if rename else value

        incoming = {name: prime(value) for name, value in self.state.items()}
        steps = [(name, self.step(expr, incoming)) for name, expr in doc["step_properties"]]
        for generator in generators:
            substitution = self.permutations[generator]
            def swapped(node):
                return self.b.let(substitution, node) if substitution else node
            renamed = Counter()
            for cube, count in self.initial_occurrences.items():
                renamed[swapped(cube)] += count
            require(renamed == self.initial_occurrences, "asymmetric initial occurrence multiset")
            both = {**substitution, **{name + "_next": prime(value) for name, value in substitution.items()}}
            for name, predicate in steps:
                require(not both or self.b.let(both, predicate) == predicate, "asymmetric step property " + name)
        self.summary = dict(method="finite-role-conjugacy", state_bits=len(self.variables),
                            initial_occurrences=self.initial_generated, occurrence_partitions=sum(self.effects.values()),
                            generators=len(generators), invariants=[name for name, _ in self.properties],
                            step_properties=[name for name, _ in steps])

    def literal(self, value):
        if isinstance(value, dict) and "unbound" in value:
            return None   # An inactive INSTANCE declaration. Every attempted read below refuses.
        return super().literal(value)

    def eval(self, i, env, assigned):
        tag, x = self.node(i)
        require(tag != "Const" or x not in self.unbound, "unbound constant in a configured proof role")
        # The general compiler can lower additional representations and arithmetic.
        # Certify this total fragment only. A failed function lookup, ill-typed value,
        # unexpanded choice or incomparable equality is an explicit compiler refusal.
        require(tag in {"Var", "Local", "Const", "Def", "Apply", "Str", "Bool", "Num", "SetEnum",
                        "FcnCons", "FcnSet", "FcnApply", "Except", "At", "Quant", "Builtin",
                        "Record", "RecordSet", "Junction", "If", "Not", "Implies"},
                "unproved definedness for " + tag)
        if tag == "Apply":
            require("Def" in x["callee"] or x["callee"].get("Std") == "FiniteSetsCardinality",
                    "unproved definedness for " + str(x["callee"]))
        if tag == "Quant":
            require(x["kind"] in ("Forall", "Exists"), "unproved choice or temporal binder")
        result = super().eval(i, env, assigned)
        if isinstance(result, (Atom, Members)):
            self.comparable(result)
            for key in result:
                self.require_scalar_or_record(key)
        return result

    def require_scalar_or_record(self, key):
        if isinstance(key, str) or self.padding(key):
            return
        kind, value = key
        if kind == "record":
            require(value, "unproved empty record representation")
            for _, field in value:
                self.require_scalar_or_record(field)
        else:
            require(kind in ("boolean", "string", "int"), "unproved comparison domain " + kind)

    def same(self, left, right):
        # The ordinary engine may check the shapes of all keys and all cell
        # values before comparing any particular pair. Even different domains
        # can demand a diagnostic. Per-cell mathematical equality is insufficient.
        require(not isinstance(left, Function) and not isinstance(right, Function),
                "unproved whole-function comparison shape")
        return super().same(left, right)

    def pair_exclusion(self, q, env, assigned):
        # Inspect every bound body for definedness, including optimized patterns.
        return None

    def record_initial(self, cube):
        super().record_initial(cube)
        self.initial_occurrences[cube] += 1

    def step(self, index, incoming):
        tag, subscript = self.node(index)
        require(tag == "StepBox", "unsupported bound step property")
        def unchanged(index):
            tag, args = self.node(index)
            if tag == "Def":
                body, env = self.call(tag, args, {}, {})
                require(not env, "parameterized step subscript")
                return unchanged(body)
            if tag == "Tuple":
                return self.conjunction(unchanged(term) for term in args)
            require(tag == "Var" and not args["primed"], "expected variables in step subscript")
            name = self.names[args["var"]]
            return self.same(self.state[name], incoming[name])
        action = self.boolean(self.eval(subscript["action"], {}, incoming))
        return action | unchanged(subscript["sub"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--layout")
    parser.add_argument("--memory-mib", type=int, default=2048)
    parser.add_argument("--identity", action="store_true")
    parser.add_argument("atoms", nargs="*")
    args = parser.parse_args()
    if args.identity:
        files = [Path(__file__).with_name(n) for n in ("engine.py", "orbit_proof.py", "finite_relation.py", "worker_process.py", "induction.py")]
        files.extend([Path(cudd.__file__), Path(sys.executable)])
        digests = []
        for path in files:
            with path.open("rb") as stream:
                digests.append(hashlib.file_digest(stream, "sha256").hexdigest())
        print(json.dumps(digests))
        return
    limit_memory(args.memory_mib)
    try:
        proof = FiniteProof(json.load(sys.stdin), args.layout, args.atoms)
    except (Unsupported, ValueError, KeyError, TypeError, IndexError, OverflowError, RecursionError, MemoryError) as error:
        sys.exit("finite orbit proof refused: " + str(error))
    print(json.dumps(proof.summary))


if __name__ == "__main__":
    main()
