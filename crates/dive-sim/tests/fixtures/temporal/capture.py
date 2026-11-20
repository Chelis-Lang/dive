#!/usr/bin/env python3
"""Capture pinned TLC -simulate evidence for temporal PROPERTY, SYMMETRY, VIEW,
ALIAS, implied properties and workers. Writes oracle.json; does not write goldens.

Usage: capture.py --java JAVA --jar tla2tools.jar --reason TEXT
Each case writes its configuration into a fresh temporary directory next to a
copy of its module, so no generated .cfg is kept here. Raw stdout is not
committed; its SHA-256 and the classified fields are.
"""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone
from pathlib import Path

PIN = "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
ORACLE = "tlc-2026.10.06.014338"
SYM = "CONSTANTS a = a b = b\n"


def sim(depth, num=20, seed=1, workers=1, *extra):
    return ["-workers", str(workers), "-seed", str(seed), "-depth", str(depth), *extra,
            "-simulate", f"num={num}"]


# id, module, cfg, flags, truth, assessment of TLC's answer against the truth.
# "truth" is the TLA+ answer for the property over all behaviors; a sample can
# only refute. "sound" means TLC's answer is a correct conclusion from its sample.
CASES = [
    ("C1", "Cycle", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive", sim(10), "violated", "sound: lasso found on the first trace"),
    ("C2", "Cycle", "SPECIFICATION FairSpec\nPROPERTY InfOftenFive", sim(10), "violated", "sound"),
    ("C3", "Cycle", "SPECIFICATION FairSpec\nPROPERTY LeadsToFive", sim(10), "violated", "sound"),
    ("C4", "Cycle", "SPECIFICATION FairSpec\nPROPERTY InfOftenZero", sim(10), "holds", "sound: no error"),
    ("C5", "Cycle", "SPECIFICATION FairSpec\nPROPERTY AlwaysSmall", sim(10), "holds", "sound: checked as an invariant"),
    ("C6", "Cycle", "SPECIFICATION FairSpec\nPROPERTY AlwaysSmallFalse", sim(10), "violated", "sound: []P is checked as an invariant (2110, exit 12)"),
    ("C7", "Cycle", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive", sim(2), "violated", "silent: depth 2 never closes the loop; reports success without saying liveness went unchecked"),
    ("C8", "Cycle", "SPECIFICATION FairSpec\nPROPERTY InfOftenZero", sim(2), "holds", "sound: truncated prefix is not treated as stuttering"),
    ("C9", "Cycle", "SPECIFICATION Spec\nPROPERTY InfOftenZero", sim(10), "violated", "sound: without fairness, stuttering at a state is a behavior"),
    ("C10", "Cycle", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive", sim(10, 50, 1, 4), "violated", "sound; 4 workers"),
    ("C11", "Cycle", "SPECIFICATION FairSpec\nPROPERTY InfOftenZero", sim(10, 50, 1, 4), "holds", "num is per worker: 4 x 50 traces"),
    ("T1", "Terminal", "SPECIFICATION FairSpec\nPROPERTY EventuallyTwo", sim(10, 20, 1, 1, "-deadlock"), "holds", "sound"),
    ("T2", "Terminal", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive", sim(10, 20, 1, 1, "-deadlock"), "violated", "sound: no successor is infinite stuttering"),
    ("T3", "Terminal", "SPECIFICATION FairSpec\nPROPERTY InfOftenZero", sim(10, 20, 1, 1, "-deadlock"), "violated", "sound"),
    ("T4", "Terminal", "SPECIFICATION FairSpec\nPROPERTY EventuallyFive", sim(10), "violated", "deadlock (2114) decides before the temporal check"),
    ("U1", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY EventuallyTen", sim(5), "holds", "silent: truncated, liveness unchecked"),
    ("U2", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY InfOftenEven", sim(5), "holds", "silent: truncated, liveness unchecked"),
    ("U3", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY LeadsToTen", sim(5), "holds", "silent: truncated, liveness unchecked"),
    ("U4", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY AlwaysBelowFour", sim(5), "violated", "sound: bad prefix found"),
    ("U5", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY AlwaysBelowTen", sim(5), "violated", "incomplete: violation lies beyond -depth"),
    ("U6", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY StepIsIncrement", sim(5), "holds", "sound"),
    ("U7", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY StepIsDouble", sim(5), "violated", "sound: [][A]_v checked per step (2112)"),
    ("U8", "Unbounded", "SPECIFICATION Spec\nPROPERTY EventuallyTen", sim(5), "violated", "sound: stuttering lasso"),
    ("U9", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY EventuallyTen", sim(20), "holds", "silent: truncated, liveness unchecked"),
    ("U10", "Unbounded", "SPECIFICATION FairSpec\nCONSTRAINT BelowFour\nPROPERTY EventuallyTen", sim(20), "holds", "deadlock reported where the constraint excludes every successor"),
    ("U10d", "Unbounded", "SPECIFICATION FairSpec\nCONSTRAINT BelowFour\nPROPERTY EventuallyTen", sim(20, 20, 1, 1, "-deadlock"), "holds", "sound: the constraint-cut stutter is unfair because Next stays enabled"),
    ("U11", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY InitZero", sim(5), "holds", "sound"),
    ("U12", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY InitOne", sim(5), "violated", "missed: -simulate never checks implied initial properties (BFS reports 2108)"),
    ("U13", "Unbounded", "SPECIFICATION FairSpec\nCONSTRAINT BelowThree\nPROPERTY StepBelowThree", sim(10), "violated", "action property checked on a candidate the constraint then excludes, as in BFS"),
    ("U14", "Unbounded", "SPECIFICATION FairSpec\nCONSTRAINT BelowFour\nPROPERTY AlwaysBelowFour", sim(10), "violated", "invariant checked on an excluded candidate, as in BFS"),
    ("F1", "Fair", "SPECIFICATION Spec\nPROPERTY EventuallyB", sim(10, 50), "violated", "sound: stuttering"),
    ("F2", "Fair", "SPECIFICATION WFSpec\nPROPERTY EventuallyB", sim(10, 50), "holds", "sound: unfair lassos rejected"),
    ("F3", "Fair", "SPECIFICATION SFSpec\nPROPERTY EventuallyB", sim(10, 50), "holds", "sound"),
    ("S1", "Strong", "SPECIFICATION WFSpec\nPROPERTY EventuallyB", sim(10, 50), "violated", "sound: WF admits the toggling lasso"),
    ("S2", "Strong", "SPECIFICATION SFSpec\nPROPERTY EventuallyB", sim(10, 50), "holds", "sound: SF rejects it"),
    ("V1", "ViewLoop", "SPECIFICATION FairSpec\nPROPERTY EventuallyThree\nVIEW View", sim(10), "holds", "unsound: loop closed on the view; spurious counterexample (BFS too)"),
    ("V2", "ViewLoop", "SPECIFICATION FairSpec\nPROPERTY EventuallyThree", sim(10), "holds", "sound control without VIEW"),
    ("V3", "ViewLoop", "SPECIFICATION FairSpec\nINVARIANT BadY\nVIEW View", sim(10), "violated", "sound: VIEW does not affect safety"),
    ("V4", "ViewLoop", "SPECIFICATION FairSpec\nINVARIANT SafeY\nVIEW View", sim(10), "holds", "sound"),
    ("Y1", "SymLoop", SYM + "SPECIFICATION FairSpec\nPROPERTY EventuallyB\nSYMMETRY Perms", sim(10), "holds", "sound"),
    ("Y2", "SymLoop", SYM + "SPECIFICATION FairSpec\nPROPERTY NeverOther\nSYMMETRY Perms", sim(10, 3), "violated", "missed: symmetric fingerprints turn the a-b loop into an unfair self-loop"),
    ("Y3", "SymLoop", SYM + "SPECIFICATION FairSpec\nPROPERTY NeverOther", sim(10, 3), "violated", "sound control without SYMMETRY"),
    ("Y4", "SymLoop", SYM + "SPECIFICATION FairSpec\nINVARIANT NotB\nSYMMETRY Perms", sim(10), "violated", "sound: SYMMETRY does not affect safety"),
    ("A1", "Unbounded", "SPECIFICATION FairSpec\nINVARIANT BelowFour\nALIAS AliasDef", sim(10), "violated", "sound: ALIAS only renders the trace"),
    ("A2", "Unbounded", "SPECIFICATION FairSpec\nINVARIANT BelowTen\nALIAS AliasDef", ["-workers", "1", "-seed", "1", "-depth", "3", "-simulate", "num=2,file=FILE"], "holds", "file= writes one module per trace, named PREFIX_worker_n, without ALIAS"),
    ("A3", "Unbounded", "SPECIFICATION FairSpec\nINVARIANT BelowFour\nALIAS AliasDef", ["-workers", "1", "-seed", "1", "-depth", "6", "-simulate", "num=2,file=FILE"], "violated", "the violating trace is printed with ALIAS; file= writes no module for it"),
    ("G1", "BigNext", "INIT Init\nNEXT Next\nINVARIANT Small", sim(20, 5), "holds", "-simulate enumerates all 10^6 initial states and all 10^6 successors of each step"),
    ("G2", "BigNext", "INIT Init\nNEXT Next\nINVARIANT Small", ["-workers", "1", "-seed", "1", "-depth", "20", "-generate", "num=5"], "holds", "-generate enumerates Init but builds one successor per step"),
    ("G3", "InfInit", "INIT Init\nNEXT Next\nINVARIANT Small", sim(20, 5), "holds", "Init over Nat is not enumerable: GENERAL 1000, exit 255"),
    ("G4", "InfInit", "INIT Init\nNEXT Next\nINVARIANT Small", ["-workers", "1", "-seed", "1", "-depth", "20", "-generate", "num=5"], "holds", "same under -generate"),
    ("G5", "InfNext", "INIT Init\nNEXT Next\nINVARIANT Small", sim(20, 5), "holds", "successors over Nat are not enumerable"),
    ("G6", "InfNext", "INIT Init\nNEXT Next\nINVARIANT Small", ["-workers", "1", "-seed", "1", "-depth", "20", "-generate", "num=5"], "holds", "same under -generate"),
]
# Seed behavior with workers: the same command repeated.
REPEATS = [
    ("W1", "Walk", "SPECIFICATION Spec\nINVARIANT NotFourteen", sim(20, 1000000, 7, 1), 3),
    ("W4", "Walk", "SPECIFICATION Spec\nINVARIANT NotFourteen", sim(20, 1000000, 7, 4), 6),
]
# BFS controls for the simulate cases whose answer differs from model checking.
BFS = [
    ("B-U12", "Unbounded", "SPECIFICATION FairSpec\nPROPERTY InitOne"),
    ("B-U13", "Unbounded", "SPECIFICATION FairSpec\nCONSTRAINT BelowThree\nPROPERTY StepBelowThree"),
    ("B-V1", "ViewLoop", "SPECIFICATION FairSpec\nPROPERTY EventuallyThree\nVIEW View"),
    ("B-Y2", "SymLoop", SYM + "SPECIFICATION FairSpec\nPROPERTY NeverOther\nSYMMETRY Perms"),
]
MESSAGE = re.compile(r"@!@!@STARTMSG (\d+):(\d+) @!@!@\n(.*?)@!@!@ENDMSG \1 @!@!@", re.S)
DECIDING = {1000, 2104, 2105, 2107, 2108, 2110, 2111, 2112, 2113, 2114, 2116}


def run(java, jar, fixture, module, cfg, flags):
    with tempfile.TemporaryDirectory(prefix="dive-sim-temporal-") as temp:
        temp = Path(temp)
        shutil.copy(fixture / f"{module}.tla", temp / f"{module}.tla")
        (temp / "case.cfg").write_text(cfg + "\n")
        (temp / "traces").mkdir()
        flags = [f.replace("file=FILE", f"file={temp / 'traces' / 't'}") for f in flags]
        command = [str(java), f"-Djava.io.tmpdir={temp}", "-cp", str(jar), "tlc2.TLC", "-tool",
                   "-metadir", str(temp / "states"), *flags, "-config", "case.cfg", module]
        done = subprocess.run(command, cwd=temp, text=True, capture_output=True, timeout=120)
        files = sorted(p.name for p in (temp / "traces").iterdir())
        assigned = sorted({line.split(" = ")[0] for p in (temp / "traces").iterdir()
                           for line in p.read_text().splitlines() if " = " in line})
    out = done.stdout
    messages = [(int(code), int(level), text.strip()) for code, level, text in MESSAGE.findall(out)]
    deciding = [(code, text.splitlines()[0]) for code, _, text in messages if code in DECIDING]
    states = [text.split("\n", 1)[1].strip() if "\n" in text else "" for code, _, text in messages if code == 2217]
    back = [re.match(r"(\d+):", text)[1] for code, _, text in messages if code == 2122]
    generated = re.search(r"The number of states generated: (\d+)", out)
    return dict(
        exit=done.returncode,
        deciding=[dict(code=code, message=text) for code, text in deciding],
        counterexample=states,
        back_to_state=int(back[0]) if back else None,
        stuttering=any(code == 2218 for code, _, _ in messages) or "Stuttering" in out,
        generated=int(generated[1]) if generated else None,
        stdout_sha256=hashlib.sha256(out.encode()).hexdigest(),
        stderr_empty=not done.stderr.strip(),
        trace_files=files,
        trace_file_variables=assigned,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--reason", required=True)
    args = parser.parse_args()
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != PIN:
        raise SystemExit("pinned jar SHA-256 mismatch")
    fixture = Path(__file__).resolve().parent
    version = subprocess.run([str(args.java), "-version"], text=True, capture_output=True, check=True)
    cases = []
    for cid, module, cfg, flags, truth, assessment in CASES:
        result = run(args.java, args.jar, fixture, module, cfg, flags)
        cases.append(dict(id=cid, module=module, cfg=cfg, flags=flags, truth=truth, tlc=assessment, **result))
    repeats = []
    for cid, module, cfg, flags, count in REPEATS:
        runs = [run(args.java, args.jar, fixture, module, cfg, flags) for _ in range(count)]
        repeats.append(dict(id=cid, module=module, cfg=cfg, flags=flags,
                            exits=[r["exit"] for r in runs], generated=[r["generated"] for r in runs],
                            counterexample_sha256=[hashlib.sha256("\n".join(r["counterexample"]).encode()).hexdigest()[:16] for r in runs],
                            counterexample_states=[len(r["counterexample"]) for r in runs]))
    bfs = []
    for cid, module, cfg in BFS:
        result = run(args.java, args.jar, fixture, module, cfg, ["-workers", "1"])
        bfs.append(dict(id=cid, module=module, cfg=cfg, **result))
    sources = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(fixture.glob("*.tla"))}
    record = dict(oracle=ORACLE, jar_sha256=PIN, java_version=version.stderr.strip().splitlines()[0],
                  reason=args.reason, captured_at=datetime.now(timezone.utc).isoformat(timespec="seconds"),
                  sources=sources, cases=cases, repeats=repeats, bfs_controls=bfs)
    (fixture / "oracle.json").write_text(json.dumps(record, indent=1) + "\n")


if __name__ == "__main__":
    main()
