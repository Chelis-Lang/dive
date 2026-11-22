"""Prove one configured invariant using finite closure and isolated integer counters."""

from dataclasses import dataclass
import gc
import json
from pathlib import Path
import time

from dd import cudd
from engine import Atom, Enumeration, Function, Members, SourceModel, require
import induction
import obligation_receipt
from worker_process import run_helper


@dataclass(frozen=True)
class IntegerUpdate:
    relative: bool
    value: int


class PropertyModel(SourceModel):
    """Each projected counter is an independent, total integer recurrence."""

    cluster_record_fields = True

    def __init__(self, doc, layout, prop, manager):
        selected = [(name, body) for name, body in doc["invariants"] if name == prop]
        require(len(selected) == 1, "expected one configured invariant named " + prop)
        # This proof checks the labeled relation. Symmetry is a search reduction,
        # so it can be omitted once the exporter has bound the original config.
        self.counters = set()
        self.closed_state_predicates = {
            definition["body"] for definition in doc["ir"]["defs"]
            if definition["scope"] == "TopLevel" and definition["level"] == "State" and not definition["params"]
        }
        self.closed_guard_values = {}
        self.initial_recipes = []
        self.step_recipes = []
        self.trace_points = []
        super().__init__({**doc, "invariants": selected, "check_deadlock": False, "symmetry": None}, layout, manager)

    def layout_expression(self, name):
        defs = [i for i, d in enumerate(self.ir["defs"]) if d["name"] == name and d["scope"] == "TopLevel"]
        require(len(defs) == 1, "expected one source layout operator named " + name)
        expr, _ = self.call("Def", defs[0], {}, {})
        return expr

    def complete_layout(self, found):
        self.counters = set(self.names) - found.keys()
        # Keep a singleton coordinate for trace decoding and completeness of each
        # assignment. Only the base, step and UNCHANGED rules below may assign it.
        for name in self.counters:
            found[name] = Members([(self.integer(0), self.b.true)])
        return found

    def eval(self, i, env, assigned):
        tag, x = self.node(i)
        if tag == "Var":
            name = self.names[x["var"]]
            require(name not in self.counters, "projected integer is read: " + name)
        if tag == "Builtin" and x == {"op": "Boolean", "args": []}:
            return Members([(("boolean", flag), self.b.true) for flag in (False, True)],
                           enumeration=Enumeration.REDUCIBLE)
        # A nullary operator denotes one Boolean function on this revision's
        # fixed state bits. Retain it only after layout/Init construction, when
        # there are no lexical arguments or pending primed assignments.
        if not self.reading_init and not env and not assigned and i in self.closed_state_predicates:
            if i in self.closed_guard_values:
                return self.closed_guard_values[i]
            value = super().eval(i, env, assigned)
            if isinstance(value, cudd.Function):
                self.closed_guard_values[i] = value
            return value
        return super().eval(i, env, assigned)

    def boolean(self, value):
        if isinstance(value, Atom) and set(value) == {("boolean", False), ("boolean", True)}:
            return value[("boolean", True)]
        return super().boolean(value)

    def encode(self, assigned, guard=None):
        def as_scalar(value):
            if isinstance(value, cudd.Function):
                return Atom({("boolean", False): ~value, ("boolean", True): value})
            if isinstance(value, Function):
                return Function((key, as_scalar(member)) for key, member in value.items())
            return value
        values = {name: as_scalar(value) for name, value in assigned.items()}
        recipes = {}
        for name in self.counters:
            if isinstance(values.get(name), IntegerUpdate):
                recipes[name] = values[name]
                values[name] = self.atom(self.integer(0))
        bits = super().encode(values, guard)
        if self.counters:
            if guard is None:
                self.initial_recipes.append((self.b.cube({k: v == self.b.true for k, v in bits.items()}), recipes))
            else:
                changes = {k: v for k, v in bits.items() if self.b.var(k) != v}
                self.step_recipes.append((guard, changes, recipes))
        return bits

    def decode(self, point):
        if self.counters:
            self.trace_points.append(point)
        return super().decode(point)

    def witness(self, layers, target, ending):
        self.trace_points.clear()
        trace = super().witness(layers, target, ending)
        if not self.counters:
            return trace
        points = self.trace_points[::-1]
        first = next((updates for cube, updates in self.initial_recipes
                      if self.b.let(points[0], cube) == self.b.true), None)
        require(first is not None and first.keys() == self.counters, "missing initial counter values")
        integers = {name: update.value for name, update in first.items()}
        trace["states"][0]["values"].update(integers)
        for index, (old, new) in enumerate(zip(points, points[1:]), 1):
            for guard, change, updates in self.step_recipes:
                if self.b.let(old, guard) != self.b.true:
                    continue
                if any((self.b.let(old, change[k]) == self.b.true if k in change else old[k]) != new[k]
                       for k in self.variables):
                    continue
                for name, update in updates.items():
                    integers[name] = integers[name] + update.value if update.relative else update.value
                trace["states"][index]["values"].update(integers)
                break
            else:
                raise ValueError("missing transition for concrete counter trace")
        return trace

    def branches(self, i, env, init, guard, assigned, *, action_conjunct=False):
        tag, x = self.node(i)
        if tag == "Builtin" and x["op"] == "Eq":
            left, right = x["args"]
            kind, var = self.node(left)
            if kind == "Var" and var["primed"] != init and self.names[var["var"]] in self.counters:
                name = self.names[var["var"]]
                require(name not in assigned, "duplicate assignment " + name)
                kind, value = self.node(right)
                if kind == "Num":
                    update = IntegerUpdate(False, int(value))
                else:
                    require(not init and kind == "Apply" and value["callee"] in
                            ({"Std": "IntegersPlus"}, {"Std": "NaturalsPlus"}),
                            "expected isolated integer assignment for " + name)
                    args = [self.node(arg["Expr"]["expr"]) for arg in value["args"]]
                    require(len(args) == 2 and args[0] == ("Var", {"primed": False, "var": var["var"]})
                            and args[1][0] == "Num", "expected isolated integer increment for " + name)
                    update = IntegerUpdate(True, int(args[1][1]))
                yield guard, {**assigned, name: update}
                return
        if not init and tag in ("Var", "FcnApply"):
            yield guard & self.boolean(self.eval(i, env, assigned)), assigned
            return
        yield from super().branches(i, env, init, guard, assigned, action_conjunct=action_conjunct)


