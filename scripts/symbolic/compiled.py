"""Compile and run one finite exact safety check under the companion supervisor."""

import argparse
import gc
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

from dd import cudd

from engine import SourceModel, Unsupported, require
from induction import write_atomic
from packed_codegen import Rust
from worker_process import Stopped, limit_memory, run_helper

TOOLCHAIN = "1.99.0"


def compiler(max_memory):
    rustc = shutil.which("rustc")
    rustup = shutil.which("rustup")
    try:
        if rustup and (not rustc or os.path.samefile(rustc, rustup)):
            # Resolve an installed binary without asking the proxy to install a toolchain.
            installed = subprocess.run([rustup, "which", "--toolchain", TOOLCHAIN, "rustc"],
                                       text=True, capture_output=True, timeout=5, check=False)
            rustc = installed.stdout.strip() if installed.returncode == 0 else None
        require(rustc, f"compiled checking requires installed rustc {TOOLCHAIN}")
        result = run_helper([rustc, "--version"], "Rust compiler startup", max_memory, timeout=5)
    except subprocess.TimeoutExpired as error:
        if max_memory:
            raise Stopped("Rust compiler startup exceeded its resource envelope") from error
        raise RuntimeError("Rust compiler startup did not complete") from error
    require(result.returncode == 0 and result.stdout.split()[:2] == ["rustc", TOOLCHAIN],
            f"compiled checking requires installed rustc {TOOLCHAIN}")
    return rustc


def replay(args, candidate_depths):
    options = [args.module, "--config", args.config, "--compat=tlc", "--seed", "0", "--fp", "0",
               "--output", "json"]
    checked = run_helper([args.dive, "check", *options, "--workers", "1", "--store", "exact",
                          "--exit-codes", "native", "--time-limit", "none", "--max-memory", "none"],
                         "Core witness search", args.max_memory)
    row = json.loads(checked.stdout)
    verdict = row["verdict"]
    if verdict in ("stopped", "resource-limit"):
        raise Stopped(row["message"] or verdict)
    depth = candidate_depths.get(verdict, 0)
    trace = (row.get("counterexample") or {}).get("trace")
    if not checked.returncode or not depth or not trace or len(trace["states"]) != depth:
        raise RuntimeError(f"Core did not reproduce the compiled finding at its layer ({verdict})")
    raw = json.dumps(trace) + "\n"
    validated = run_helper([args.dive, "validate-trace", *options, "--trace", "/dev/stdin"],
                           "independent trace replay", args.max_memory, input=raw)
    if validated.returncode or json.loads(validated.stdout).get("result") != "valid":
        raise RuntimeError("independent trace replay did not validate the compiled finding")
    report = dict(verdict=verdict, depth=depth, ending=trace["ending"], trace_validated=True)
    if args.trace:
        write_atomic(Path(args.trace), raw.encode("utf-8"))
        report["trace_file"] = args.trace
    else:
        report["trace"] = trace
    return report


def emit_worker(document, type_invariant, template, manager):
    model = SourceModel(document, type_invariant, manager, state_bit_limit=63)
    emitter = Rust(model)
    text = emitter.emit(template)
    return text, emitter.metadata


def lower(document, type_invariant, template):
    # Dispose of cyclic decision nodes while the manager is still alive.
    manager = cudd.BDD(memory_estimate=512 << 20)
    try:
        return emit_worker(document, type_invariant, template, manager)
    finally:
        gc.collect()


def run(args):
    limit_memory(args.max_memory)
    rustc = compiler(args.max_memory)
    exported = run_helper([args.exporter, args.module, args.config], "Dive frontend", args.max_memory)
    require(exported.returncode == 0, "Dive frontend: " + exported.stderr.strip())
    template = Path(__file__).with_name("packed_worker.rs.in").read_text()
    text, layout = lower(json.loads(exported.stdout), args.type_invariant, template)
    folder = Path(args.scratch)
    source = folder / "worker.rs"
    binary = folder / "worker"
    source.write_text(text)
    # The supervisor owns this directory, including temporary linker files, across cancellation.
    started = time.monotonic()
    built = run_helper([rustc, "--edition=2024", "-Copt-level=3", "-Cpanic=abort", "-Clto=off",
                        "-Ccodegen-units=1", source, "-o", binary],
                       "Rust compiler", args.max_memory, cwd=folder,
                       env={**os.environ, "TMPDIR": str(folder)})
    compile_seconds = time.monotonic() - started
    if built.returncode:
        raise RuntimeError("could not compile packed checker: " + built.stderr.strip()[-1500:])
    started = time.monotonic()
    searched = run_helper([binary, str(args.workers)], "compiled checker", args.max_memory)
    search_seconds = time.monotonic() - started
    if searched.returncode == 4:
        raise Stopped("compiled state bitset exceeds available memory")
    if searched.returncode == 6:
        raise RuntimeError(searched.stderr.strip().removeprefix("Error: "))
    if searched.returncode not in (0, 5):
        raise RuntimeError(f"unexpected compiled checker status {searched.returncode}")
    result = json.loads(searched.stdout)
    if searched.returncode == 5:
        row = replay(args, {"invariant-violation": result["invariant_depth"],
                            "deadlock": result["deadlock_depth"]})
    else:
        row = dict(verdict="success", left_on_queue=0, **result)
    row.update(workers_requested=args.workers, state_bits=layout["width"],
               membership_bits=layout["membership_bits"],
               compile_seconds=round(compile_seconds, 6), search_seconds=round(search_seconds, 6))
    return row, 11 if searched.returncode == 5 else 0


def main():
    args = argparse.Namespace(**json.loads(sys.argv[1]))
    start = time.monotonic()
    try:
        row, status = run(args)
    except Unsupported as error:
        row, status = dict(verdict="unsupported", reason=str(error)), 2
    except (Stopped, MemoryError) as error:
        row, status = dict(verdict="stopped", reason=str(error) or "memory exhausted"), 4
    except Exception as error:
        row, status = dict(verdict="error", reason=f"{type(error).__name__}: {error}"), 3
    row.update(format="dive-symbolic-report-v1", strategy="compiled", module=args.module,
               config=args.config, seconds=round(time.monotonic() - start, 6))
    print(json.dumps(row), flush=True)
    return status


if __name__ == "__main__":
    sys.exit(main())
