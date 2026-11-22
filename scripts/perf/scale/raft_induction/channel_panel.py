#!/usr/bin/env python3
"""Reproduce the slot controls, portable arbitrary-channel proofs and source counterexamples on Linux."""

import argparse
from copy import deepcopy
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from check import TARGET, config, digest, graph_path
from panel import altered, path, third_message_path
from values import require

HERE = Path(__file__).resolve().parent


def process_group_rss(pid):
    """Sample the parent and descendants, including workers created by threads."""
    todo = [pid]
    seen = set()
    total = 0
    while todo:
        current = todo.pop()
        if current in seen:
            continue
        seen.add(current)
        try:
            directory = Path("/proc") / str(current)
            total += (
                int((directory / "statm").read_text().split()[1])
                * os.sysconf("SC_PAGE_SIZE")
                // 1024
            )
            for task in (directory / "task").iterdir():
                try:
                    todo.extend(map(int, (task / "children").read_text().split()))
                except FileNotFoundError:
                    pass
        except (FileNotFoundError, ProcessLookupError):
            pass
    return total


def measured(argv, expected=0, timeout=300):
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        started = time.monotonic()
        with subprocess.Popen(
            argv, stdout=stdout, stderr=stderr, start_new_session=True
        ) as proc:
            peak = 0
            try:
                while proc.poll() is None:
                    peak = max(peak, process_group_rss(proc.pid))
                    if time.monotonic() - started >= timeout:
                        raise TimeoutError("panel process exceeded its wall limit")
                    time.sleep(0.04)
            finally:
                if proc.poll() is None:
                    os.killpg(proc.pid, signal.SIGKILL)
            elapsed = round(time.monotonic() - started, 4)
            stdout.seek(0)
            stderr.seek(0)
            out = stdout.read()
            err = stderr.read()
            require(
                proc.returncode == expected and not err,
                (proc.returncode, out[-500:], err[-500:]),
            )
            return json.loads(out), dict(
                wall_s=elapsed, sampled_process_tree_rss_kib=peak
            )


def copy_inputs(source, where):
    where.mkdir()
    for item in [
        source / "AsyncRaft.tla",
        source / "AsyncRaft.cfg",
        graph_path(source),
    ]:
        shutil.copy(item, where / item.name)


