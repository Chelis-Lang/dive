#!/usr/bin/env python3
"""Supervise a finite safety check of the bound TLA+ source."""

import argparse
from contextlib import nullcontext
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys
from tempfile import TemporaryDirectory


def positive_seconds(value):
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise argparse.ArgumentTypeError("expected positive finite seconds")
    return number


def positive_mib(value):
    number = int(value)
    if not 0 < number <= sys.maxsize >> 20:
        raise argparse.ArgumentTypeError("expected a positive number of MiB")
    return number


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--type-invariant")
    parser.add_argument("--exporter", default="target/release/examples/symbolic_input")
    parser.add_argument("--dive", default="target/release/dive")
    parser.add_argument(
        "--order-by-path",
        action="store_true",
        help="ablation: allocate each complete TLA+ variable in declaration order",
    )
    parser.add_argument(
        "--single-partition",
        action="store_true",
        help="ablation: compute each complete simultaneous action image in one operation",
    )
    parser.add_argument("--refine-domains", action="store_true",
                        help="learn members of wide finite set fields during exact reachability")
    parser.add_argument(
        "--prove-inductive",
        action="store_true",
        help="try the conjunction of the configured invariants as an induction hypothesis",
    )
    parser.add_argument(
        "--write-predicate", type=Path, help="export the exact closed set after a successful search"
    )
    parser.add_argument(
        "--verify-predicate", type=Path, help="prove safety by checking a supplied predicate for induction"
    )
    strategy = parser.add_mutually_exclusive_group()
    strategy.add_argument("--property-proof", metavar="NAME", help="prove a single configured state invariant")
    strategy.add_argument("--occupancy", action="store_true", help="exact labeled BFS on anonymous status tables")
    strategy.add_argument("--compiled", action="store_true", help="compile a packed exact safety search")
    strategy.add_argument("--auto-core-args", type=json.loads, help=argparse.SUPPRESS)
    parser.add_argument("--workers", type=int, choices=range(1, 9), help="maximum compiled search workers")
    parser.add_argument("--max-orbits", type=int, help="maximum reachable population vectors for the occupancy strategy")
    parser.add_argument("--trace", type=Path)
    parser.add_argument(
        "--recheck",
        nargs=2,
        metavar=("MODULE", "CONFIG"),
        action="append",
        default=[],
        help="check the edited input using proved reachability layers or surviving refined set coordinates",
    )
    parser.add_argument("--time-limit", type=positive_seconds, metavar="SECONDS",
                        help="wall-time budget for the whole invocation, including binding and validation")
    parser.add_argument("--max-memory", type=positive_mib, metavar="MIB",
                        help="virtual address-space limit inherited by the worker and each child")
    args = parser.parse_args()
    if args.type_invariant is None and not args.property_proof:
        args.type_invariant = "TypeInvariant"
    auto = args.auto_core_args is not None
    if auto and args.max_memory is None and not any(
        a == "--max-memory" or a.startswith("--max-memory=") for a in args.auto_core_args
    ):
        args.max_memory = max(1, os.sysconf("SC_PHYS_PAGES") * os.sysconf("SC_PAGE_SIZE") >> 21)
    if auto and any((args.trace, args.recheck, args.refine_domains, args.prove_inductive,
                     args.write_predicate, args.verify_predicate, args.max_orbits,
                     args.order_by_path, args.single_partition)):
        parser.error("auto selects an exhaustive check of a single original configuration")
    if args.prove_inductive and (args.write_predicate or args.verify_predicate):
        parser.error("direct induction and portable predicate checking require separate runs")
    if args.write_predicate and args.verify_predicate and not args.property_proof:
        parser.error("checking and repairing a portable predicate requires --property-proof")
    if args.property_proof == "":
        parser.error("--property-proof requires a nonempty invariant name")
    if args.property_proof and any((args.refine_domains, args.prove_inductive,
                                    args.order_by_path, args.single_partition)):
        parser.error("property proof chooses its own finite closure and induction checks")
    if args.property_proof and args.recheck and (args.write_predicate or args.verify_predicate):
        parser.error("a portable property predicate belongs to a single revision")
    if args.max_orbits is not None and (not args.occupancy or args.max_orbits < 1):
        parser.error("--max-orbits requires occupancy and a positive integer")
    if args.occupancy and any((args.recheck, args.refine_domains, args.order_by_path, args.single_partition)):
        parser.error("occupancy does not accept BDD layout or --recheck options")
    if args.workers is not None and not (args.compiled or auto):
        parser.error("--workers requires compiled checking")
    if args.compiled and any((args.recheck, args.refine_domains, args.order_by_path, args.single_partition,
                              args.prove_inductive, args.write_predicate, args.verify_predicate)):
        parser.error("compiled checking accepts one fresh finite safety check")
    if sys.platform != "linux":
        parser.error("the symbolic checker currently supports Linux")
    if args.trace or args.write_predicate:
        try:
            outputs = [path.resolve() for path in (args.trace, args.write_predicate) if path]
            sources = {Path(path).resolve() for pair in [(args.module, args.config), *args.recheck]
                       for path in pair}
            if args.verify_predicate:
                sources.add(args.verify_predicate.resolve())
        except (OSError, RuntimeError) as error:
            parser.error(f"cannot resolve symbolic input or output path: {error}")
        if len(outputs) != len(set(outputs)) or any(path in sources for path in outputs):
            parser.error("trace, predicate output and checked inputs need separate paths")

    if args.compiled and sys.maxsize < 2**32:
        parser.error("compiled checking requires 64-bit Linux")
    directory = TemporaryDirectory(prefix="dive-compiled-") if args.compiled or auto else nullcontext(None)
    with directory as args.scratch:
        return supervise(args)


