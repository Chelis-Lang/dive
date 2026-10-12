#!/usr/bin/env python3
"""Compare dive's PlusCal source ASTs (JSON on stdin) with pinned TLC projections.

Each `<input>.projection.json` is the Java source-grammar AST of the pinned
`tla2tools.jar`, projected by the original capture tooling into the shared
comparison view. `native_project` below projects dive's AST into that view.
Run by `tests/oracle.rs`; prints a JSON summary of outcomes.
"""
import copy
from collections import Counter
import json
from pathlib import Path
import sys


def native_project(parsed):
    def expr(value):
        if value is None:
            return []
        span = value["span"]
        return [t["text"] for t in parsed["tokens"] if span["start"] <= t["span"]["start"] < span["end"]]

    def decl(value):
        initial = value["initial"]
        return [value["name"]["text"], initial["kind"] if initial else "Equal", expr(initial["expression"]) if initial else None]

    def sequence(block):
        output = []
        for ident in block["statements"]:
            value = parsed["statements"][ident]
            label = value["label"]["name"]["text"] if value["label"] else ""
            variant = value["kind"]
            kind = variant if isinstance(variant, str) else next(iter(variant))
            data = None if isinstance(variant, str) else variant[kind]
            if kind == "Block":
                children = sequence(data)
                if label:
                    if children[0][0]:
                        raise ValueError("duplicate native block labels")
                    children[0][0] = label
                output.extend(children)
                continue
            if kind == "Assign":
                node = [kind, [[a["variable"]["text"], expr(a["selectors"]), expr(a["value"])] for a in data]]
            elif kind == "While":
                node = [kind, expr(data["test"]), sequence(data["body"])]
            elif kind == "If":
                tail = sequence(data["otherwise"]) if data["otherwise"] else []
                for branch in reversed(data["branches"]):
                    tail = [["", ["If", expr(branch["test"]), sequence(branch["body"]), tail]]]
                node = tail[0][1]
            elif kind == "Either":
                node = [kind, [sequence(b) for b in data]]
            elif kind == "With":
                tail = sequence(data["body"])
                for binder in reversed(data["bindings"]):
                    tail = [["", ["With", binder["name"]["text"], binder["binding"]["kind"], expr(binder["binding"]["expression"]), tail]]]
                node = tail[0][1]
            elif kind == "Await":
                node = ["When", expr(data["condition"])]
            elif kind in ("Assert", "Print"):
                node = ["PrintS" if kind == "Print" else kind, expr(data)]
            elif kind in ("Skip", "Return"):
                node = [kind]
            elif kind == "Goto":
                node = [kind, data["text"].strip('"')]
            elif kind in ("Call", "MacroCall"):
                node = [kind, data["name"]["text"], [expr(a) for a in data["arguments"]]]
            else:
                raise ValueError("unaccounted native source node: " + kind)
            output.append([label, node])
        return output

    algorithm = parsed["algorithm"]
    def procedure(p):
        return [p["name"]["text"], [decl(d) for d in p["parameters"]], [decl(d) for d in p["variables"]], sequence(p["body"])]

    def process(p):
        return [p["name"]["text"], p["fairness"], p["id"]["kind"], expr(p["id"]["expression"]), [decl(d) for d in p["variables"]], sequence(p["body"])]

    body = algorithm["body"]
    return {"name": algorithm["name"]["text"], "syntax": algorithm["syntax"], "fair": algorithm["fairness"] == "Weak",
            "declarations": [decl(d) for d in algorithm["variables"]], "definitions": expr(algorithm["definitions"]) if algorithm["definitions"] else None,
            "macros": [[m["name"]["text"], [p["text"] for p in m["parameters"]], sequence(m["body"])] for m in algorithm["macros"]],
            "procedures": [procedure(p) for p in algorithm["procedures"]],
            "body": ["Processes", [process(p) for p in body["Processes"]]] if "Processes" in body else ["Sequential", sequence(body["Sequential"])]}


def source_integrity(parsed, source):
    if parsed["source"].encode() != source:
        raise ValueError("native parser changed source")
    def walk(value):
        if isinstance(value, list):
            for child in value:
                walk(child)
        elif isinstance(value, dict):
            if set(value) == {"start", "end"}:
                a, b = value["start"], value["end"]
                if not isinstance(a, int) or not isinstance(b, int) or not 0 <= a <= b <= len(source):
                    raise ValueError("invalid source span")
                source[:a].decode(); source[:b].decode()
            else:
                for child in value.values():
                    walk(child)
    walk(parsed)
    previous = 0
    for token in parsed["tokens"]:
        span = token["span"]
        if span["start"] < previous or source[span["start"]:span["end"]].decode() != token["text"]:
            raise ValueError("invalid lexical spelling/order")
        previous = span["end"]
    # Every arena statement has exactly one owner, with no cycles or dropped nodes.
    seen = set()
    def block(value):
        for ident in value["statements"]:
            if not isinstance(ident, int) or ident in seen or not 0 <= ident < len(parsed["statements"]):
                raise ValueError("invalid statement arena")
            seen.add(ident)
            stmt = parsed["statements"][ident]["kind"]
            if isinstance(stmt, str):
                continue
            kind, data = next(iter(stmt.items()))
            if kind == "Block":
                block(data)
            elif kind in ("While", "With"):
                block(data["body"])
            elif kind == "If":
                for branch in data["branches"]:
                    block(branch["body"])
                if data["otherwise"]:
                    block(data["otherwise"])
            elif kind == "Either":
                for branch in data:
                    block(branch)
    algorithm = parsed["algorithm"]
    for node in algorithm["macros"] + algorithm["procedures"]:
        block(node["body"])
    body = algorithm["body"]
    if "Processes" in body:
        for process in body["Processes"]:
            block(process["body"])
    else:
        block(body["Sequential"])
    if len(seen) != len(parsed["statements"]):
        raise ValueError("unowned source statement")


