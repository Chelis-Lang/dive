#!/usr/bin/env python3
"""Compare actual source-admitted occupancy searches with labeled Core and pinned TCommit counts."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import tomllib

from check_transaction_commit import expected
from check_occupancy_predicate import check_certificates


CREW = r"""---- MODULE Crew ----
EXTENDS TLC
CONSTANT CrewIds, Chief
VARIABLE job
Bounds == job \in [CrewIds -> {"idle","waiting","busy","broken"}]
Empty == job = [w \in CrewIds |-> "idle"]
StartState == Empty \/ Empty
Wait(w) == /\ job[w] = "idle" /\ job' = [job EXCEPT ![w] = "waiting"]
NoBusy == \A person \in CrewIds : job[person] # "busy"
Begin(w) == /\ job[w] = "waiting" /\ NoBusy /\ job' = [job EXCEPT ![w] = "busy"]
End(w) == /\ job[w] = "busy" /\ job' = [job EXCEPT ![w] = "idle"]
Step == \/ \E w \in CrewIds : Wait(w) \/ Wait(w) \/ Begin(w) \/ End(w) \/ UNCHANGED job
        \/ UNCHANGED job
Safe == \A w \in CrewIds : job[w] # "broken"
FixedOwner == \E w \in CrewIds : /\ w = Chief /\ Wait(w)
Partial == \E w \in {Chief} : Wait(w)
ReadChief == job[Chief] = "idle"
OutOfType == \E w \in CrewIds : job' = [job EXCEPT ![w] = "unknown"]
Sym == Permutations(CrewIds)
====
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path, nargs="?", help="pinned Examples transaction_commit directory")
    parser.add_argument("--dive", default="target/release/dive")
    parser.add_argument("--exporter", default="target/release/examples/symbolic_input")
    parser.add_argument("--installed", action="store_true")
    args = parser.parse_args()
    if args.model is None:
        repo = Path(__file__).resolve().parents[2]
        pin = next(c["commit"] for c in tomllib.loads((repo / "conformance/corpora.toml").read_text())["corpus"]
                   if c["id"] == "examples")
        cache = Path(os.environ.get("DIVE_CACHE_DIR", Path.home() / ".cache/dive"))
        args.model = cache / "git/examples" / pin / "specifications/transaction_commit"

    def run(module, cfg, *flags, code=0):
        if args.installed:
            cmd = [args.dive, "check", module, "--config", cfg, "--strategy", "occupancy", *flags, "--output", "json"]
        else:
            cmd = [sys.executable, Path(__file__).with_name("check.py"), module, cfg, "--occupancy", *flags,
                   "--exporter", args.exporter, "--dive", args.dive]
        proc = subprocess.run(cmd, capture_output=True, text=True)
        assert proc.returncode == code, (cmd, proc.returncode, proc.stdout, proc.stderr)
        return json.loads(proc.stdout)

    def core(module, cfg):
        proc = subprocess.run([args.dive, "check", module, "--config", cfg, "--store", "exact", "--workers", "1",
                               "--compat=tlc", "--seed", "0", "--fp", "0", "--output", "json"],
                              capture_output=True, text=True)
        row = json.loads(proc.stdout)
        assert row["verdict"] in ("success", "invariant-violation", "deadlock"), (row, proc.stderr)
        return row

    def compare(module, cfg, name):
        row = run(module, cfg, "--type-invariant", name)
        native = core(module, cfg)
        assert row["verdict"] == native["verdict"] == "success"
        for field in ("distinct", "generated", "depth"):
            assert row[field] == native["statistics"][field], (cfg.read_text(), field, row, native["statistics"])
        assert row["distinct_decimal"] == str(row["distinct"]) and row["generated_decimal"] == str(row["generated"])
        return row

    def replay(module, cfg, trace, result):
        proc = subprocess.run([args.dive, "validate-trace", module, "--config", cfg, "--trace", trace,
                               "--compat=tlc", "--output", "json"], capture_output=True, text=True)
        assert (proc.returncode, json.loads(proc.stdout)["result"]) == (0 if result == "valid" else 1, result)

    with tempfile.TemporaryDirectory() as tmp:
        folder = Path(tmp)
        tcommit = folder / "TCommit.tla"
        tcommit.write_bytes((args.model / tcommit.name).read_bytes())
        base = (args.model / "TCommit.cfg").read_text()
        assert base.count("{r1, r2, r3}") == 1
        cfg = folder / "Model.cfg"
        for n in range(6):
            cfg.write_text(base.replace("{r1, r2, r3}", "{" + ", ".join(f"r{i+1}" for i in range(n)) + "}"))
            row = compare(tcommit, cfg, "TCTypeOK")
            counts = expected(n) if n else (1, 1, 1)
            assert (row["distinct"], row["generated"], row["depth"]) == counts
            assert row["occupancies"] == (n+2)*(n+1)//2+n

        check_certificates(folder, tcommit, cfg, base, run, compare)

        members = ", ".join(f"r{i}" for i in range(8193))
        cfg.write_text(base.replace("{r1, r2, r3}", "{" + members + "}"))
        oversized = run(tcommit, cfg, "--type-invariant", "TCTypeOK", code=2)
        assert "owner count" in oversized["reason"]

        crew = folder / "Crew.tla"
        crew.write_text(CREW)
        cfgbody = ('CONSTANTS CrewIds={a,b,c} Chief=a\nINIT StartState\nNEXT Step\n'
                   'INVARIANTS Bounds Safe\nCHECK_DEADLOCK FALSE\n')
        cfg.write_text(cfgbody)
        original = compare(crew, cfg, "Bounds")
        crew.write_text(CREW.replace("StartState == Empty \\/ Empty",
                                     r'StartState == Empty \/ job = [w \in CrewIds |-> "busy"]'))
        multiple = compare(crew, cfg, "Bounds")
        assert multiple["distinct"] > original["distinct"]
        crew.write_text(CREW)
        cfg.write_text(cfgbody.replace("INIT StartState", "INIT Empty"))
        single = compare(crew, cfg, "Bounds")
        assert original["generated"] == single["generated"] + 1
        crew.write_text(CREW.replace("Wait(w) \\/ Wait(w)", "Wait(w)"))
        once = compare(crew, cfg, "Bounds")
        assert once["distinct"] == single["distinct"] and once["generated"] < single["generated"]
        crew.write_text(CREW)
        cfg.write_text(cfgbody + "INVARIANT NoBusy\n")
        trace = folder / "word.json"
        bad = run(crew, cfg, "--type-invariant", "Bounds", "--trace", trace, code=1)
        assert bad["trace_validated"] and bad["ending"] == {"kind": "invariant", "name": "NoBusy"}
        assert bad["depth"] == core(crew, cfg)["statistics"]["depth"] == 3
        replay(crew, cfg, trace, "valid")
        word = json.loads(trace.read_text())
        word["states"][-1] = word["states"][0]
        trace.write_text(json.dumps(word))
        replay(crew, cfg, trace, "invalid")

        crew.write_text(CREW.replace('job EXCEPT ![w] = "idle"', 'job EXCEPT ![w] = "broken"'))
        cfg.write_text(cfgbody)
        changed = run(crew, cfg, "--type-invariant", "Bounds", code=1)
        assert changed["trace_validated"] and changed["ending"]["name"] == "Safe"
        assert changed["depth"] == core(crew, cfg)["statistics"]["depth"] == 4
        crew.write_text(CREW)
        for text, reason in ((cfgbody.replace("NEXT Step", "NEXT FixedOwner"), "FcnApply"),
                             (cfgbody.replace("NEXT Step", "NEXT Partial"), "complete owner domain"),
                             (cfgbody.replace("NEXT Step", "NEXT OutOfType"), "outside the declared"),
                             (cfgbody + "INVARIANT ReadChief\n", "currently bound"),
                             (cfgbody + "SYMMETRY Sym\n", "configured symmetry"),
                             (cfgbody + "CONSTRAINT NoBusy\n", "state constraint")):
            cfg.write_text(text)
            refused = run(crew, cfg, "--type-invariant", "Bounds", code=2)
            assert refused["verdict"] == "unsupported" and reason in refused["reason"], (reason, refused)

        cfg.write_text(cfgbody)
        limited = run(crew, cfg, "--type-invariant", "Bounds", "--max-orbits", "1", code=4)
        assert limited["verdict"] == "stopped" and "distinct" not in limited
        dead = CREW.replace('Step == \\/', 'Step == FALSE /\\ UNCHANGED job\nDeclaredStep == \\/')
        assert dead != CREW
        crew.write_text(dead)
        cfg.write_text(cfgbody.replace("CHECK_DEADLOCK FALSE", "CHECK_DEADLOCK TRUE"))
        stopped = run(crew, cfg, "--type-invariant", "Bounds", code=1)
        assert stopped["ending"] == {"kind": "deadlock"} and stopped["trace_validated"] and stopped["depth"] == 1
    print("occupancy: original counts; independent source, multiplicities, mutation, replay and refusal controls passed")


if __name__ == "__main__":
    main()