def supervise(args):
    # Stop the whole process group including native CUDD, compiler or search children.
    script = "compiled.py" if args.compiled else "occupancy.py" if args.occupancy else "engine.py"
    if args.auto_core_args is not None:
        script = "auto.py"
    worker = Path(__file__).with_name(script)
    args.workers = args.workers or 1
    class Cancelled(Exception):
        """Cancel without the extra grace in Popen.wait for KeyboardInterrupt."""

    def interrupted(signum, _frame):
        raise Cancelled(signum)

    watched = {signal.SIGINT, signal.SIGTERM, signal.SIGHUP}
    # Defer cancellation until the supervisor owns the new process group.
    old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, watched)
    for signum in watched:
        signal.signal(signum, interrupted)
    python_flags = ["-Es"] if sys.flags.ignore_environment else []
    process = subprocess.Popen(
        [sys.executable, *python_flags, str(worker), json.dumps(vars(args), default=str)],
        start_new_session=True,
    )
    try:
        signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        status = process.wait(timeout=args.time_limit)
    except (subprocess.TimeoutExpired, Cancelled) as error:
        signal.pthread_sigmask(signal.SIG_BLOCK, watched)
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait()
        reason = "time limit" if isinstance(error, subprocess.TimeoutExpired) else "interrupted"
    else:
        if args.auto_core_args is not None and Path(args.scratch, "core").exists() and status >= 0:
            return status
        if status == 11:
            return 1
        if status == 1:
            # Only the worker's private status 11 certifies a validated witness.
            if args.max_memory:
                print(json.dumps(dict(verdict="stopped", reason="native worker exited under its memory budget")),
                      flush=True)
                return 4
            print(json.dumps(dict(verdict="error", reason="native worker exited before returning a result")),
                  flush=True)
            return 3
        if status >= 0:
            return status
        reason = "worker terminated by " + signal.Signals(-status).name
    print(json.dumps(dict(verdict="stopped", reason=reason)), flush=True)
    return 4


if __name__ == "__main__":
    sys.exit(main())
