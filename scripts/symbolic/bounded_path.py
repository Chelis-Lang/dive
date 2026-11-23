#!/usr/bin/env python3
"""Exact finite path layers and k-induction over one source-bound relation."""

import argparse
import gc
import hashlib
import json
from pathlib import Path
import sys
import time

from dd import cudd
from engine import SourceModel, Unsupported, require, validate_finding
from induction import write_atomic
from worker_process import Stopped, limit_memory, run_helper


def digest(data):
    return hashlib.sha256(data).hexdigest()


class PathModel(SourceModel):
    """Track action-layout escapes on their exact source guards."""

    def layout_escape(self, path, outside, guard, reason):
        if guard is None:
            super().layout_escape(path, outside, guard, reason)
        else:
            self.invalid_sources |= guard & self.any(outside.values())


def image(model, layer):
    admitted = layer & model.constraint
    require(admitted & model.invalid_sources == model.b.false,
            "symbolic path source emits outside the finite layout")
    return model.any(model.image(admitted, guard, parts) for guard, parts in model.actions)


def induction_step(model, length):
    safe = model.conjunction(term for _, term in model.properties)
    legal = model.conjunction(~model.encoding(bits, index)
                              for bits, choices in model.layout.values() if choices
                              for index, key in enumerate(choices) if model.padding(key))
    frontier = legal & safe
    for depth in range(1, length + 1):
        frontier = image(model, frontier)
        if depth < length:
            frontier &= safe
    bad = frontier & ~safe
    result = dict(bound=length, base="unsat", step="unsat")
    if bad != model.b.false:
        result["step"] = "sat"
        result["final_assignment"] = model.decode(model.b.pick(bad, care_vars=set(model.variables)))
    return result


def solve(model, bounds, induction):
    layers = [model.initial]
    completed = []
    timings = []
    for depth in range(max(max(bounds), induction or 0) + 1):
        started = time.monotonic()
        if depth:
            # Keeping the full exact-depth image preserves explicit stuttering
            # and revisits. Only admitted source states can take another step.
            layers.append(image(model, layers[-1]))
        layer = layers[-1]
        for name, safe in model.properties:
            bad = layer & ~safe
            if bad != model.b.false:
                row, trace = model.failure(layers, bad, dict(kind="invariant", name=name), reused=False)
                row.update(path_depth=depth, completed_bounds=completed, model_proved=False)
                return row, trace, timings
        if depth in bounds:
            admitted = model.state_count(layer & model.constraint)
            completed.append(dict(path_bound=depth, admitted_states=admitted,
                                  bad_prefix="unsat", executions="sat" if admitted else "unsat"))
        timings.append(round(time.monotonic() - started, 9))
    verdict = "bounded-safe" if completed[-1]["admitted_states"] else "executions-too-short"
    row = dict(verdict=verdict, completed_bounds=completed, model_proved=False)
    if induction:
        step = induction_step(model, induction)
        row.update(induction=step, model_proved=step["step"] == "unsat")
        row["verdict"] = "inductive-safety" if row["model_proved"] else "induction-inconclusive"
    return row, None, timings


def check(args, raw, manager, source_digests, exporter_digest):
    doc = json.loads(raw)
    require(doc["symmetry"] is None, "symbolic paths with SYMMETRY")
    require(not doc["explicit_deadlock"], "symbolic paths with explicit CHECK_DEADLOCK TRUE")
    require(doc["invariants"], "symbolic paths require a configured invariant")
    bounds = sorted(set(args.path_bound))
    binding = dict(format="dive-symbolic-path-receipt-v1", input_sha256=digest(raw.encode()),
                   exporter_sha256=exporter_digest, source_sha256=source_digests,
                   type_invariant=args.type_invariant,
                   bounds=bounds, k_induction=args.k_induction)
    expect = None
    if args.verify_path_receipt:
        with Path(args.verify_path_receipt).open() as stream:
            expect = json.load(stream)
        require(isinstance(expect, dict) and expect.get("binding") == binding,
                "path receipt does not match the bound query")
    started = time.monotonic()
    model = PathModel(doc, args.type_invariant, manager)
    # Build relation partitions once for all depth images and the induction step.
    actions = len(model.actions)
    construction = time.monotonic() - started
    started = time.monotonic()
    row, trace, timings = solve(model, set(bounds), args.k_induction)
    checking = time.monotonic() - started
    receipt = dict(binding=binding, result=row.copy())
    if expect is not None:
        require(receipt == expect and trace is None, "path receipt differs from freshly checked obligations")
        row["receipt_rechecked"] = True
    if trace:
        validate_finding(args, args.module, args.config, row, trace, True)
    if args.write_path_receipt and trace is None:
        write_atomic(Path(args.write_path_receipt), (json.dumps(receipt, sort_keys=True) + "\n").encode())
    row.update(strategy="symbolic-path", query=binding, state_bits=len(model.variables),
               relation_actions=actions, active_properties=[name for name, _ in model.properties],
               relation_build_seconds=construction, depth_seconds=timings, query_seconds=checking)
    print(json.dumps(row), flush=True)
    return 11 if trace else 0


def main(args):
    limit_memory(args.max_memory)
    sources = [Path(args.module), Path(args.config)]
    source_digests = [digest(path.read_bytes()) for path in sources]
    exported = run_helper([args.exporter, args.module, args.config], "Dive frontend", args.max_memory)
    require(exported.returncode == 0, "Dive frontend: " + exported.stderr.strip())
    require([digest(path.read_bytes()) for path in sources] == source_digests,
            "source or CFG changed during binding")
    manager = cudd.BDD(memory_estimate=512 << 20)
    try:
        status = check(args, exported.stdout, manager, source_digests, digest(Path(args.exporter).read_bytes()))
    except Unsupported as error:
        print(json.dumps(dict(verdict="unsupported", reason=str(error))), flush=True)
        status = 2
    finally:
        gc.collect()
    return status


if __name__ == "__main__":
    try:
        sys.exit(main(argparse.Namespace(**json.loads(sys.argv[1]))))
    except (MemoryError, Stopped) as error:
        print(json.dumps(dict(verdict="stopped", reason=str(error) or "memory exhausted")), flush=True)
        sys.exit(4)
    except (Unsupported, OSError, ValueError, KeyError, IndexError, TypeError, RecursionError) as error:
        print(json.dumps(dict(verdict="unsupported", reason=str(error))), flush=True)
        sys.exit(2)
