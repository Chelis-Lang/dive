#!/usr/bin/env python3
"""Compare native source ASTs; never claim PlusCal translation certification."""
import argparse
import base64
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import sys
sys.path.insert(0, str(Path(__file__).parent))
from project import java_project, native_project
from capture import JAR_SHA


def sha(data):
    return hashlib.sha256(data).hexdigest()


def reference(directory, result, name):
    raw = {}
    for stream in ("stdout", "stderr"):
        data = gzip.decompress((directory / result[stream]).read_bytes())
        if sha(data) != result[stream + "_sha256"]:
            raise ValueError("oracle transcript integrity mismatch")
        raw[stream] = data
    if result["status"] not in ("accepted", "rejected"):
        return None
    found = []
    for line in raw["stdout"].decode().splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0] == name and parts[1] == result["status"]:
            found.append(base64.b64decode(parts[2]).decode())
    if len(found) != 1 or result.get("exit_code") != 0:
        raise ValueError("missing or duplicate reference disposition")
    return json.loads(found[0]) if result["status"] == "accepted" else found[0]


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


def outcome(row, native, secondary, primary, source):
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
    left, right = java_project(secondary), native_project(native["ast"])
    differences = [key for key in left if left[key] != right[key]]
    result = {"projection_sha256": sha(json.dumps(left, sort_keys=True).encode())}
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


def compare(args):
    rows = []
    for manifest_path, input_directory in args.bundle:
        manifest = json.loads(manifest_path.read_text())
        if manifest["oracle_sha256"] != JAR_SHA or manifest["adapter_sha256"] != sha(Path(__file__).with_name("PlusCalProbe.java").read_bytes()):
            raise ValueError("oracle pin/adapter mismatch")
        for row in manifest["rows"]:
            source = (input_directory / row["input"]).read_bytes()
            if sha(source) != row["input_sha256"]:
                raise ValueError("input integrity mismatch")
            secondary = reference(manifest_path.parent, row["source-grammar-secondary"], row["input"])
            primary = reference(manifest_path.parent, row["primary"], row["input"])
            process = subprocess.run([str(args.native)], input=source, capture_output=True, timeout=10)
            try:
                if process.returncode != 0:
                    raise ValueError("native process did not return JSON")
                native = json.loads(process.stdout)
                result = outcome(row, native, secondary, primary, source)
            except (ValueError, KeyError, TypeError, RecursionError) as error:
                result = {"outcome": "disagree", "reason": str(error)}
            rows.append({"id": row["id"], "bundle": manifest_path.parent.name, "input_sha256": row["input_sha256"],
                         "primary": row["primary"]["status"], "secondary": row["source-grammar-secondary"]["status"], **result})
    for selection in args.selection:
        for row in json.loads(selection.read_text())["rows"]:
            if row["status"] != "selected":
                rows.append({"id": row["id"], "outcome": "selection-error", "reason": row["reason"], "source_sha256": row["source_sha256"]})
    summary = dict(Counter(row["outcome"] for row in rows))
    document = {"schema_version": 1, "stage": "source-grammar", "translation": "unsupported: pending",
                "family_certified": False, "selection_total": len(rows), "summary": summary,
                "native_sha256": sha(args.native.read_bytes()), "comparison_sha256": sha(Path(__file__).read_bytes()),
                "projection_sha256": sha(Path(__file__).with_name("project.py").read_bytes()), "rows": rows}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n")
    print(json.dumps(summary))
    return 1 if summary.get("disagree") else 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--bundle", type=Path, nargs=2, action="append", required=True, metavar=("MANIFEST", "INPUTS"))
    parser.add_argument("--selection", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path, required=True)
    raise SystemExit(compare(parser.parse_args()))
