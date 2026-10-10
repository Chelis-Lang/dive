#!/usr/bin/env python3
"""Capture clean pinned TLC simulation evidence. This does not write goldens."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
from datetime import datetime, timezone

PIN = "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
ORACLE = "tlc-2026.10.06.014338"


def run(java, jar, fixture, model, flags, temp):
    command = [str(java), "-cp", str(jar), "tlc2.TLC", "-tool", "-workers", "1", *flags,
               "-config", str(fixture / f"{model}.cfg"), str(fixture / f"{model}.tla")]
    return subprocess.run(command, cwd=temp, text=True, capture_output=True, timeout=30)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--java", type=Path, required=True)
    parser.add_argument("--jar", type=Path, required=True)
    parser.add_argument("--reason", required=True)
    args = parser.parse_args()
    if hashlib.sha256(args.jar.read_bytes()).hexdigest() != PIN:
        raise SystemExit("pinned jar SHA-256 mismatch")
    java_version = subprocess.run([str(args.java), "-version"], text=True, capture_output=True, check=True)
    fixture = Path(__file__).resolve().parent
    choices = []
    errors = []
    traces_dir = fixture / "tlc-traces"
    traces_dir.mkdir(exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="dive-simulation-oracle-") as directory:
        temp = Path(directory)
        for seed in (0, 1, 7, 2147483647, -1):
            for aril in (0, 3):
                prefix = temp / f"s{seed}-a{aril}"
                result = run(args.java, args.jar, fixture, "SimChoice",
                             ["-seed", str(seed), "-aril", str(aril), "-depth", "5", "-simulate", f"num=3,file={prefix}"], temp)
                stem = f"SimChoice.seed{seed}.aril{aril}"
                (fixture / f"{stem}.stdout").write_text(result.stdout)
                (fixture / f"{stem}.stderr").write_text(result.stderr)
                files = sorted(temp.glob(f"{prefix.name}_0_*"))
                if result.returncode or len(files) != 3:
                    raise SystemExit(f"{stem} did not capture three complete traces")
                states = []
                for number, file in enumerate(files):
                    shutil.copyfile(file, traces_dir / f"{stem}.trace{number}.tla")
                    states.append([int(s) for s in re.findall(r"^x = (-?\d+)$", file.read_text(), re.M)])
                generated = int(re.search(r"The number of states generated: (\d+)", result.stdout)[1])
                choices.append(dict(seed=seed, aril=aril, exit=result.returncode, traces=states, generated=generated))
        for file in sorted(fixture.glob("Sim*.tla")):
            if file.stem == "SimChoice":
                continue
            result = run(args.java, args.jar, fixture, file.stem,
                         ["-seed", "0", "-depth", "4", "-simulate", "num=1"], temp)
            (fixture / f"{file.stem}.stdout").write_text(result.stdout)
            (fixture / f"{file.stem}.stderr").write_text(result.stderr)
            generated = re.search(r"The number of states generated: (\d+)", result.stdout)
            errors.append(dict(model=file.stem, native_scope="unsupported" if file.stem == "SimConstrainedAction" else "supported", exit=result.returncode,
                               codes=[int(c) for c in re.findall(r"STARTMSG (\d+):1", result.stdout)],
                               states=[int(s) for s in re.findall(r"(?:^|\n)(?:/\\ )?x = (-?\d+)", result.stdout)],
                               generated=int(generated[1]) if generated else None))
    (fixture / "choice-oracle.json").write_text(json.dumps(choices, indent=2) + "\n")
    (fixture / "error-oracle.json").write_text(json.dumps(errors, indent=2) + "\n")
    files = [p for p in fixture.rglob("*") if p.is_file() and p.suffix in (".tla", ".cfg", ".stdout", ".stderr")]
    provenance = dict(schema_version=1, oracle=ORACLE, jar_sha256=PIN,
                      source_revision="94d0c5070852b37b801bb8998c436383ab7ee37b",
                      expected_jre_archive_sha256="190480874ccceb358cbc840393207f77ac3e63a4c5f8129d0e23e9518b96ad05",
                      java_executable_sha256=hashlib.sha256(args.java.read_bytes()).hexdigest(),
                      java_version=java_version.stderr.strip(),
                      jre_archive_verification="archive verification is performed by the cache fetcher; capture records the actual executable",
                      reason=args.reason, captured_at=datetime.now(timezone.utc).isoformat(),
                      primary_flags=["-tool", "-workers", "1", "-seed", "<seed>", "-aril", "<aril>", "-depth", "<depth>", "-simulate", "num=<traces>"],
                      raw_files={str(p.relative_to(fixture)): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(files)})
    (fixture / "capture-provenance.json").write_text(json.dumps(provenance, indent=2) + "\n")


if __name__ == "__main__":
    main()
