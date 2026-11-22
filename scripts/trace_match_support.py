"""Source literals and standalone Dive replay for staged trace specifications."""

import json
import os
import re
import subprocess


class Unsupported(ValueError):
    """An input or backend combination that supplies no matching verdict."""


def unique_fields(items):
    result = dict(items)
    if len(result) != len(items):
        raise Unsupported("duplicate JSON field")
    return result


def tla_literal(value):
    if type(value) is bool:
        return "TRUE" if value else "FALSE"
    if type(value) is int and 0 <= value < 2**64:
        return str(value)
    if isinstance(value, str) and value.isascii() and all(32 <= ord(c) < 127 for c in value):
        return json.dumps(value)
    if isinstance(value, list):
        return "<<" + ", ".join(tla_literal(item) for item in value) + ">>"
    if isinstance(value, dict):
        if any(not re.fullmatch(r"[a-z][a-zA-Z0-9_]*", key) for key in value):
            raise Unsupported("unsupported JSON record field name")
        if not value:
            return r"[k \in {} |-> 0]"
        return "[" + ", ".join(key + " |-> " + tla_literal(v) for key, v in value.items()) + "]"
    raise Unsupported("unsupported trace value (expected records, arrays, booleans, printable ASCII or unsigned integers)")


def run(binary, directory, command, model, options, timeout):
    # Keep ambient per-user settings or library paths out of the replay.
    env = {k: v for k, v in os.environ.items() if not k.startswith("DIVE_")}
    env["XDG_CONFIG_HOME"] = str(directory / "config")
    args = [binary, command, f"{model}.tla", "--config", f"{model}.cfg",
            "--output", "json", "--seed", "0", "--fp", "0", *options]
    process = subprocess.run(args, cwd=directory, env=env, capture_output=True, timeout=timeout)
    try:
        report = json.loads(process.stdout)
    except ValueError as exc:
        raise Unsupported(f"{command} returned no JSON report (exit {process.returncode})") from exc
    if not isinstance(report, dict):
        raise Unsupported(f"unexpected {command} report")
    return process.returncode, report


def replay(binary, directory, model, name, word, timeout):
    (directory / name).write_text(json.dumps(word, separators=(",", ":")) + "\n")
    code, report = run(binary, directory, "validate-trace", model, ["--trace", name], timeout)
    if code != 0 or report.get("result") != "valid":
        raise Unsupported(f"independent replay on {model} returned {report.get('result')} ({report.get('message') or report.get('failure')})")
