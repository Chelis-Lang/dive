#!/usr/bin/env python3
"""Try an existing invariant witness on a larger population and validate the full result."""

import argparse
import json
from pathlib import Path
import subprocess

from induction import write_atomic


def extend(trace, member, extra):
    if not trace.get("states") or trace["ending"]["kind"] != "invariant":
        raise ValueError("expected a finite invariant witness")
    fields = {}
    for name, value in trace["states"][0]["values"].items():
        if isinstance(value, dict) and "fcn" in value:
            for key, initial in value["fcn"]:
                if key == {"model": member}:
                    fields[name] = initial
    if not fields:
        raise ValueError("existing member has no top-level function entries")
    for state in trace["states"]:
        for name, initial in fields.items():
            entries = state["values"][name]["fcn"]
            if any(key == {"model": extra} for key, _ in entries):
                raise ValueError("additional member already present")
            entries.append([{"model": extra}, initial])
    return sorted(fields)


def validate(dive, module, config, trace):
    result = subprocess.run([dive, "validate-trace", module, "--config", config, "--trace", "/dev/stdin",
                             "--output", "json"], input=json.dumps(trace), capture_output=True, text=True)
    if result.returncode != 0 or json.loads(result.stdout).get("result") != "valid":
        raise ValueError(f"witness rejected on {config}: " + (result.stdout or result.stderr).strip())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for argument in ("module", "base_config", "target_config", "trace"):
        parser.add_argument(argument, type=Path)
    parser.add_argument("--dive", default="dive")
    parser.add_argument("--existing-member", required=True)
    parser.add_argument("--new-member", required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.output and args.output.resolve() in {p.resolve() for p in
                                                  (args.module, args.base_config, args.target_config, args.trace)}:
        parser.error("output must be separate from source inputs")
    try:
        trace = json.loads(args.trace.read_text())
        validate(args.dive, args.module, args.base_config, trace)
        fields = extend(trace, args.existing_member, args.new_member)
        validate(args.dive, args.module, args.target_config, trace)
        if args.output:
            write_atomic(args.output, (json.dumps(trace) + "\n").encode())
        print(json.dumps(dict(proof="violated", property=trace["ending"]["name"], trace_validated=True,
                              states=len(trace["states"]), extended_fields=fields)))
    except (ValueError, KeyError, TypeError, OSError) as error:
        parser.exit(2, f"{error}\n")


if __name__ == "__main__":
    main()
