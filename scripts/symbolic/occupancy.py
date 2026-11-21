#!/usr/bin/env python3
"""Source-checkout prototype for exact labeled BFS over anonymous finite populations."""

import argparse
import json
from pathlib import Path
import sys
import tempfile
import time

from induction import write_atomic
from occupancy_source import PopulationSource, Unsupported
from worker_process import Stopped, limit_memory, run_helper


class Budget(Exception):
    """A resource stop carries no complete counts."""


def shift(population, source, target):
    changed = list(population)
    changed[source] -= 1
    changed[target] += 1
    return tuple(changed)


def witness(model, parents, end, ending):
    moves = []
    population = end
    while parents[population] is not None:
        source, target = parents[population]
        moves.append((source, target))
        population = shift(population, target, source)
    if (len(moves) + 1) * max(1, len(model.ids)) > 100_000:
        raise Budget("concrete witness exceeds 100000 status entries")
    root = next((i for i, count in enumerate(population) if count), 0)
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
    parents = {}
    frontier = []
    for status in model.initial:
        population = tuple(len(model.ids) if i == status else 0 for i in range(len(model.statuses)))
        if population not in parents:
            if len(parents) == limit:
                raise Budget("occupancy limit")
            parents[population] = None
            frontier.append((population, 1))

    distinct, generated, depth = 0, len(model.initial), 0
    while frontier:
        depth += 1
        following = []
        for population, weight in frontier:
            mask = sum(1 << i for i, count in enumerate(population) if count)
            if mask in model.violations:
                ending = model.violations[mask]
                verdict = "invariant-violation" if ending["kind"] == "invariant" else "deadlock"
                return {"verdict": verdict, "depth": depth, "ending": ending}, witness(model, parents, population, ending)
            distinct += weight
            degree = 0
            for source, target, multiplicity in model.tables[mask]:
                degree += multiplicity * (1 if source is None else population[source])
                if source == target:
                    continue
                successor = shift(population, source, target)
                if successor not in parents:
                    if len(parents) == limit:
                        raise Budget("occupancy limit")
                    parents[successor] = (source, target)
                    # The multinomial orbit size changes by c_source / (c_target + 1).
                    next_weight = weight * population[source] // successor[target]
                    following.append((successor, next_weight))
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
    if args.trace and Path(args.trace).resolve() in {Path(f["display"]).resolve() for f in doc["ir"]["files"]}:
        raise Unsupported("concrete trace path aliases a source module")
    model = PopulationSource(doc, args.type_invariant)
    row, trace = search(model, args.max_orbits or 1_000_000)
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
    started = time.monotonic()
    try:
        row = run(args)
        code = 0 if row["verdict"] == "success" else 1
    except Unsupported as error:
        row, code = {"verdict": "unsupported", "reason": str(error)}, 2
    except (Budget, Stopped) as error:
        row, code = {"verdict": "stopped", "reason": str(error)}, 4
    except MemoryError:
        row, code = {"verdict": "stopped", "reason": "occupancy worker memory budget"}, 4
    except (OSError, RecursionError, RuntimeError, json.JSONDecodeError) as error:
        row, code = {"verdict": "error", "reason": str(error)}, 3
    row.update(format="dive-occupancy-report-v1", module=args.module, config=args.config, seconds=round(time.monotonic() - started, 6))
    print(json.dumps(row))
    return code


if __name__ == "__main__":
    status = main()
    sys.exit(11 if status == 1 else status)
