#!/usr/bin/env python3
"""Check finite fair progress for the released BlockingQueuePoisonApple source."""

import argparse
import hashlib
import json
import os
import re
from pathlib import Path
from tempfile import TemporaryDirectory
import subprocess
import time
from dataclasses import dataclass, fields

from poison_queue import MODULE, SOURCE_HASH


def and_(*terms):
    return f"(and {' '.join(terms)})"


def or_(*terms):
    return f"(or {' '.join(terms)})"


def neg(term):
    return f"(not {term})"


def call(op, *args):
    return "(" + " ".join([op, *map(str, args)]) + ")"


def min_(a, b):
    return call("ite", call("<=", a, b), a, b)


def update(a, b, values):
    return and_(
        *(
            call("=", getattr(b, k.name), values.get(k.name, getattr(a, k.name)))
            for k in fields(a)
        )
    )


@dataclass
class State:
    pq: str
    cq: str
    p: str
    c: str
    pa: str
    ca: str
    length: str
    poisons: str
    tail: str


def state(prefix):
    return State(*(f"{prefix}{k.name}" for k in fields(State)))


def admitted_source(path):
    """Admit the pinned model and its uniform two-slice quota variant."""
    data = path.read_bytes()
    source_hash = hashlib.sha256(data).hexdigest()
    if source_hash == SOURCE_HASH:
        return data.decode("utf-8"), 1, source_hash
    text = data.decode("utf-8")
    original = text
    for cardinality in ("Cardinality(Consumers)", "Cardinality(Producers)"):
        if original.count("2 * " + cardinality) != 2:
            raise ValueError("source is outside the checked poison-queue quota forms")
        original = original.replace("2 * " + cardinality, cardinality)
    if hashlib.sha256(original.encode()).hexdigest() != SOURCE_HASH:
        raise ValueError("source is outside the checked poison-queue quota forms")
    return text, 2, source_hash


def limits(s, producers, consumers, cap, quota_scale):
    a = and_(
        call("<=", 0, s.p, producers),
        call("<=", 0, s.c, consumers),
        call("<=", s.p, s.pq, call("*", s.p, consumers * quota_scale)),
        call("<=", s.c, s.cq, call("*", s.c, producers * quota_scale)),
        call("<=", 0, s.pa, s.p),
        call("<=", 0, s.ca, s.c),
        call("<=", 0, s.length, cap),
        call("<=", 0, s.poisons, s.length),
        call("=>", s.tail, call("<", 0, s.poisons)),
        call(
            "=>",
            and_(call("<", 0, s.length), neg(s.tail)),
            call("<", s.poisons, s.length),
        ),
        call("=>", call("=", s.length, 0), neg(s.tail)),
    )
    b = and_(
        call("=", call("-", s.cq, s.pq), s.poisons),
        call(">=", s.pa, min_(call("-", cap, s.length), s.p)),
        call(">=", s.ca, min_(s.length, s.c)),
        call("=>", and_(call("=", s.pq, 0), call("<", 0, s.length)), s.tail),
    )
    return a, b


def transitions(s, t, cap):
    actions = {"stutter": update(s, t, {})}
    actions["producer-wait"] = and_(
        call("<", 0, s.pa),
        call("=", s.length, cap),
        update(s, t, {"pa": call("-", s.pa, 1)}),
    )
    actions["consumer-wait"] = and_(
        call("<", 0, s.ca),
        call("=", s.length, 0),
        update(s, t, {"ca": call("-", s.ca, 1)}),
    )
    wake_c = min_(s.c, call("+", s.ca, 1))
    wake_p = min_(s.p, call("+", s.pa, 1))
    adding = and_(call("<", 0, s.pa), call("<", s.length, cap))
    removing = and_(call("<", 0, s.ca), call("<", 0, s.length))
    for poison, stops in [(0, 0), (1, 0), (1, 1)]:
        actions[f"put-{poison}-{stops}"] = and_(
            adding,
            update(
                s,
                t,
                {
                    "pq": call("-", s.pq, poison),
                    "p": call("-", s.p, stops),
                    "pa": call("-", s.pa, stops),
                    "ca": wake_c,
                    "length": call("+", s.length, 1),
                    "poisons": call("+", s.poisons, poison),
                    "tail": str(bool(poison)).lower(),
                },
            ),
        )
        head = call("<", 0, s.poisons) if poison else call("<", s.poisons, s.length)
        same_as_tail = s.tail if poison else neg(s.tail)
        if poison:
            enough = call(">=", s.poisons, 2)
        else:
            enough = call(">=", call("-", s.length, s.poisons), 2)
        room_for_tail = and_(
            call("=>", call("=", s.length, 1), same_as_tail),
            call("=>", and_(call(">", s.length, 1), same_as_tail), enough),
        )
        actions[f"get-{poison}-{stops}"] = and_(
            removing,
            head,
            room_for_tail,
            update(
                s,
                t,
                {
                    "cq": call("-", s.cq, poison),
                    "c": call("-", s.c, stops),
                    "ca": call("-", s.ca, stops),
                    "pa": wake_p,
                    "length": call("-", s.length, 1),
                    "poisons": call("-", s.poisons, poison),
                    "tail": and_(call(">", s.length, 1), s.tail),
                },
            ),
        )
    return actions


