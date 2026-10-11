"""Capture pre-effect and actual transient-storage failures for native recovery.

Run from the repository root: capture_failures.py DIVE FRESH_OUTPUT.
Every failure remains incomplete; archive checksums are corruption checks.
"""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd().resolve()
binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
for key in ("DIVE_COMPAT", "DIVE_CHAOS"):
    env.pop(key, None)
sha = lambda data: hashlib.sha256(data).hexdigest()
index = {"schema_version": 1, "family_certified": False,
    "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
    "native_binary_sha256": sha(binary.read_bytes()), "capture_script_sha256": sha(Path(__file__).read_bytes()),
    "controls": [], "archives": {}}
def save():
    (out/"index.json").write_text(json.dumps(index, indent=2)+"\n")
def run(stem, args):
    r = subprocess.run([str(binary), *args], cwd=root, env=env, capture_output=True, timeout=45)
    (out/(stem+".stdout.json")).write_bytes(r.stdout)
    (out/(stem+".stderr")).write_bytes(r.stderr)
    index["controls"].append({"id": stem, "args": args, "exit": r.returncode,
        "stdout": stem+".stdout.json", "stdout_sha256": sha(r.stdout),
        "stderr": stem+".stderr", "stderr_sha256": sha(r.stderr)})
    save()
    return r.returncode, json.loads(r.stdout) if r.stdout else None, r.stderr
source = "conformance/probes/native-checkpoint-keys/specs/CpSymView/CpSymView.tla"
config = source[:-4]+".cfg"
index["source_sha256"] = {p: sha((root/p).read_bytes()) for p in (source, config,
    "crates/dive-check/src/checkpoint.rs", "crates/dive-check/src/engine.rs",
    "crates/dive-api/src/check.rs", "crates/dive-api/tests/native_checkpoint_spill.rs")}
blocked = out/"blocked"
blocked.write_bytes(b"not a directory")
for profile in ("dive", "tlc"):
    for store in ("fp", "exact"):
        stem = f"{profile}-{store}"
        base = ["check", source, "--config", config, "--seed", "7", "--fp", "0", "--store", store, "--output", "json"]
        if profile == "tlc": base += ["--compat=tlc"]
        archive = out/(stem+".cp")
        good = run(stem+"-good-pause", base+["--workers", "4", "--checkpoint", str(archive), "--checkpoint-after-levels", "1"])
        assert good[1]["checkpoint"]["status"] == "paused-incomplete"
        assert good[1]["checkpoint"]["transient_storage_cleanup_completed"]
        saved = archive.read_bytes()
        index["archives"][archive.name] = sha(saved)
        for width in (1, 4, 8):
            for family in ("frontier", "predecessor", "both"):
                flags = []
                if family in ("frontier", "both"):
                    flags += ["--frontier-memory-bytes", "0", "--frontier-directory", str(blocked)]
                if family in ("predecessor", "both"):
                    flags += ["--predecessor-memory-bytes", "0", "--predecessor-directory", str(blocked)]
                fresh = out/(stem+f"-w{width}-{family}-failed.cp")
                for stage, native in (("pause", ["--checkpoint", str(fresh), "--checkpoint-after-levels", "1"]), ("recover", ["--recover", str(archive)])):
                    result = run(stem+f"-w{width}-{family}-{stage}", base+["--workers", str(width), *flags, *native])
                    value = result[1]
                    assert value["verdict"] == "resource-limit" and value["checkpoint"] is None
                    assert value["counterexample"] is None and value["collision"] is None
                    assert value["output"] == [] and value["options"]["workers"]["used"] == 1
                    assert value["frontier_storage"]["failures"] or value.get("predecessor_storage", {}).get("failures")
                assert not fresh.exists()
                assert archive.read_bytes() == saved
            corrupt = out/(stem+f"-w{width}-corrupt.cp")
            damaged = bytearray(saved);damaged[-1] ^= 1;corrupt.write_bytes(damaged)
            index["archives"][corrupt.name] = sha(damaged)
            bad = run(stem+f"-w{width}-corrupt", base+["--workers", str(width), "--frontier-memory-bytes", "0", "--predecessor-memory-bytes", "0", "--recover", str(corrupt)])
            assert bad[0] != 0 and bad[1] is None and b"DIVE-S0400" in bad[2]
        for family in ("frontier", "predecessor"):
            args = ["check", "missing-model.tla", "--"+family+"-directory", str(blocked), "--checkpoint", str(out/(stem+"-orphan.cp")), "--checkpoint-after-levels", "0", "--output", "json"]
            bad = run(stem+f"-{family}-orphan", args)
            assert bad[0] != 0 and bad[1] is None and b"requires" in bad[2]
        assert archive.read_bytes() == saved
assert len(index["controls"]) == 96 and len(index["archives"]) == 16
save()
print("96 pause/recovery/corruption/orphan-directory controls passed", flush=True)
