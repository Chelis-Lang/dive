#!/usr/bin/env python3
"""Discover finite strengthening clauses from an independently replayed CTI."""

import argparse
from copy import deepcopy
import itertools
import json
from pathlib import Path
import re
from shutil import copyfile
import subprocess
from tempfile import TemporaryDirectory
from time import monotonic


class Inconclusive(ValueError):
    pass


def literal(value, model_names=None):
    if type(value) is bool:
        return "TRUE" if value else "FALSE"
    if isinstance(value, (str, int)):
        return json.dumps(value)
    nested = lambda item: literal(item, model_names)
    if "model" in value:
        return model_names[value["model"]]
    if "seq" in value:
        return "<<" + ", ".join(map(nested, value["seq"])) + ">>"
    if "set" in value:
        return "{" + ", ".join(map(nested, value["set"])) + "}"
    if "fcn" in value:
        pairs = [f"({nested(key)} :> {nested(item)})" for key, item in value["fcn"]]
        return "(" + " @@ ".join(pairs) + ")" if pairs else r"[diveKey \in {} |-> diveKey]"
    if "record" in value:
        return "[" + ", ".join(f"{key} |-> {nested(item)}" for key, item in value["record"].items()) + "]"
    raise Inconclusive("unsupported value in the typed counterexample")


def atoms(ir, roots):
    def node(index):
        expr = ir["exprs"][index]["expr"]
        return next(iter(expr.items())) if isinstance(expr, dict) else (expr, None)

    def definition(index):
        tag, data = node(index)
        if tag == "Def":
            return ir["defs"][data]
        if tag == "Apply" and "Def" in data["callee"]:
            return ir["defs"][data["callee"]["Def"]]
        return None

    def walk(root, allowed_helpers=None):
        pending = [root]
        visited = set()
        while pending:
            index = pending.pop()
            if index in visited:
                continue
            visited.add(index)
            yield index
            tag, data = node(index)
            called = definition(index)
            if called is not None:
                if (allowed_helpers is None or not called["name"].startswith("H_")
                        or called["name"] in allowed_helpers):
                    pending.append(called["body"])
            elif tag in ("Quant", "Let"):
                pending.append(data["body"])
                if tag == "Let":
                    pending.extend(ir["defs"][def_id]["body"] for def_id in data["defs"])
            elif tag == "Junction":
                pending.extend(data["items"])
            elif tag in ("Implies", "Equiv"):
                pending.extend(data)
            elif tag == "Not":
                pending.append(data)
            elif tag == "If":
                pending.extend(data[key] for key in ("cond", "then", "other"))

    definitions = {row["name"]: row["body"] for row in ir["defs"] if row["scope"] == "TopLevel"}
    target, action = (definitions[name] for name in roots)
    target_nodes = list(walk(target))
    target_helpers = {called["name"] for index in target_nodes
                      if (called := definition(index)) is not None and called["name"].startswith("H_")}

    def term(index):
        tag, data = node(index)
        if tag in ("Num", "Str", "Bool"):
            return literal(data) if tag != "Num" else data
        if tag == "Var" and not data["primed"]:
            return ir["decls"][ir["vars"][data["var"]]]["name"]
        if tag == "Const":
            return ir["decls"][ir["consts"][data]]["name"]
        if tag == "Local" and data["up"] == 0:
            return f"_diveSlot{data['slot']}_"
        if tag == "Def" and ir["defs"][data]["scope"] == "TopLevel":
            name = ir["defs"][data]["name"]
            if not name.startswith("H_") or name in target_helpers:
                return name
        if tag == "FcnApply":
            return f"{term(data['fcn'])}[{term(data['arg'])}]"
        if tag == "Record":
            return "[" + ", ".join(f"{row['name']} |-> {term(row['value'])}" for row in data) + "]"
        if tag == "SetEnum":
            return "{" + ", ".join(map(term, data)) + "}"
        raise Inconclusive("expression outside atom grammar")

    names = [ir["decls"][index]["name"] for index in ir["vars"]]
    reads = re.compile(r"\b(?:" + "|".join(map(re.escape, names)) + r")\b")
    slot = re.compile(r"_diveSlot\d+_")
    result = set()
    operators = {"In": r"\in", "NotIn": r"\in", "Eq": "=", "Neq": "="}
    for index in itertools.chain(target_nodes, walk(action, target_helpers)):
        tag, data = node(index)
        if tag != "Builtin" or data["op"] not in operators or len(data["args"]) != 2:
            continue
        try:
            left, right = map(term, data["args"])
        except Inconclusive:
            continue
        op = operators[data["op"]]
        if op == "=":
            left, right = sorted((left, right))
        atom = f"{left} {op} {right}"
        variables = sorted(set(slot.findall(atom)))
        if len(variables) > 2 or (not reads.search(atom) and len(variables) != 2):
            continue
        for order in itertools.permutations(("diveI", "diveJ"), len(variables)):
            rename = dict(zip(variables, order, strict=True))
            result.add(slot.sub(lambda match: rename[match[0]], atom))
    if not 1 <= len(result) <= 32:
        raise Inconclusive("expected 1 to 32 source atoms in the finite grammar")
    return sorted(result)


