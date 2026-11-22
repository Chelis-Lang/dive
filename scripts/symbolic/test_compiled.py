#!/usr/bin/env python3
"""Run the packaged compiled CLI through success, refusal, replay and cancellation."""

import json
import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
from tempfile import TemporaryDirectory
import time

HERE = Path(__file__).resolve().parent


def check(command, status=0, **kwargs):
    result = subprocess.run(command, text=True, capture_output=True, check=False, timeout=30, **kwargs)
    assert result.returncode == status, (result.returncode, result.stdout, result.stderr)
    if "--strategy=compiled" in command:
        assert not result.stderr, result.stderr
    return json.loads(result.stdout)


def cancel(command, env, marker):
    with subprocess.Popen(command, env=env, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
        try:
            deadline = time.monotonic() + 15
            while not marker.exists() and child.poll() is None and time.monotonic() < deadline:
                time.sleep(0.01)
            assert marker.exists(), "compiled phase did not start"
            child.send_signal(signal.SIGTERM)
            out, err = child.communicate(timeout=10)
            assert child.returncode == 4 and json.loads(out)["reason"] == "interrupted", (out, err)
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()
    assert not list(Path(env["TMPDIR"]).glob("dive-compiled-*"))


def installed(root, binary, exporter):
    pack = root / "libexec" / "dive-symbolic"
    pack.mkdir(parents=True)
    dive = root / "dive"
    shutil.copy2(binary, dive)
    (pack / "python").symlink_to(Path(sys.prefix).resolve(), target_is_directory=True)
    (pack / "symbolic_input").symlink_to(exporter)
    names = ("check.py", "compiled.py", "engine.py", "finite_relation.py", "induction.py",
             "packed_codegen.py", "packed_worker.rs.in", "worker_process.py")
    for name in names:
        (pack / name).symlink_to(HERE / name)
    return dive


def check_wide(root, dive):
    module = root / "WideReceipts.tla"
    module.write_text(r'''---- MODULE WideReceipts ----
EXTENDS TLC
CONSTANT IDs
VARIABLE got
Tokens == {"red", "blue", "a", "b", "c", "d", "e", "f", "g", "h", "i", "j"}
TypeInvariant == got \in [IDs -> SUBSET Tokens]
OnlyRed == \A id \in IDs : got[id] \subseteq {"red"}
OnlyBlue == \A id \in IDs : got[id] \subseteq {"blue"}
OnlyColors == \A id \in IDs : got[id] \subseteq {"red", "blue"}
SomeEmpty == \E id \in IDs : got[id] = {}
Never == \E id \in IDs : got[id] = {"red"} /\ got[id] = {"blue"}
Symmetric == Permutations(IDs)
Init == got = [id \in IDs |-> {}]
Receive(token) == \E id \in IDs :
    /\ got[id] = {}
    /\ got' = [got EXCEPT ![id] = {token}]
Next == \/ Receive("red")
        \/ Receive("red")
        \/ UNCHANGED got
NextBad == \/ Next
           \/ Receive("blue")
Mixed == \/ /\ \A id \in IDs : got[id] = {}
            /\ Receive("red")
         \/ /\ \A id \in IDs : got[id] = {}
            /\ Receive("blue")
         \/ /\ ~(\A id \in IDs : got[id] /= {"red"})
            /\ Receive("a")
====
''')
    cfg = module.with_suffix(".cfg")
    base = 'CONSTANT IDs = {p1, p2, p3}\nINIT Init\nINVARIANT TypeInvariant\nSYMMETRY Symmetric\n'
    compiled = [dive, "check", module, "--strategy=compiled", "--output", "json"]
    core = [dive, "check", module, "--compat=tlc", "--store", "exact", "--seed", "0", "--fp", "0",
            "--workers", "1", "--output", "json"]
    cfg.write_text(base + 'NEXT Next\nINVARIANT OnlyRed\n')
    row = check([*compiled, "--workers", "8"])
    assert row["state_bits"] == 36 and row["membership_bits"] == 4
    ordinary = check(core)["statistics"]
    keys = ("initial_generated", "distinct", "generated", "depth", "left_on_queue")
    assert tuple(row[k] for k in keys) == tuple(ordinary[k] for k in keys) == (1, 4, 17, 4, 0)

    # Blue is excluded by the lane projection, as is the initial row if
    # the invariant is identically false. Both findings must survive ranking.
    for next_op, invariant, depth, ranks in (("NextBad", "OnlyRed", 2, 4),
                                            ("Next", "OnlyBlue", 2, 4),
                                            ("Next", "Never", 1, 0)):
        cfg.write_text(base + f'NEXT {next_op}\nINVARIANT {invariant}\n')
        row = check(compiled, 1)
        assert row["trace_validated"] and row["ending"]["name"] == invariant
        assert row["witness_strategy"] == "compiled"
        assert row["depth"] == depth and row["membership_bits"] == ranks

    # Each lane of the final row has a rank, but the combination still
    # violates the cross-lane predicate.
    cfg.write_text(base + 'NEXT Next\nINVARIANT OnlyRed\nINVARIANT SomeEmpty\n')
    row = check(compiled, 1)
    assert row["trace_validated"] and row["ending"]["name"] == "SomeEmpty" and row["depth"] == 4
    assert row["witness_strategy"] == "compiled"
    parallel = check([*compiled, "--workers", "8"], 1)
    assert row["trace"] == parallel["trace"]

    # Two different property names occur on the same first failing layer.
    cfg.write_text(base + 'NEXT NextBad\nINVARIANT OnlyRed\nINVARIANT OnlyBlue\n')
    row = check(compiled, 1)
    assert row["trace_validated"] and row["witness_strategy"] == "core" and row["depth"] == 2
    assert row["ending"] == check([*core, "--exit-codes", "native"], 10)["counterexample"]["trace"]["ending"]

    # The same expansion wave can expose a deadlock and a bad successor.
    cfg.write_text(base + 'NEXT Mixed\nINVARIANT OnlyColors\n')
    row = check(compiled, 1)
    assert row["trace_validated"] and row["witness_strategy"] == "core"
    assert (row["verdict"], row["depth"]) in (("deadlock", 2), ("invariant-violation", 3))

    cfg.write_text(base.replace('SYMMETRY Symmetric\n', '') + 'NEXT Next\nINVARIANT OnlyRed\n')
    row = check(compiled, 2)
    assert row["verdict"] == "unsupported" and "proved index symmetry" in row["reason"]


def run(root, binary, exporter):
    dive = installed(root, binary, exporter)
    check_wide(root, dive)
    module = root / "Receipts.tla"
    module.write_text(r'''---- MODULE Receipts ----
CONSTANT IDs
VARIABLE got
TypeInvariant == got \subseteq IDs
Init == got = {}
Receive == \E i \in IDs \ got : got' = got \union {i}
Next == \/ Receive
        \/ Receive
        \/ UNCHANGED got
Empty == got = {}
DeadEnd == Receive
Restricted == got = {}
====
''')
    cfg = module.with_suffix(".cfg")
    base = 'CONSTANT IDs = {red, green, blue}\nINIT Init\nINVARIANT TypeInvariant\n'
    cfg.write_text(base + 'NEXT Next\n')
    compiled = [dive, "check", module, "--strategy=compiled", "--output", "json"]
    core = [dive, "check", module, "--compat=tlc", "--store", "exact", "--seed", "0", "--fp", "0",
            "--workers", "1", "--output", "json"]
    ordinary = check(core)["statistics"]
    keys = ("initial_generated", "distinct", "generated", "depth", "left_on_queue")
    for workers in (1, 8):
        row = check([*compiled, "--workers", str(workers)])
        assert row["verdict"] == "success" and row["workers_requested"] == workers
        assert tuple(row[key] for key in keys) == tuple(ordinary[key] for key in keys) == (1, 8, 33, 4, 0)
    cfg.write_text(base + 'NEXT Next\nINVARIANT Empty\n')
    trace = root / "witness.json"
    row = check([*compiled, "--trace", trace], 1)
    assert row["trace_validated"] and row["depth"] == 2 and row["ending"]["name"] == "Empty"
    assert row["witness_strategy"] == "compiled"
    validate = [dive, "validate-trace", module, "--compat=tlc", "--trace", trace, "--output", "json"]
    assert check(validate)["result"] == "valid"
    bad = json.loads(trace.read_text())
    bad["states"].pop(0)
    trace.write_text(json.dumps(bad))
    invalid = subprocess.run(validate, text=True, capture_output=True, check=False, timeout=10)
    assert invalid.returncode and json.loads(invalid.stdout)["result"] != "valid"

    cfg.write_text(base + 'NEXT DeadEnd\n')
    row = check([*compiled, "--workers", "8"], 1)
    assert row["trace_validated"] and row["verdict"] == "deadlock" and row["depth"] == 4
    assert row["witness_strategy"] == "core"
    cfg.write_text(base + 'NEXT Next\nCONSTRAINT Restricted\n')
    row = check(compiled, 2)
    assert row["verdict"] == "unsupported" and "state constraints" in row["reason"]
    cfg.write_text(base + 'NEXT Next\n')
    row = check([*compiled, "--time-limit", "1ms"], 4)
    assert row["verdict"] == "stopped" and row["reason"] == "time limit" and "distinct" not in row
    row = check([*compiled, "--max-memory", "32m"], 4)
    assert row["verdict"] == "stopped" and "distinct" not in row

    compiler = subprocess.check_output(["rustup", "which", "--toolchain", "1.99.0", "rustc"], text=True).strip()
    work = root / "compiler-control"
    work.mkdir()
    compiler_path = work / "rustc"
    env = {**os.environ, "PATH": str(work) + ":" + os.environ["PATH"], "TMPDIR": str(work)}
    name = shlex.quote(compiler)
    for phase in ("compile", "search", "witness"):
        marker = work / phase
        wait = f'printf ready > {shlex.quote(str(marker))}\nexec sleep 600\n'
        tool = f'#!/bin/sh\nif [ "$1" = --version ]; then exec {name} "$@"; fi\n'
        if phase == "compile":
            tool += wait
        else:
            dispatch = ""
            if phase == "witness":
                cfg.write_text(base + 'NEXT Next\nINVARIANT Empty\n')
                dispatch = 'if [ "$1" != witness ]; then exec "$0.native" "$@"; fi\n'
            tool += f'''{name} "$@" || exit "$?"
for output do :; done
mv "$output" "$output.native"
cat > "$output" <<'WORKER'
#!/bin/sh
{dispatch}{wait}WORKER
chmod +x "$output"
'''
        compiler_path.write_text(tool)
        compiler_path.chmod(0o755)
        cancel(compiled, env, marker)


if __name__ == "__main__":
    with TemporaryDirectory() as tmp:
        run(Path(tmp), *(Path(path).resolve() for path in sys.argv[1:]))