def outcome(row, native, left, source):
    if native.get("stage") != "source-grammar":
        return {"outcome": "disagree", "reason": "native stage not source grammar"}
    if "ast" in native:
        if native.get("translation") != "unsupported: pending":
            return {"outcome": "disagree", "reason": "native source AST claims translation"}
        source_integrity(native["ast"], source)
    else:
        if "error" not in native or native.get("code") not in {"DIVE-P0001", "DIVE-P0002", "DIVE-P0003", "DIVE-P0004", "DIVE-P0040", "DIVE-P0090"}:
            return {"outcome": "disagree", "reason": "native internal/unknown parser failure"}
        if native["code"] in {"DIVE-P0040", "DIVE-P0090"}:
            return {"outcome": "native-refusal", "reason": native["error"]["message"], "native_code": native["code"]}
    if row["source-grammar-secondary"]["status"] not in ("accepted", "rejected"):
        return {"outcome": "oracle-error", "reason": row["source-grammar-secondary"]["status"]}
    accepted = row["source-grammar-secondary"]["status"] == "accepted"
    if accepted != ("ast" in native):
        return {"outcome": "disagree", "reason": "source grammar disposition differs"}
    if not accepted:
        if row["primary"]["status"] == "accepted":
            return {"outcome": "disagree", "reason": "primary accepted input rejected natively"}
        return {"outcome": "grammar-rejection-match", "native_code": native["code"]}
    right = native_project(native["ast"])
    differences = [key for key in left if left[key] != right[key]]
    result = {}
    if differences:
        # GetMacro performs expansion before the secondary adapter can stop it.
        # These rows remain uncertified/pending, never counted as AST matches.
        if differences == ["macros"] and [m[:2] for m in left["macros"]] == [m[:2] for m in right["macros"]] and any("MacroCall" in json.dumps(m[2]) for m in right["macros"]):
            result.update(outcome="reference-stage-pending", reason="secondary GetMacro expands earlier macro calls", fields_pending=["macros.body"], other_fields_match=True)
        else:
            result.update(outcome="disagree", reason="source AST projection differs", fields=differences)
    else:
        result.update(outcome="source-ast-match", primary_accepted=row["primary"]["status"] == "accepted")
        if row["primary"]["status"] != "accepted":
            result["normalization_pending"] = row["primary"]["status"]
    return result


root = Path(__file__).parent
native = json.load(sys.stdin)
counts = Counter()
controls = {}
for bundle in ("curated", "edges", "imported"):
    manifest = json.loads((root / bundle / "manifest.json").read_text())
    for row in manifest["rows"]:
        source = (root / bundle / row["input"]).read_bytes()
        left = json.loads((root / bundle / row["projection"]).read_text()) if "projection" in row else None
        document = native.pop(row["id"])
        result = outcome(row, document, left, source)
        assert result["outcome"] != "disagree", (row["id"], result)
        counts[result["outcome"]] += 1
        controls[row["id"]] = (row, document, left, source)
assert not native, "unaccounted native input"

# These controls start from full real source ASTs, never malformed short fakes.
row, genuine, left, source = controls["C-process"]
mutations = []
fake = copy.deepcopy(genuine); fake["translation"] = "complete"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["stage"] = "translated"; mutations.append(fake)
fake = {"stage": "source-grammar", "error": {"message": "worker bug"}, "code": "DIVE-P0050"}; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["name"]["text"] = "Wrong"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["body"]["Processes"][0]["fairness"] = "Unfair"; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["algorithm"]["body"]["Processes"][0]["body"]["statements"] = []; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["statements"].append(copy.deepcopy(fake["ast"]["statements"][0])); mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["source"] += " "; mutations.append(fake)
fake = copy.deepcopy(genuine); fake["ast"]["tokens"][0]["text"] = "wrong"; mutations.append(fake)
for fake in mutations:
    try:
        result = outcome(row, fake, left, source)
        assert result["outcome"] == "disagree", result
    except (ValueError, KeyError, TypeError):
        pass  # Integrity failure cannot turn into an agreement.

# A normalization reject must not excuse a native bug/false translation claim.
row, genuine, left, source = controls["P-labels_macro"]
fake = {"stage": "source-grammar", "error": {"message": "bug"}, "code": "DIVE-P0050"}
assert outcome(row, fake, left, source)["outcome"] == "disagree"
print(json.dumps({"summary": dict(counts), "false_agreement_mutations": len(mutations) + 1}))