class Trial:
    def __init__(self, args, directory):
        self.args = args
        self.deadline = monotonic() + args.seconds
        self.module = directory / "DiveInductionTrial.tla"
        self.cfg = self.module.with_suffix(".cfg")
        self.trace = directory / "word.json"
        self.constants = args.constants.read_text()
        self.domain = args.domain.read_text()
        for source in [args.module, *args.dependency]:
            copyfile(source, directory / source.name)
        self.checks = 0

    def execute(self, command, input=None):
        remaining = self.deadline - monotonic()
        if remaining < 0.001:
            raise Inconclusive("discovery deadline")
        try:
            completed = subprocess.run(command, input=input, text=True, capture_output=True, timeout=remaining)
        except subprocess.TimeoutExpired as exc:
            raise Inconclusive("discovery deadline") from exc
        try:
            row = json.loads(completed.stdout)
        except ValueError as exc:
            raise Inconclusive(completed.stderr.strip() or "checker returned no report") from exc
        return completed.returncode, row

    def prepare(self, candidate, *, init="TrialInit", next_action="TrialNext", goal=None, extra="", bindings=""):
        args = self.args
        source = fr"""---- MODULE DiveInductionTrial ----
EXTENDS {args.module.stem}, TLC
VARIABLE diveStep
InductionDomain == {self.domain}
Strengthening == {candidate}
TrialInit == /\ InductionDomain /\ {args.target} /\ Strengthening /\ diveStep = FALSE
CoverageInit == /\ InductionDomain /\ {args.target} /\ diveStep = FALSE
OriginalInit == /\ {args.init} /\ diveStep = FALSE
TrialNext == /\ ~diveStep /\ {args.action} /\ diveStep' = TRUE
TrialHold == UNCHANGED <<{args.variables}, diveStep>>
Coverage == Strengthening \/ ENABLED ({args.action} /\ ~{args.target}')
"""
        self.module.write_text(source + extra + "\n====\n")
        self.cfg.write_text(f"{self.constants}\n{bindings}\nINIT {init}\nNEXT {next_action}\n"
                            f"INVARIANT {goal or args.target}\nCHECK_DEADLOCK FALSE\n")

    def check(self, candidate, **options):
        self.checks += 1
        self.prepare(candidate, **options)
        budget = max(1, int((self.deadline - monotonic()) * 1000))
        code, row = self.execute([self.args.dive, "check", self.module, "--config", self.cfg,
                                  "--store", "exact", "--workers", "1", "--seed", "0", "--fp", "0",
                                  "--time-limit", f"{budget}ms", "--exit-codes", "native", "--output", "json"])
        if code not in (0, 10) or row["verdict"] not in ("success", "invariant-violation"):
            diagnostics = "; ".join(item["message"] for item in row.get("diagnostics", []))
            raise Inconclusive(row.get("message") or diagnostics or row["verdict"])
        if code == 0 and row["statistics"]["left_on_queue"] != 0:
            raise Inconclusive("finite check did not exhaust")
        return row

    def replay(self, word):
        self.trace.write_text(json.dumps(word))
        _, row = self.execute([self.args.dive, "validate-trace", self.module, "--config", self.cfg,
                               "--trace", self.trace, "--output", "json"])
        return row["result"]

    def cti(self, row, candidate, *, goal=None):
        word = row["counterexample"]["trace"]
        if len(word["states"]) != 2 or self.replay(word) != "valid":
            raise Inconclusive("could not reproduce a two-state induction counterexample")
        damaged = deepcopy(word)
        damaged["states"][1]["values"] = dict(word["states"][0]["values"], diveStep=True)
        if self.replay(damaged) != "invalid":
            raise Inconclusive("damaged counterexample was accepted")
        self.prepare(candidate, init="OriginalInit", goal=goal)
        original = self.replay(word)
        return word, original

    def patterns(self, word, features):
        values = word["states"][0]["values"]
        def collect(value):
            if isinstance(value, dict):
                if "model" in value:
                    yield value["model"]
                else:
                    for child in value.values():
                        yield from collect(child)
            elif isinstance(value, list):
                for child in value:
                    yield from collect(child)
        models = {value: f"DiveModelValue{i}" for i, value in enumerate(sorted(set(collect(values))))}
        state = " /\\ ".join(f"{name} = {literal(value, models)}" for name, value in values.items())
        vector = "<<" + ", ".join(f"({atom})" for atom in features) + ">>"
        declarations = "CONSTANT " + ", ".join(models.values()) + "\n" if models else ""
        bindings = "\n".join(f"CONSTANT {name} = {value}" for value, name in models.items())
        extra = (f"{declarations}VARIABLE diveMask\nMaskInit == /\\ {state}\n"
                 f" /\\ diveMask = {{{vector} : diveI,diveJ \\in {self.args.carrier}}}\n"
                 f"MaskHold == UNCHANGED <<{self.args.variables},diveStep,diveMask>>\nMaskFalse == diveStep\n")
        row = self.check("TRUE", init="MaskInit", next_action="MaskHold", goal="MaskFalse", extra=extra, bindings=bindings)
        masks = row["counterexample"]["trace"]["states"][0]["values"]["diveMask"]["set"]
        return sorted(tuple(mask["seq"]) for mask in masks)

    def discover(self):
        start = monotonic()
        baseline = self.check("TRUE")
        if baseline["verdict"] != "invariant-violation":
            raise Inconclusive("the finite target and action have no counterexample to induction")
        word, original_replay = self.cti(baseline, "TRUE")
        first_cti_seconds = monotonic() - start
        _, document = self.execute([self.args.exporter, self.module, self.cfg])
        features = atoms(document["ir"], (self.args.target, self.args.action))
        clauses = []
        formula = "TRUE"

        def merged(literals):
            clause = f"\\A diveI,diveJ \\in {self.args.carrier} : ~( " + r" /\ ".join(literals) + " )"
            return r" /\ ".join(f"({item})" for item in [*clauses, clause]), clause

        def covers(literals):
            candidate, _ = merged(literals)
            return self.check(candidate, init="CoverageInit", next_action="TrialHold", goal="Coverage")["verdict"] == "success"

        for _ in range(8):
            for mask in self.patterns(word, features):
                literals = [f"({atom})" if present else f"~({atom})" for atom, present in zip(features, mask, strict=True)]
                if covers(literals):
                    break
            else:
                raise Inconclusive("source atom grammar cannot distinguish an unsafe prestate")
            for feature in literals.copy():
                smaller = [value for value in literals if value != feature]
                if smaller and covers(smaller):
                    literals = smaller
            formula, clause = merged(literals)
            clauses.append(clause)
            step = self.check(formula)
            if step["verdict"] == "success":
                break
            word, _ = self.cti(step, formula)
        else:
            raise Inconclusive("eight strengthening clauses did not close the finite step")

        stats = step["statistics"]
        if stats["generated"] <= stats["initial_generated"]:
            raise Inconclusive("strengthening leaves no enabled action in the typed domain")
        initial = self.check(formula, init="OriginalInit", next_action="TrialHold", goal="Strengthening")
        if initial["verdict"] != "success":
            raise Inconclusive("strengthening fails original Init inclusion")
        recheck_start = monotonic()
        preservation = self.check(formula, goal="Strengthening")
        preservation_witness = None
        if preservation["verdict"] == "invariant-violation":
            _, original = self.cti(preservation, formula, goal="Strengthening")
            preservation_witness = {
                "states": 2, "hypothesis_replay": "valid",
                "original_init_replay": original, "damaged_replay": "invalid",
            }
        recheck_seconds = monotonic() - recheck_start
        edit_start = monotonic()
        omitted = self.check("TRUE")
        if omitted["verdict"] != "invariant-violation":
            raise Inconclusive("removing the strengthening did not reproduce the original failure")
        _, original_omission = self.cti(omitted, "TRUE")
        return {"result": "finite-strengthening", "candidate": formula, "atom_count": len(features),
                "candidate_checks": self.checks, "first_replayed_cti_s": first_cti_seconds,
                "checked_candidate_s": recheck_start - start, "recheck_s": recheck_seconds,
                "omit_and_replay_s": monotonic() - edit_start,
                "baseline_cti": {"states": 2, "hypothesis_replay": "valid", "original_init_replay": original_replay,
                                 "damaged_replay": "invalid"},
                "omitted_cti": {"states": 2, "hypothesis_replay": "valid", "original_init_replay": original_omission,
                                "damaged_replay": "invalid"},
                "checked": {"target": self.args.target, "action": self.args.action, "Init": "included",
                            "target_preservation": "proved on typed domain", "safe_prestate_coverage": "proved",
                            "nonvacuous": stats["generated"] - stats["initial_generated"],
                            "candidate_preservation": preservation["verdict"],
                            "candidate_preservation_witness": preservation_witness, "counts": stats}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("--dependency", action="append", type=Path, default=[])
    parser.add_argument("--target", required=True)
    parser.add_argument("--action", required=True)
    parser.add_argument("--variables", default="vars")
    parser.add_argument("--init", default="Init")
    parser.add_argument("--carrier", required=True)
    parser.add_argument("--constants", type=Path, required=True)
    parser.add_argument("--domain", type=Path, required=True)
    parser.add_argument("--dive", type=Path, required=True)
    parser.add_argument("--exporter", type=Path, required=True)
    parser.add_argument("--seconds", type=int, default=60)
    args = parser.parse_args()
    if args.seconds <= 0:
        parser.error("expected a positive total budget")
    try:
        with TemporaryDirectory() as temporary:
            result = Trial(args, Path(temporary)).discover()
    except (Inconclusive, KeyError, OSError, TypeError, ValueError) as exc:
        print(json.dumps({"result": "inconclusive", "reason": str(exc)}))
        return 4
    print(json.dumps(result))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