def proof(producers, consumers, cap, solver, quota_scale=1):
    s, t = state("s_"), state("t_")
    legal, inv = limits(s, producers, consumers, cap, quota_scale)
    after_legal, after_inv = limits(t, producers, consumers, cap, quota_scale)
    both = and_(legal, inv)
    active = or_(call("<", 0, s.ca), call("<", 0, s.pa))
    full = getattr(s, "length")
    actions = transitions(s, t, cap)
    obligations = {
        f"{name}-preserves": and_(both, step, after_legal, neg(after_inv))
        for name, step in actions.items()
    }
    obligations["awake-if-actors-active"] = and_(
        both, or_(call("<", 0, s.p), call("<", 0, s.c)), neg(active)
    )
    obligations["full-with-producer-has-awake-reader"] = and_(
        both, call("=", full, cap), call("<", 0, s.pq), call("=", s.ca, 0)
    )
    obligations["space-with-producer-has-awake-writer"] = and_(
        both, call("<", full, cap), call("<", 0, s.pq), call("=", s.pa, 0)
    )
    obligations["no-producers-active-consumer-can-read"] = and_(
        both,
        call("=", s.pq, 0),
        call("<", 0, s.cq),
        or_(call("=", full, 0), call("=", s.ca, 0)),
    )
    obligations["terminated-buffer-empty"] = and_(
        both, call("=", s.pq, 0), call("=", s.cq, 0), call("<", 0, full)
    )
    start = {
        "pq": str(producers * consumers * quota_scale),
        "cq": str(producers * consumers * quota_scale),
        "p": str(producers),
        "c": str(consumers),
        "pa": str(producers),
        "ca": str(consumers),
        "length": "0",
        "poisons": "0",
        "tail": "false",
    }
    obligations["initial"] = and_(
        *(call("=", getattr(s, k), v) for k, v in start.items()), neg(both)
    )
    req = "(set-logic QF_LIA)\n" + "".join(
        call("declare-const", getattr(q, k.name), "Bool" if k.name == "tail" else "Int")
        + "\n"
        for q in (s, t)
        for k in fields(q)
    )
    for expr in obligations.values():
        req += f"(push)\n(assert {expr})\n(check-sat)\n(pop)\n"
    start = time.monotonic()
    run = subprocess.run(
        [solver, "-in", "-smt2"],
        input=req,
        text=True,
        capture_output=True,
        check=True,
        timeout=10,
    )
    rows = dict(zip(obligations, run.stdout.splitlines(), strict=True))
    return {
        "bounds": [producers, consumers, cap],
        "obligations": rows,
        "wall_ms": 1000 * (time.monotonic() - start),
        "proved": set(rows.values()) == {"unsat"},
    }


CFG = re.compile(
    r"""CONSTANTS\s+BufCapacity\s*=\s*(\d+)\s+
Producers\s*=\s*\{([^}]+)\}\s+Consumers\s*=\s*\{([^}]+)\}\s+Poison\s*=\s*Poison\s+
SPECIFICATION\s+SSpec\s+INVARIANT\s+TypeInv\s+INVARIANT\s+NoDeadlock\s+
PROPERTIES\s+Terminates\s+PROPERTIES\s+QueueEmpty\s+PROPERTIES\s+GlobalTermination
(?:\s+CHECK_DEADLOCK\s+(FALSE))?""",
    re.VERBOSE,
)


def configuration(config):
    match = CFG.fullmatch(re.sub(r"\\\*[^\n]*", "", config).strip())
    if not match:
        raise ValueError(
            "expected the original SSpec roles and finite literal queue parameters"
        )
    capacity, producers, consumers, disabled = match.groups()
    names = [
        tuple(part.strip() for part in role.split(","))
        for role in (producers, consumers)
    ]
    for prefix, role in zip(("p", "c"), names):
        if (
            not 1 <= len(role) <= 64
            or len(set(role)) != len(role)
            or any(not re.fullmatch(f"{prefix}[1-9][0-9]*", name) for name in role)
        ):
            raise ValueError("expected distinct non-empty pN and cN actor carriers")
    if not 1 <= int(capacity) <= 2**31 - 1:
        raise ValueError("expected a positive literal TLC buffer capacity")
    return *names, int(capacity), disabled is not None


