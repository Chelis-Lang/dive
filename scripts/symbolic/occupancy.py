#!/usr/bin/env python3
"""Exact labeled BFS over reachable occupancies of an admitted TLA+ status table."""

import argparse
import json
from pathlib import Path
import sys
import tempfile
import time

from induction import write_atomic
import occupancy_predicate
from occupancy_source import PopulationSource, Unsupported
from worker_process import Stopped, limit_memory, run_helper


class Budget(Exception):
    """A resource stop carries no complete counts."""


def shift(population, source, target):
    changed = list(population)
    changed[source] -= 1
    changed[target] += 1
    return tuple(changed)


def witness(model, parents, roots, strides, end, ending):
    moves = []
    key = end
    while parents[key] is not None:
        source, target = parents[key]
        moves.append((source, target))
        key += strides[source] - strides[target]
    if (len(moves) + 1) * max(1, len(model.ids)) > 100_000:
        raise Budget("concrete witness exceeds 100000 status entries")
    root = roots[key]
    concrete = [root] * len(model.ids)

    def state():
        entries = [[{"model": ident}, model.statuses[value]] for ident, value in zip(model.ids, concrete)]
        return {"values": {model.name: {"fcn": entries}}}

    states = [state()]
    for source, target in reversed(moves):
        concrete[concrete.index(source)] = target
        states.append(state())
    return {"format": "dive-trace", "version": 1, "variables": [model.name], "states": states, "ending": ending}


def search(model, limit):
    # Each count fits in one digit. The last status follows from the fixed total.
    width = max(1, len(model.ids).bit_length())
    strides = [1 << (width * i) for i in range(len(model.statuses) - 1)] + [0]
    moves = [[(source, target) for target in range(len(strides))] for source in range(len(strides))]
    parents, roots = {}, {}
    frontier = []
    for status in model.initial:
        population = tuple(len(model.ids) if i == status else 0 for i in range(len(model.statuses)))
        key = len(model.ids) * strides[status]
        if key not in parents:
            if len(parents) == limit:
                raise Budget("occupancy limit")
            parents[key] = None
            roots[key] = status
            frontier.append((key, population, 1))

    distinct, generated, depth = 0, len(model.initial), 0
    while frontier:
        depth += 1
        following = []
        for key, population, weight in frontier:
            mask = sum(1 << i for i, count in enumerate(population) if count)
            if mask in model.violations:
                ending = model.violations[mask]
                verdict = "invariant-violation" if ending["kind"] == "invariant" else "deadlock"
                return {"verdict": verdict, "depth": depth, "ending": ending}, witness(model, parents, roots, strides, key, ending)
            distinct += weight
            degree = 0
            for source, target, multiplicity in model.tables[mask]:
                degree += multiplicity * (1 if source is None else population[source])
                if source == target:
                    continue
                next_key = key + strides[target] - strides[source]
                if next_key not in parents:
                    if len(parents) == limit:
                        raise Budget("occupancy limit")
                    parents[next_key] = moves[source][target]
                    successor = shift(population, source, target)
                    # The multinomial orbit size changes by c_source / (c_target + 1).
                    next_weight = weight * population[source] // successor[target]
                    following.append((next_key, successor, next_weight))
            generated += weight * degree
        frontier = following

    return {"verdict": "success", "distinct": distinct, "generated": generated,
            "distinct_decimal": str(distinct), "generated_decimal": str(generated),
            "depth": depth, "occupancies": len(parents), "invariants": model.invariants}, None


def run(args):
    limit_memory(args.max_memory)
    process = run_helper([args.exporter, args.module, args.config], "Dive frontend", args.max_memory)
    if process.returncode:
        raise Unsupported("bound exporter: " + process.stderr.strip())
    doc = json.loads(process.stdout)
    sources = {Path(f["display"]).resolve() for f in doc["ir"]["files"]}
    for output in (args.trace, args.write_predicate):
        if output and Path(output).resolve() in sources:
            raise Unsupported("output path aliases a source module")
    model = PopulationSource(doc, args.type_invariant)
    if args.prove_inductive or args.verify_predicate:
        try:
            masks = (occupancy_predicate.load(args.verify_predicate, model) if args.verify_predicate
                     else occupancy_predicate.closure(model))
            failed = occupancy_predicate.verify(model, masks)
        except (ValueError, UnicodeError) as error:
            failed = "invalid support predicate: " + str(error)
        return {"proof": "inconclusive" if failed else "inductive-safety", "failed_obligation": failed,
                "predicate_format": occupancy_predicate.FORMAT, "invariants": model.invariants}
    if len(model.ids) > 8192:
        raise Unsupported("exact search owner count exceeds 8192")
    row, trace = search(model, args.max_orbits or 1_000_000)
    if row["verdict"] == "success" and args.write_predicate:
        try:
            occupancy_predicate.store(args.write_predicate, model, occupancy_predicate.closure(model))
        except ValueError as error:
            raise Budget(str(error)) from error
        row["predicate_format"] = occupancy_predicate.FORMAT
    if trace is not None:
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "word.json"
            raw = json.dumps(trace)
            if len(raw) > 16 << 20:
                raise Budget("concrete witness exceeds 16 MiB")
            path.write_text(raw)
            replay = run_helper([args.dive, "validate-trace", args.module, "--config", args.config,
                                     "--trace", path, "--compat=tlc", "--seed", "0", "--fp", "0",
                                     "--output", "json"], "Dive trace validator", args.max_memory)
            if replay.returncode or json.loads(replay.stdout).get("result") != "valid":
                raise RuntimeError("independent concrete trace validation failed")
            if args.trace:
                write_atomic(Path(args.trace), (raw + "\n").encode())
                row["trace_file"] = args.trace
            else:
                row["trace"] = trace
        row["trace_validated"] = True
    return row


def main():
    args = argparse.Namespace(**json.loads(sys.argv[1]))
    # The domain caps bound a complete generated count below 8,000 digits.
    # This private worker also emits exact JSON integers for the public API.
    if 0 < sys.get_int_max_str_digits() < 8_000:
        sys.set_int_max_str_digits(8_000)
    started = time.monotonic()
    try:
        row = run(args)
        if "proof" in row:
            code = 0 if row["proof"] == "inductive-safety" else 2
        else:
            code = 0 if row["verdict"] == "success" else 1
    except Unsupported as error:
        row, code = {"verdict": "unsupported", "reason": str(error)}, 2
    except (Budget, Stopped) as error:
        row, code = {"verdict": "stopped", "reason": str(error)}, 4
    except MemoryError:
        row, code = {"verdict": "stopped", "reason": "occupancy worker memory budget"}, 4
    except RecursionError:
        row, code = {"verdict": "stopped", "reason": "bound expression nesting limit"}, 4
    except (OSError, RuntimeError, json.JSONDecodeError) as error:
        row, code = {"verdict": "error", "reason": str(error)}, 3
    row.update(format="dive-occupancy-report-v1", module=args.module, config=args.config, seconds=round(time.monotonic() - started, 6))
    print(json.dumps(row))
    return code


if __name__ == "__main__":
    status = main()
    sys.exit(11 if status == 1 else status)