def certify(model, states, checked):
    """Check Init, property and all new guarded images against this exact predicate."""
    if model.initial & ~states != model.b.false:
        return "Init inclusion", None
    for ending, _ in model.violations(states):
        return ending["name"], None
    actions = set()
    for guard, parts in model.actions:
        key = (guard, tuple((relation, tuple(old), tuple(sorted(rename.items())))
                            for relation, old, rename in parts))
        if key in actions:
            continue
        if key not in checked and model.image(states & model.constraint, guard, parts) & ~states != model.b.false:
            return "Next closure", None
        actions.add(key)
    return None, actions


def support(model):
    """Close the property bits under guards and simultaneous assignment dependencies."""
    relevant = set(model.constraint.support)
    for _, predicate in model.properties:
        relevant.update(predicate.support)
    while True:
        before = len(relevant)
        for guard, changes in model.effects:
            if any(name in relevant for name, _ in changes):
                relevant.update(guard.support)
                for name, expr in changes:
                    if name in relevant:
                        relevant.update(expr.support)
        if len(relevant) == before:
            return relevant


def closure(model, relevant):
    # Existentially forget unrelated finite coordinates after each image. Give
    # the resulting cylinder to certify(), which checks every full action.
    omitted = set(model.variables) - relevant
    def project(states):
        return model.b.exist(omitted, states) if omitted else states

    reached = project(model.initial)
    rounds = 0
    while True:
        if any(model.violations(reached)):
            # Reconstruct a concrete path using the original Init, guards and
            # assignments. Never decode arbitrary values from a projected cube.
            layers = []
            seen = frontier = model.initial
            while frontier != model.b.false:
                layers.append(frontier)
                for ending, bad in model.violations(frontier):
                    return None, model.witness(layers, bad & frontier, ending), rounds
                following = model.any(model.image(frontier & model.constraint, g, p) for g, p in model.actions)
                frontier = following & ~seen
                seen |= frontier
            raise ValueError("projected violation has no reachable witness")
        before = reached
        for guard, parts in model.actions:
            reached |= project(model.image(reached & model.constraint, guard, parts))
        rounds += 1
        if reached == before:
            return reached, None, rounds


