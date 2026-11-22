#!/usr/bin/env python3
"""Check a compact source predicate by finite induction against the original bound Next."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from dd import cudd
from engine import Atom, Function, Unsupported, require
from induction import write_atomic
from property_proof import PropertyModel


def canonical(document):
    return json.dumps(document, separators=(",", ":"), sort_keys=True, allow_nan=False).encode()


def digest(document):
    # Diagnostic display paths change when a client moves the checked input.
    # Retain file order, module identity, selected source bytes and every IR edge.
    files = [{key: value for key, value in file.items() if key != "display"}
             for file in document["ir"]["files"]]
    bound = {**document, "ir": {**document["ir"], "files": files}}
    return hashlib.sha256(canonical(bound)).hexdigest()


class InductionModel(PropertyModel):
    """Retain out-of-layout source guards until the inductive domain is known."""

    def layout_escape(self, path, outside, guard, reason):
        if guard is None or self.reading_init:
            return super().layout_escape(path, outside, guard, reason)
        if outside:
            self.invalid_sources |= guard & self.any(outside.values())

    def eval(self, index, env, assigned):
        tag, node = self.node(index)
        if tag != "FcnApply":
            return super().eval(index, env, assigned)
        value = self.eval(node["fcn"], env, assigned)
        key = self.key(self.eval(node["arg"], env, assigned))
        if isinstance(value, Function):
            require(key in value, "index outside finite function domain")
            return value[key]
        require(isinstance(value, Atom) and isinstance(key, tuple) and key[0] == "int",
                "expected a finite function or integer-indexed tuple")
        result = Atom()
        for item, guard in value.items():
            if guard == self.b.false:
                continue
            require(isinstance(item, tuple) and item[0] == "tuple" and 1 <= key[1] <= len(item[1]),
                    "tuple index outside finite domain")
            member = item[1][key[1] - 1]
            result[member] = result.get(member, self.b.false) | guard
        return result

    def branches(self, index, env, init, guard, assigned, *, action_conjunct=False):
        tag, node = self.node(index)
        if tag == "Apply" and node["callee"].get("Std") in {
                "IntegersLt", "IntegersLeq", "IntegersGt", "IntegersGeq"}:
            if guard != self.b.false:
                yield guard & self.boolean(self.eval(index, env, assigned)), assigned
            return
        yield from super().branches(index, env, init, guard, assigned, action_conjunct=action_conjunct)


def action_roots(model):
    tag, node = model.node(model.doc["next"])
    if tag != "Junction" or node["kind"] != "Or":
        return [("Next", model.doc["next"])]
    roots = []
    for index in node["items"]:
        kind, child = model.node(index)
        name = model.ir["defs"][child]["name"] if kind == "Def" else f"Next[{len(roots) + 1}]"
        roots.append((name, index))
    return roots


def verify(document, layout, candidate, property_name):
    model = InductionModel(document, layout, property_name, cudd.BDD(memory_estimate=512 << 20))
    require(not model.counters, "the layout must cover every original variable")
    expr = model.layout_expression(candidate)
    tag, node = model.node(expr)
    parts = node["items"] if tag == "Junction" and node["kind"] == "And" else [expr]
    clauses = [model.boolean(model.eval(part, {}, {})) for part in parts]
    predicate = model.conjunction(clauses)
    domain = model.conjunction(~model.encoding(fields, idx)
                               for fields, choices in model.layout.values() if choices
                               for idx, key in enumerate(choices) if model.padding(key))
    candidate_domain = predicate & domain
    admitted = candidate_domain & model.constraint
    _, target = model.properties[0]

    def discharge(bad, obligation):
        if bad != model.b.false:
            state = model.decode(model.b.pick(bad, care_vars=set(model.variables)))
            raise Unsupported(f"undischarged {obligation}; symbolic source valuation {state}")

    discharge(model.initial & ~target, "initial safety")
    discharge(model.initial & ~candidate_domain, "initiation")
    discharge(admitted & ~target, "safety implication")
    discharge(admitted & model.invalid_sources, "complete finite destination layout")
    rows = []
    for action, root in action_roots(model):
        row = dict(action=action, alternatives=0, occurrences=0, outside_constraint=0)
        for guard, writes in model.branches(root, {}, False, model.b.true, {}):
            source = admitted & guard
            if source == model.b.false:
                continue
            updated = {name: val for name, val in model.encode(writes, guard).items()
                       if val != model.b.var(name)}
            destination_in = model.b.let(updated, model.constraint)
            kept = source & destination_in
            discharge(source & ~model.b.let(updated, target), action + " destination safety")
            for i, term in enumerate(clauses, 1):
                discharge(kept & ~model.b.let(updated, term), f"{action} preservation clause {i}")
            row["alternatives"] += 1
            row["occurrences"] += model.state_count(source)
            row["outside_constraint"] += model.state_count(source & ~destination_in)
        rows.append(row)
    return dict(format="source-named-induction-v1", property=property_name, candidate=candidate, layout=layout,
                input_sha256=digest(document), clauses=len(clauses), state_bits=len(model.variables),
                candidate_bdd_nodes=predicate.dag_size, admitted_valuations=model.state_count(admitted),
                initial_valuations=model.state_count(model.initial), actions=rows)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--exporter", required=True, type=Path)
    parser.add_argument("--layout", required=True)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--property", required=True)
    receive = parser.add_mutually_exclusive_group()
    receive.add_argument("--save", type=Path)
    receive.add_argument("--verify", type=Path)
    args = parser.parse_args()
    try:
        run = subprocess.run([args.exporter.resolve(), args.module.resolve(), args.config.resolve()],
                             capture_output=True, check=False)
        require(run.returncode == 0, run.stderr.decode(errors="replace") or "source export refused")
        document = json.loads(run.stdout)
        config_digest = hashlib.sha256(args.config.read_bytes()).hexdigest()
        if args.verify:
            require(args.verify.stat().st_size <= 1 << 20, "oversize named induction certificate")
            old = json.loads(args.verify.read_text())
            require(isinstance(old, dict) and old.get("input_sha256") == digest(document)
                    and old.get("config_sha256") == config_digest,
                    "certificate belongs to a different source or configuration")
        result = verify(document, args.layout, args.candidate, args.property)
        result["config_sha256"] = config_digest
        if args.verify:
            require(canonical(old) == canonical(result), "certificate differs from independently checked obligations")
        if args.save:
            write_atomic(args.save, (json.dumps(result, sort_keys=True) + "\n").encode())
        print(json.dumps({"result": "proved", **result}))
    except (Unsupported, OSError, ValueError, MemoryError) as error:
        print(json.dumps({"result": "inconclusive", "reason": str(error)}))
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
