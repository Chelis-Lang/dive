#!/usr/bin/env python3
"""Capture pinned TLC -simulate evidence for RandomElement and the Randomization
operators reached in Init, Next, invariants and ENABLED. Writes oracle.json; does
not write goldens.

Usage: capture.py --java JAVA --jar tla2tools.jar --reason TEXT
Each case writes its configuration into a fresh temporary directory next to a
copy of its module, so no generated .cfg is kept here. Raw stdout is not
committed; its SHA-256 and the classified fields are. Each trace that TLC
writes with `file=` is kept as its list of states, one TLA+ string per state.
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
SAFETY = "INIT Init\nNEXT Next\nINVARIANT Safe"


def sim(depth, num, seed=0, *extra):
    return ["-workers", "1", "-seed", str(seed), "-depth", str(depth), *extra,
            "-simulate", f"num={num},file=FILE"]


# id, module, cfg, flags, what the case shows.
CASES = [
    ("N1", "RndNext", SAFETY, sim(3, 2), "RandomElement in Next draws a fresh value at every step of every trace"),
    ("N2", "RndNext", SAFETY, sim(3, 2, 7), "another seed"),
    ("I1", "RndInit", SAFETY, sim(2, 3), "RandomElement in Init is drawn once, when Init is enumerated: every trace starts in the same state"),
    ("E1", "RndExists", SAFETY, sim(3, 2), "one draw per existential binding; the successor is chosen among them"),
    ("D1", "RndDeadlock", SAFETY, sim(10, 20), "a step whose draw disables Next is reported as a deadlock (2114)"),
    ("D2", "RndDeadlock", SAFETY + "\nCHECK_DEADLOCK FALSE", sim(10, 20), "without deadlock checking, the trace ends at that step"),
    ("S1", "RndSubset", SAFETY + "\nCONSTANT K = 2", sim(3, 2), "RandomSubset in Init (drawn once) and Next (k elements)"),
    ("S2", "RndSubset", SAFETY + "\nCONSTANT K = 12", sim(2, 1), "RandomSubset(k, S) with k > Cardinality(S) is all of S"),
    ("S3", "RndSubset", SAFETY + "\nCONSTANT K = 0", sim(2, 1), "RandomSubset(0, S) is the empty set"),
    ("S4", "RndSubsetNegative", SAFETY, sim(2, 1), "RandomSubset with a negative count is an evaluation error"),
    ("R1", "RndSetOfSubsets", SAFETY, sim(3, 2), "RandomSetOfSubsets in Next"),
    ("V1", "RndInvariant", SAFETY, sim(4, 5), "RandomElement in an invariant draws on every check"),
    ("B1", "RndEnabled", SAFETY, sim(4, 5), "RandomElement inside ENABLED"),
    ("C1", "RndConst", SAFETY, sim(2, 3), "a constant-level definition draws once at start-up"),
    ("C2", "RndConst", SAFETY + "\nVIEW View", sim(2, 3), "the same with a VIEW"),
    ("C3", "RndConfigured", SAFETY + "\nCONSTANT C <- Pick", sim(4, 2), "a constant configured with C <- Pick names Pick's body, which draws again at every use"),
]
MESSAGE = re.compile(r"@!@!@STARTMSG (\d+):(\d+) @!@!@\n(.*?)@!@!@ENDMSG \1 @!@!@", re.S)
DECIDING = {1000, 2103, 2104, 2105, 2107, 2108, 2110, 2111, 2112, 2113, 2114, 2116, 2154, 2169}
STATE = re.compile(r"STATE_\d+ == \n(.*?)\n\n", re.S)


def run(java, jar, fixture, module, cfg, flags):
    with tempfile.TemporaryDirectory(prefix="dive-sim-random-") as temp:
        temp = Path(temp)
        shutil.copy(fixture / f"{module}.tla", temp / f"{module}.tla")
        (temp / "case.cfg").write_text(cfg + "\n")
        (temp / "traces").mkdir()
        flags = [f.replace("file=FILE", f"file={temp / 'traces' / 't'}") for f in flags]
        command = [str(java), f"-Djava.io.tmpdir={temp}", "-cp", str(jar), "tlc2.TLC", "-tool",
                   "-metadir", str(temp / "states"), *flags, "-config", "case.cfg", module]
        done = subprocess.run(command, cwd=temp, text=True, capture_output=True, timeout=120)
        traces = [[" ".join(s.split()) for s in STATE.findall(p.read_text())]
                  for p in sorted((temp / "traces").iterdir(), key=lambda p: int(p.name.split("_")[-1]))]
    out = done.stdout
    messages = [(int(code), int(level), text.strip()) for code, level, text in MESSAGE.findall(out)]
    deciding = [(code, " ".join(text.split())[:300]) for code, _, text in messages if code in DECIDING]
    states = [" ".join(text.split("\n", 1)[1].split()) if "\n" in text else ""
              for code, _, text in messages if code == 2217]
    generated = re.search(r"The number of states generated: (\d+)", out)
    return dict(
        exit=done.returncode,
        deciding=[dict(code=code, message=text) for code, text in deciding],
        counterexample=states,
        generated=int(generated[1]) if generated else None,
        traces=traces,
        stdout_sha256=hashlib.sha256(out.encode()).hexdigest(),
        stderr_empty=not done.stderr.strip(),
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
    for cid, module, cfg, flags, shows in CASES:
        result = run(args.java, args.jar, fixture, module, cfg, flags)
        cases.append(dict(id=cid, module=module, cfg=cfg, flags=flags, shows=shows, **result))
    sources = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(fixture.glob("*.tla"))}
    record = dict(oracle=ORACLE, jar_sha256=PIN, java_version=version.stderr.strip().splitlines()[0],
                  reason=args.reason, captured_at=datetime.now(timezone.utc).isoformat(timespec="seconds"),
                  sources=sources, cases=cases)
    (fixture / "oracle.json").write_text(json.dumps(record, indent=1) + "\n")


if __name__ == "__main__":
    main()
