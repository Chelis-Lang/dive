#!/usr/bin/env python3
"""Generate pinned PlusCal primary and explicitly secondary grammar evidence."""
import argparse
import base64
import hashlib
import gzip
import json
from pathlib import Path
import subprocess
import tempfile
import time

JAR_SHA = "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def capture(args):
    if sha(args.jar) != JAR_SHA:
        raise ValueError("pinned oracle jar SHA-256 mismatch")
    args.output.mkdir(parents=True, exist_ok=True)
    cases = sorted(args.inputs.glob("*.pcal"))
    rows = []
    with tempfile.TemporaryDirectory(prefix="dive-pluscal-probe-") as classes:
        adapter = Path(__file__).with_name("PlusCalProbe.java")
        compilation = subprocess.run([str(args.javac), "-cp", str(args.jar), "-d", classes, str(adapter)], capture_output=True, timeout=30)
        if compilation.returncode != 0:
            raise ValueError("Java probe compilation failed: " + compilation.stderr.decode())
        for index, case in enumerate(cases):
            row = {"id": case.stem, "input": case.name, "input_sha256": sha(case)}
            for kind in ("primary", "source-grammar-secondary"):
                command = [str(args.java), "-Xmx256m", "-cp", classes + ":" + str(args.jar), "PlusCalProbe"]
                if kind != "primary":
                    command.append("--source-grammar")
                command.append(case.name)
                stem = row["id"] + "." + kind
                result = {"stdout": stem + ".stdout.gz", "stderr": stem + ".stderr.gz"}
                try:
                    process = subprocess.run(command, cwd=args.inputs, capture_output=True, timeout=args.timeout)
                    stdout, stderr = process.stdout, process.stderr
                    result["exit_code"] = process.returncode
                    found = []
                    for line in stdout.decode("utf-8", errors="replace").splitlines():
                        columns = line.split("\t")
                        if len(columns) == 3 and columns[0] == case.name and columns[1] in ("accepted", "rejected"):
                            decoded = base64.b64decode(columns[2]).decode("utf-8")
                            found.append((columns[1], decoded))
                    if process.returncode == 0 and len(found) == 1:
                        result["status"], data = found[0]
                        if result["status"] == "accepted":
                            json.loads(data)  # AST remains in the retained raw response.
                        else:
                            result["diagnostic"] = data
                    else:
                        result["status"] = "oracle-error"
                except subprocess.TimeoutExpired as error:
                    stdout, stderr = error.stdout or b"", error.stderr or b""
                    result.update(status="timeout", timeout_seconds=args.timeout)
                (args.output / result["stdout"]).write_bytes(gzip.compress(stdout, mtime=0))
                (args.output / result["stderr"]).write_bytes(gzip.compress(stderr, mtime=0))
                result["stdout_sha256"] = hashlib.sha256(stdout).hexdigest()
                result["stderr_sha256"] = hashlib.sha256(stderr).hexdigest()
                row[kind] = result
            rows.append(row)
            if index % 25 == 0:
                print(f"captured {index + 1}/{len(cases)}", flush=True)
    report = {"schema_version": 1, "reason": args.reason, "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
              "oracle_id": "tlc-2026.10.06.014338", "oracle_sha256": JAR_SHA,
              "source_commit": "94d0c5070852b37b801bb8998c436383ab7ee37b",
              "adapter_sha256": sha(adapter), "capture_script_sha256": sha(Path(__file__)), "java_sha256": sha(args.java), "javac_sha256": sha(args.javac),
              "primary_stage": "getAlgorithm: grammar plus normalization, not translation",
              "secondary_stage": "public grammar-method adapter; excludes global normalization; GetMacro and call tails still normalize",
              "rows": rows}
    (args.output / "manifest.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("java", "javac", "jar", "inputs", "output"):
        parser.add_argument("--" + name, type=lambda p: Path(p).resolve(), required=True)
    parser.add_argument("--reason", required=True)
    parser.add_argument("--timeout", type=float, default=5)
    capture(parser.parse_args())
