"""Recapture the pinned TLC recovery contract, without changing goldens.

Usage: python3 -I capture_tlc.py PINNED_JAVA PINNED_JAR FRESH_OUTPUT_DIRECTORY
Timed checkpoint prefixes vary; this preserves each raw run independently.
"""
import hashlib
import json
import pathlib
import shutil
import subprocess
import sys

java, jar, out = map(lambda s: pathlib.Path(s).resolve(), sys.argv[1:])
assert hashlib.sha256(jar.read_bytes()).hexdigest() == "7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d"
out.mkdir(parents=True, exist_ok=False)
root = pathlib.Path(__file__).resolve().parent
for name in ("CheckpointGrid.tla", "CheckpointGrid.cfg"):
    shutil.copy2(root/name, out/name)
base = [str(java), "-Xmx512m", "-cp", str(jar), "tlc2.TLC", "-tool", "-workers", "1", "-seed", "7", "-fp", "0", "-config", "CheckpointGrid.cfg"]
runs = []
def run(name, args):
    result = subprocess.run(args, cwd=out, capture_output=True, timeout=120)
    (out/f"{name}.stdout").write_bytes(result.stdout)
    (out/f"{name}.stderr").write_bytes(result.stderr)
    runs.append({"name": name, "args": args, "exit": result.returncode,
                 "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                 "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()})
    return result.returncode
pause = [str(java), "-Xmx512m", "-Dtlc2.TLC.stopAfter=1", *base[2:], "-checkpoint", "1", "-metadir", str(out/"pause-meta"), "CheckpointGrid"]
assert run("pause", pause) == 0
assert run("recover-parent-negative", base + ["-recover", str(out/"pause-meta"), "CheckpointGrid"]) != 0
checkpoints = [p for p in (out/"pause-meta").iterdir() if p.is_dir() and (p/"vars.chkpt").exists()]
assert len(checkpoints) == 1
assert run("recover", base + ["-recover", str(checkpoints[0]), "CheckpointGrid"]) == 0
assert run("full", base + ["-metadir", str(out/"full-meta"), "CheckpointGrid"]) == 0
(out/"runs.json").write_text(json.dumps(runs, indent=2) + "\n")