def run(args, root):
    reference = (
        HERE.parents[3]
        / "conformance/reports/perf/2026-11-22-asyncraft-log-induction.json"
    )
    earlier = json.loads(reference.read_bytes())
    for capacity in (2, 3):
        (original,) = [
            r
            for r in earlier["rows"]
            if r["servers"] == 3 and r["max_messages"] == capacity
        ]
        cert = root / f"slot-{capacity}.json"
        result, usage = measured(
            [
                sys.executable,
                HERE / "check.py",
                args.source,
                "--exporter",
                args.exporter,
                "--servers",
                "3",
                "--max-messages",
                str(capacity),
                "--save",
                cert,
            ]
        )
        require(
            result["result"] == "proved"
            and digest(cert.read_bytes()) == original["certificate_sha256"],
            "literal slot control differs from published receipt",
        )
        print(
            json.dumps(
                dict(
                    kind="literal_slots",
                    max_messages=capacity,
                    usage=usage,
                    sha256=digest(cert.read_bytes()),
                )
            ),
            flush=True,
        )

    def command(source, cfg, mode, cert, jobs):
        return [
            sys.executable,
            HERE / "check.py",
            source,
            "--exporter",
            args.exporter,
            "--config",
            cfg,
            "--message-sets",
            "--jobs",
            str(jobs),
            mode,
            cert,
        ]

    for bound in (2, 3):
        source = root / f"source-{bound}"
        copy_inputs(args.source, source)
        cfg = source / "RaftLog.cfg"
        if bound == 3:
            upstream = (source / "AsyncRaft.cfg").read_text()
            original = "INVARIANT H_CandidatesOrLeadersVotedForSelf"
            require(upstream.count(original) == 1, "upstream property changed")
            cfg.write_text(
                upstream.replace(original, "INVARIANT " + TARGET)
                + "\nCHECK_DEADLOCK FALSE\n"
            )
        else:
            cfg.write_text(config(source / "AsyncRaft.cfg", 3, 1000, bound, bound))
        cert = root / f"proof-{bound}.json"
        initial, usage = measured(command(source, cfg, "--save", cert, args.jobs))
        proof = json.loads(cert.read_bytes())
        require(
            proof == {k: v for k, v in initial.items() if k != "solver_metrics"}
            and proof["result"] == "proved",
            "incomplete proof",
        )
        received = root / f"received-{bound}"
        shutil.copytree(source, received)
        recfg = received / cfg.name
        verifier, receiving = measured(
            command(received, recfg, "--verify", cert, args.jobs)
        )
        require(
            proof == {k: v for k, v in verifier.items() if k != "solver_metrics"},
            "relocated proof differs",
        )
        record = dict(
            kind="membership",
            jobs=args.jobs,
            max_term=bound,
            max_log_length=bound,
            max_messages=1000,
            receipt=proof,
            checking=usage,
            receiving=receiving,
            solver_metrics=initial["solver_metrics"],
            bytes=cert.stat().st_size,
            receipt_sha256=digest(cert.read_bytes()),
        )
        if bound == 3:
            single, single_use = measured(command(received, recfg, "--verify", cert, 1))
            require(
                proof == {k: v for k, v in single.items() if k != "solver_metrics"},
                "single-worker verifier differs",
            )
            record["single_worker_verification"] = single_use
            record["original_word"] = path(
                args.dive, source / "AsyncRaft.tla", cfg, source
            )
            record["third_message_word"] = third_message_path(
                args.dive, source / "AsyncRaft.tla", cfg, source
            )
            controls = {}
            for name, gated in [
                ("ordinary_follower", False),
                ("three_pending_follower", True),
            ]:
                work = root / name
                work.mkdir()
                mutant, word = altered(
                    args.dive, source, cfg, work, three_pending=gated
                )
                # The input pin changes while the receipt stays byte-for-byte identical.
                refused, _ = measured(
                    command(mutant, cfg, "--verify", cert, args.jobs), 2
                )
                require(
                    "different source" in refused["reason"], "stale source accepted"
                )
                failed, failed_use = measured(
                    command(mutant, cfg, "--save", work / "bad.json", args.jobs), 2
                )
                require(
                    "ClientRequestAction" in failed["reason"]
                    and "raw destination safety" in failed["reason"],
                    "wrong failed mutant obligation",
                )
                controls[name] = dict(
                    word=word, induction=failed["reason"], usage=failed_use
                )
            wrong = root / "wrong.json"
            damaged = deepcopy(proof)
            damaged["encoding"] = "truncated-message-key"
            wrong.write_text(json.dumps(damaged))
            refused, _ = measured(
                command(received, recfg, "--verify", wrong, args.jobs), 2
            )
            require(
                "differs from independent action" in refused["reason"],
                "corrupt encoding accepted",
            )
            bad_cfg = root / "altered.cfg"
            bad_cfg.write_bytes(recfg.read_bytes() + b"\n")
            refused, _ = measured(
                command(received, bad_cfg, "--verify", cert, args.jobs), 2
            )
            require("different source" in refused["reason"], "changed CFG accepted")
            record["controls"] = dict(
                mutants=controls,
                changed_cfg_rejected=True,
                corrupt_encoding_rejected=True,
            )
        if args.compare_core:
            record["ordinary"] = []
            for workers in (1, args.jobs) if args.jobs != 1 else (1,):
                argv = [
                    args.dive,
                    "check",
                    source / "AsyncRaft.tla",
                    "--config",
                    cfg,
                    "--workers",
                    str(workers),
                    "--store",
                    "fp",
                    "--seed",
                    "0",
                    "--fp",
                    "0",
                    "--time-limit",
                    f"{args.compare_core}s",
                    "--max-memory",
                    "2147483648",
                    "--output",
                    "json",
                ]
                ordinary, resources = measured(argv, timeout=args.compare_core + 30)
                require(
                    ordinary["verdict"] in ("success", "stopped"),
                    "unexpected ordinary result",
                )
                record["ordinary"].append(
                    dict(
                        workers=workers,
                        seconds_budget=args.compare_core,
                        verdict=ordinary["verdict"],
                        statistics=ordinary["statistics"],
                        usage=resources,
                    )
                )
        print(json.dumps(record), flush=True)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source", type=Path)
    p.add_argument("--exporter", required=True, type=Path)
    p.add_argument("--dive", required=True, type=Path)
    p.add_argument("--jobs", type=int, default=6, choices=range(1, 12))
    p.add_argument("--compare-core", type=int, default=0, metavar="SECONDS")
    args = p.parse_args()
    for key in ("source", "exporter", "dive"):
        setattr(args, key, getattr(args, key).resolve())
    require(args.compare_core >= 0, "negative Core budget")
    with tempfile.TemporaryDirectory() as tmp:
        run(args, Path(tmp))


if __name__ == "__main__":
    main()
