#!/usr/bin/env python3
"""Supervise a finite symbolic or occupancy safety check of the bound TLA+ source."""

import argparse
import json
import math
import os
from pathlib import Path
import signal
import subprocess
import sys


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
    parser.add_argument("--type-invariant", default="TypeInvariant")
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
    certificate = parser.add_mutually_exclusive_group()
    certificate.add_argument(
        "--prove-inductive",
        action="store_true",
        help="try the conjunction of the configured invariants as an induction hypothesis",
    )
    certificate.add_argument(
        "--write-predicate", type=Path, help="export the exact closed set after a successful search"
    )
    certificate.add_argument(
        "--verify-predicate", type=Path, help="prove safety by checking a supplied predicate for induction"
    )
    parser.add_argument("--occupancy", action="store_true", help="exact labeled BFS on anonymous status tables")
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
    if args.max_orbits is not None and (not args.occupancy or args.max_orbits < 1):
        parser.error("--max-orbits requires occupancy and a positive integer")
    if args.occupancy and any((args.recheck, args.refine_domains, args.order_by_path, args.single_partition)):
        parser.error("occupancy does not accept BDD layout or --recheck options")
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

    # A supervisor can interrupt a long CUDD operation and stop its process group.
    # The normal Dive binary and other sessions keep their own limits.
    worker = Path(__file__).with_name("occupancy.py" if args.occupancy else "engine.py")
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