def terminal_word(producers, consumers, quota_scale):
    counters = {
        "prod": dict.fromkeys(producers, len(consumers) * quota_scale),
        "cons": dict.fromkeys(consumers, len(producers) * quota_scale),
    }
    rows = []

    def row(full):
        values = {
            role: {"fcn": [[{"model": name}, quota] for name, quota in counts.items()]}
            for role, counts in counters.items()
        }
        values.update(
            buffer={"seq": [{"model": "Poison"}] if full else []}, waitSet={"set": []}
        )
        rows.append({"values": values, "action": None})

    row(False)
    # A full quota exchange is always enabled at positive capacity. The trace
    # names an actual producer and consumer at each pair of steps.
    for producer in producers:
        for consumer in consumers:
            for _ in range(quota_scale):
                counters["prod"][producer] -= 1
                row(True)
                counters["cons"][consumer] -= 1
                row(False)
    return {
        "format": "dive-trace",
        "version": 1,
        "variables": ["prod", "cons", "buffer", "waitSet"],
        "length": len(rows),
        "states": rows,
        "ending": {"kind": "deadlock"},
    }


def validate(dive, module, config, trace, word):
    trace.write_text(json.dumps(word, separators=(",", ":")) + "\n")
    process = subprocess.run(
        [
            dive,
            "validate-trace",
            module,
            "--config",
            config,
            "--trace",
            trace,
            "--seed",
            "0",
            "--fp",
            "0",
            "--compat=dive",
            "--output",
            "json",
        ],
        capture_output=True,
        check=False,
        timeout=30,
        env={
            **{
                key: value
                for key, value in os.environ.items()
                if not key.startswith("DIVE_")
            },
            "XDG_CONFIG_HOME": str(trace.parent / "config"),
        },
    )
    report = json.loads(process.stdout)
    return process.returncode, report


def run(args):
    source, quota_scale, source_hash = admitted_source(args.module)
    cfg = args.config.read_bytes()
    p, c, capacity, deadlock_disabled = configuration(cfg.decode("utf-8"))
    if not deadlock_disabled:
        raise ValueError("complete fair checking requires CHECK_DEADLOCK FALSE")
    solver_identity = subprocess.run(
        [args.solver, "--version"],
        capture_output=True,
        text=True,
        timeout=2,
        check=True,
    ).stdout.strip()
    obligations = proof(len(p), len(c), capacity, args.solver, quota_scale)
    if not obligations["proved"]:
        return 5, {
            "result": "inconclusive",
            "reason": "progress obligations did not close",
            **obligations,
        }
    terminal = terminal_word(p, c, quota_scale)
    with TemporaryDirectory(prefix="dive-poison-queue-proof-") as tmp:
        staging = Path(tmp)
        # Resolve only the standard modules bundled with the trusted validator.
        # A caller's search paths and sibling modules cannot alter the proof.
        module = staging / f"{MODULE}.tla"
        module.write_text(source)
        trace = staging / "terminal.json"
        original_cfg = staging / "deadlock.cfg"
        cfg_text = re.sub(r"\\\*[^\n]*", "", cfg.decode("utf-8")).strip()
        original_cfg.write_text(re.sub(r"\s+CHECK_DEADLOCK\s+FALSE$", "\n", cfg_text))
        start = time.monotonic()
        status, checked = validate(args.dive, module, original_cfg, trace, terminal)
        if status != 0 or checked.get("result") != "valid":
            return 5, {
                "result": "inconclusive",
                "reason": "terminal witness did not independently validate",
                "terminal": checked,
            }
        terminal_row = {
            "result": "deadlock",
            "independent_validation": "valid",
            "states": terminal["length"],
            "wall_ms": 1000 * (time.monotonic() - start),
        }
        if args.out:
            with args.out.open("x") as file:
                file.write(trace.read_text())
    return 0, {
        "format": "dive-poison-queue-progress-v1",
        "result": "proved",
        **obligations,
        "module_sha256": source_hash,
        "quota_scale": quota_scale,
        "cfg_sha256": hashlib.sha256(cfg).hexdigest(),
        "solver": solver_identity,
        "checked_properties": dict.fromkeys(
            ("TypeInv", "NoDeadlock", "Terminates", "QueueEmpty", "GlobalTermination"),
            "holds",
        ),
        "stronger_terminal_empty_invariant": True,
        "terminal": terminal_row,
        "fairness": "WF on each original Get; SF on each original quota-decreasing Put",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("--dive", default="dive")
    parser.add_argument("--solver", default="z3")
    parser.add_argument(
        "--out",
        type=Path,
        help="new output path for the independently validated terminal trace",
    )
    args = parser.parse_args()
    try:
        code, report = run(args)
    except (ValueError, OSError) as error:
        code, report = 3, {"result": "unsupported", "reason": str(error)}
    except (subprocess.TimeoutExpired, subprocess.CalledProcessError) as error:
        code, report = 5, {"result": "inconclusive", "reason": str(error)}
    print(json.dumps(report))
    return code


if __name__ == "__main__":
    raise SystemExit(main())
