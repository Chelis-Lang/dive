"""Choose an exhaustive representation after binding the original check request."""

import argparse
import gc
import json
from pathlib import Path
import resource
import subprocess
import sys
import time

from dd import cudd

import compiled
import engine
from finite_values import FiniteValueModel, whole_record_layout
import occupancy
from occupancy_source import PopulationSource, Unsupported as PopulationUnsupported
from packed_codegen import Rust
from worker_process import Stopped, limit_memory, run_helper


class UseCore(Exception):
    """No finite representation has admitted the complete request."""


def select(args, document, owner):
    invariants = document["invariants"]
    # Only configured invariants can supply the layout. The names carry no
    # semantics; every candidate must pass the same admission as explicit use.
    for name, _ in invariants:
        try:
            population = PopulationSource(document, name)
        except PopulationUnsupported:
            continue
        if len(population.ids) <= 8192:
            return "occupancy", occupancy.check_model(args, population)

    owner.append(cudd.BDD(memory_estimate=512 << 20))
    for name, expr in invariants:
        try:
            model = engine.bind_source_model(document, name, owner[0], False, False, None)
        except engine.Unsupported:
            if not whole_record_layout(document, expr):
                continue
            try:
                model = FiniteValueModel(document, expr)
            except engine.Unsupported:
                continue
        # Direct 22–32 bit rows amortize native compilation on the measured
        # panel. A dense visited bitmap is 2**width bits before the frontier.
        # Only the ordinary, fully checked index action admits packed lowering.
        width = len(model.variables)
        if (type(model) is engine.SourceModel and 22 <= width <= 32
                and any(len(update) > 1 for _, update in model.effects)
                and (not args.max_memory or (1 << width) <= args.max_memory * (1 << 21))):
            try:
                rustc = compiled.compiler(args.max_memory)
                emitter = Rust(model)
                program = emitter.emit(Path(__file__).with_name("packed_worker.rs.in").read_text(),
                                       witness=True)
            except engine.Unsupported:
                pass
            else:
                args.type_invariant = name
                row, _ = compiled.search(args, rustc, document, program, emitter.metadata)
                return "compiled", row
        row, trace = model.check(None)
        if trace is not None:
            engine.validate_finding(args, args.module, args.config, row, trace, True)
        return "symbolic", row
    raise UseCore("no configured invariant admits a complete finite layout")


def run(args, owner):
    limit_memory(args.max_memory)
    exported = run_helper([args.exporter, args.module, args.config], "Dive frontend", args.max_memory)
    if exported.returncode:
        raise UseCore(exported.stderr.strip())
    document = json.loads(exported.stdout)
    return select(args, document, owner)


def check_core(args, reason, original_limit):
    # The ordinary checker enforces its resident-memory limit; the enclosing
    # supervisor still owns the wall budget, process group and scratch files.
    resource.setrlimit(resource.RLIMIT_AS, original_limit)
    print(f"dive: auto selected Core ({reason})", file=sys.stderr, flush=True)
    result = subprocess.run([args.dive, "check", *args.auto_core_args], text=True, stdout=subprocess.PIPE)
    if result.returncode < 0:
        raise Stopped(f"Core terminated by signal {-result.returncode}")
    if result.stdout:
        row = json.loads(result.stdout)
        trace = (row.get("counterexample") or {}).get("trace")
        if trace is not None:
            opts = row["options"]
            replay = run_helper([args.dive, "validate-trace", args.module, "--config", args.config,
                                 "--seed", str(opts["seed"]), "--fp", "0",
                                 "--trace", "/dev/stdin", "--output", "json"],
                                "independent Core replay", args.max_memory, input=json.dumps(trace))
            if replay.returncode or json.loads(replay.stdout).get("result") != "valid":
                raise RuntimeError("independent Core trace validation failed: " + replay.stdout[-500:])
            row["counterexample"]["validation"] = "valid"
        print(json.dumps(row), flush=True)
    Path(args.scratch, "core").touch()
    return result.returncode


def main():
    args = argparse.Namespace(**json.loads(sys.argv[1]))
    started = time.monotonic()
    original_limit = resource.getrlimit(resource.RLIMIT_AS)
    owner = []
    try:
        try:
            strategy, row = run(args, owner)
        except UseCore as reason:
            return check_core(args, reason, original_limit)
        if row["verdict"] == "success":
            row["left_on_queue"] = 0
            status = 0
        elif row.get("trace_validated"):
            status = 11
        else:
            raise RuntimeError("finite search returned an uncertified finding")
        row.update(format="dive-symbolic-report-v1", strategy=strategy, module=args.module,
                   config=args.config, seconds=round(time.monotonic() - started, 6))
    except (Stopped, MemoryError, occupancy.Budget) as error:
        row, status = dict(verdict="stopped", reason=str(error) or "memory exhausted"), 4
    except (engine.Unsupported, PopulationUnsupported) as error:
        row, status = dict(verdict="unsupported", reason=str(error)), 2
    except Exception as error:
        row, status = dict(verdict="error", reason=f"{type(error).__name__}: {error}"), 3
    finally:
        gc.collect()
    print(json.dumps(row), flush=True)
    return status


if __name__ == "__main__":
    if 0 < sys.get_int_max_str_digits() < 8_000:
        sys.set_int_max_str_digits(8_000)
    sys.exit(main())
