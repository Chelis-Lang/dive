#!/usr/bin/env python3
"""Capture clean pinned PlusCal translation, followed by SANY validation.

Development evidence only. Native translation never calls this script or Java.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time

JAR_SHA = "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def run(command, cwd, timeout, output, stem):
    result = {}
    try:
        process = subprocess.run(command, cwd=cwd, capture_output=True, timeout=timeout)
        stdout, stderr = process.stdout, process.stderr
        result["exit_code"] = process.returncode
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b"", error.stderr or b""
        result.update(status="timeout", timeout_seconds=timeout)
    for name, content in (("stdout", stdout), ("stderr", stderr)):
        filename = stem + "." + name + ".gz"
        (output / filename).write_bytes(gzip.compress(content, mtime=0))
        result[name] = filename
        result[name + "_sha256"] = digest(content)
    return result, stdout, stderr


def capture(args):
    if digest(args.jar.read_bytes()) != JAR_SHA:
        raise ValueError("pinned oracle jar SHA-256 mismatch")
    cases = sorted(args.inputs.glob("*.tla"))
    if not cases:
        raise ValueError("empty translation selection")
    args.output.mkdir(parents=True, exist_ok=True)
    rows = []
    command = [str(args.java), "-Xmx256m", "-cp", str(args.jar)]
    for index, case in enumerate(cases):
        source = case.read_bytes()
        if b"BEGIN TRANSLATION" in source:
            raise ValueError("input already contains a translation: " + case.name)
        row = {"id": case.stem, "input": case.name, "input_sha256": digest(source)}
        with tempfile.TemporaryDirectory(prefix="dive-pluscal-translation-") as temp:
            work = Path(temp)
            module = work / case.name
            module.write_bytes(source)
            primary, stdout, stderr = run(command + ["pcal.trans", case.name], work,
                                          args.timeout, args.output, case.stem + ".translation")
            translated = module.read_bytes()
            if primary.get("status") != "timeout":
                if primary["exit_code"] == 0 and b"BEGIN TRANSLATION" in translated:
                    primary["status"] = "translated"
                elif b"Unrecoverable error" in stdout + stderr:
                    primary["status"] = "rejected"
                else:
                    primary["status"] = "oracle-error"
            row["primary"] = primary
            if primary["status"] == "translated":
                filename = case.stem + ".translated.tla"
                (args.output / filename).write_bytes(translated)
                row["translated"] = {"path": filename, "sha256": digest(translated)}
                config = module.with_suffix(".cfg")
                if config.exists():
                    name = case.stem + ".translated.cfg"
                    data = config.read_bytes()
                    (args.output / name).write_bytes(data)
                    row["config"] = {"path": name, "sha256": digest(data)}
                sany, _, _ = run(command + ["tla2sany.SANY", "-error-codes", case.name],
                                  work, args.timeout, args.output, case.stem + ".sany")
                if sany.get("status") != "timeout":
                    sany["status"] = "accepted" if sany["exit_code"] == 0 else "rejected"
                row["sany"] = sany
        rows.append(row)
        if index % 20 == 0:
            print(f"captured {index + 1}/{len(cases)}", flush=True)
    report = {"schema_version": 1, "reason": args.reason,
              "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "oracle_id": "tlc-2026.10.06.014338", "oracle_sha256": JAR_SHA,
              "source_commit": "94d0c5070852b37b801bb8998c436383ab7ee37b",
              "java_sha256": digest(args.java.read_bytes()),
              "capture_script_sha256": digest(Path(__file__).read_bytes()),
              "primary_command": ["java", "-Xmx256m", "-cp", "<pinned-jar>", "pcal.trans", "<case.tla>"],
              "validation_command": ["java", "-Xmx256m", "-cp", "<pinned-jar>", "tla2sany.SANY", "-error-codes", "<case.tla>"],
              "rows": rows}
    (args.output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("java", "jar", "inputs", "output"):
        parser.add_argument("--" + name, type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--reason", required=True)
    parser.add_argument("--timeout", type=float, default=5)
    capture(parser.parse_args())