def check_properties(args):
    manager = cudd.BDD(memory_estimate=512 << 20)
    try:
        return check_revisions(args, manager)
    finally:
        gc.collect()


def check_revisions(args, manager):
    previous = states = None
    checked = set()
    for module, config in [(args.module, args.config), *args.recheck]:
        start = time.monotonic()
        bound = run_helper([args.exporter, module, config], "Dive frontend", args.max_memory)
        require(bound.returncode == 0, "Dive frontend: " + bound.stderr.strip())
        model = PropertyModel(json.loads(bound.stdout), args.type_invariant, args.property_proof, manager)
        row = dict(format="dive-property-proof-v1", module=str(module), config=str(config),
                   property=args.property_proof, projected_integers=sorted(model.counters))
        relevant = support(model)
        row["support_bits"] = len(relevant)
        row["support_variables"] = sorted({path[0] for path, (bits, _) in model.layout.items()
                                           if any(bit.var in relevant for bit in bits)})
        compatible = previous is not None and model.same_encoding_and_admission(previous) and model.counters == previous.counters
        _, actions = certify(model, states, checked) if compatible else (None, None)
        reused = 0
        if args.verify_predicate:
            candidate, signed = induction.load(Path(args.verify_predicate), model, include_receipt=True)
            receipt = obligation_receipt.for_candidate(model, candidate, args) if signed is not None else None
            checked = receipt.checked(signed) if receipt is not None else set()
            failed, actions = certify(model, candidate, checked)
            row.update(proof="inductive-safety" if actions is not None else "inconclusive",
                       failed_obligation=failed,
                       closure_sweeps=0, reused_obligations=len(actions & checked) if actions is not None else 0,
                       transition_obligations=len(actions) if actions is not None else None,
                       seconds=round(time.monotonic() - start, 6))
            print(json.dumps(row), flush=True)
            return 0 if actions is not None else 2
        if actions is not None:
            reused = len(actions & checked)
            row["closure_sweeps"] = 0
        else:
            states, trace, row["closure_sweeps"] = closure(model, relevant)
            if trace is not None:
                raw = json.dumps(trace)
                validated = run_helper([args.dive, "validate-trace", module, "--trace", "/dev/stdin",
                                        "--config", config, "--output", "json"], "Dive trace validator",
                                       args.max_memory, input=raw)
                require(validated.returncode == 0 and json.loads(validated.stdout)["result"] == "valid",
                        "projected witness could not be validated on the original model")
                row.update(verdict="invariant-violation", ending=trace["ending"], depth=len(trace["states"]),
                           trace_validated=True, seconds=round(time.monotonic() - start, 6))
                if args.trace:
                    induction.write_atomic(Path(args.trace), (raw + "\n").encode())
                    row["trace_file"] = str(args.trace)
                else:
                    row["trace"] = trace
                print(json.dumps(row), flush=True)
                return 1
            failed, actions = certify(model, states, set())
            require(actions is not None, f"candidate failed induction: {failed}")
        if args.write_predicate:
            receipt = obligation_receipt.for_candidate(model, states, args)
            signed = receipt.encode(actions) if receipt is not None else None
            induction.store(Path(args.write_predicate), model, states, signed)
        row.update(proof="inductive-safety", failed_obligation=None,
                   transition_obligations=len(actions), reused_obligations=reused,
                   seconds=round(time.monotonic() - start, 6))
        print(json.dumps(row), flush=True)
        previous, checked = model, actions
    return 0
