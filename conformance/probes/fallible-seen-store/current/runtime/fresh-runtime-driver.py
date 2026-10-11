"""Run only after exact frozen full gate closes; never overwrite older proof."""

from pathlib import Path
import hashlib
import json
import os
import re
import shutil
import subprocess
import time

PRODUCER = "0be524349e4b09f452ee399edc7005653a4825d5"
ROOT = Path("/Users/jeff/code/dive-trace-representation-work")
ART = Path("/Users/jeff/code/dive-storage-tuple-function-artifacts")
TARGET = Path("/Users/jeff/code/dive-trace-representation-target")
REQ = {
    "CARGO_TARGET_DIR": str(TARGET),
    "CARGO_BUILD_JOBS": "2",
    "DIVE_CACHE_DIR": "/Users/jeff/code/dive-trace-representation-cache",
}
for name, value in REQ.items():
    assert os.environ.get(name) == value, (name, os.environ.get(name))
assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == PRODUCER
assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip()
gate = json.loads((ART / "seen-library-current-full-gate/index.json").read_text())
assert gate["producer_commit"] == PRODUCER and len(gate["stages"]) == 8
assert all(c["exit"] == 0 for stage in gate["stages"] for c in stage["commands"])
assert "1179 tests run: 1179 passed, 0 skipped" in (ART / "seen-library-current-full-gate/nextest-0.log").read_text()

OUT = ART / "seen-library-current-runtime"
BIN = ART / "seen-library-current-binaries"
OUT.mkdir(exist_ok=False)
BIN.mkdir(exist_ok=False)
env = dict(os.environ)
rows = []
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()


def execute(name, command, extra=None):
    before = time.monotonic()
    stdout = OUT / (name + ".stdout")
    stderr = OUT / (name + ".stderr")
    print("START", name, flush=True)
    with stdout.open("wb") as out, stderr.open("wb") as err:
        result = subprocess.run(command, cwd=ROOT, env={**env, **(extra or {})}, stdout=out, stderr=err)
    row = {
        "name": name, "command": command, "exit": result.returncode,
        "seconds": time.monotonic() - before,
        "stdout": stdout.name, "stderr": stderr.name,
        "stdout_sha256": sha(stdout), "stderr_sha256": sha(stderr),
    }
    rows.append(row)
    (OUT / "in-progress.json").write_text(json.dumps({"producer": PRODUCER, "rows": rows}, indent=2) + "\n")
    print("DONE", name, result.returncode, flush=True)
    assert result.returncode == 0, row
    return row


build = execute("build", ["cargo", "build", "--release", "-p", "dive-cli", "-p", "dive-conformance", "--locked"])
for name in ["dive", "dive-conformance"]:
    shutil.copy2(TARGET / "release" / name, BIN / name)

execute("library-controls", ["cargo", "test", "-p", "dive-store", "--lib", "seen::", "--locked"])
raw = (OUT / "library-controls.stderr").read_text()
matches = re.findall(r"Running unittests src/lib\.rs \(([^\n]+)\)", raw)
assert len(matches) == 1, raw
test_bin = Path(matches[0])
assert test_bin.resolve().is_relative_to(TARGET.resolve())
shutil.copy2(test_bin, BIN / "dive-store-controls")

execute("rust-guide", ["cargo", "run", "-p", "dive-store", "--example", "disk_seen", "--locked"])
assert (OUT / "rust-guide.stdout").read_text().splitlines() == [
    "4 exact keys, explicit owned cleanup complete",
    "4 exact keys, explicit owned cleanup complete",
]
shutil.copy2(TARGET / "debug/examples/disk_seen", BIN / "disk_seen")

env["HARNESS"] = str(BIN / "dive-conformance")
execute("cm1", ["bash", "scripts/conformance-gate.sh", "conformance/gates/cm1.txt", "--workers", "1,4,8", "--dive-bin", str(BIN / "dive")])
execute("validation-smoke", ["bash", "scripts/conformance-gate.sh", "conformance/gates/validation-smoke.txt", "--workers", "1,4,8", "--dive-bin", str(BIN / "dive")], {"DIVE_GATE_CORPUS": "validation-smoke"})

source = json.loads((ART / "seen-library-current-full-gate/source/index.json").read_text())
for name, row in source["inputs"].items():
    assert sha(ROOT / name) == row["sha256"], name
assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() == PRODUCER
producer = {
    "source_commit": PRODUCER,
    "source_index_sha256": sha(ART / "seen-library-current-full-gate/source/index.json"),
    "binaries_sha256": {name: sha(BIN / name) for name in ["dive", "dive-conformance", "dive-store-controls", "disk_seen"]},
}
(BIN / "producer.json").write_text(json.dumps(producer, indent=2) + "\n")
(OUT / "receipt.json").write_text(json.dumps({
    "runtime_producer": producer,
    "environment": REQ,
    "widths": [1, 4, 8],
    "gate_selection": {"cm1_models": 21, "validation_smoke_models": 16},
    "build": build,
    "rows": rows[1:],
    "driver_sha256": sha(Path(__file__)),
    "scope": "Library controls and unchanged engine gates; no native disk engine admission",
}, indent=2) + "\n")
print("CLOSED fresh library controls + Cargo guide +37 models at1/4/8", flush=True)
