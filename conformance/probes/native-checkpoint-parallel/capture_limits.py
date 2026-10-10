"""Compare native counter stopping prefixes with the frozen inline binary.

Usage: python3 -I capture_limits.py INLINE_DIVE CURRENT_DIVE FRESH_OUT
Coherent-checksum edits are negative native-import controls, not oracle goldens.
Each executable creates its own matching archive. No Java archive is imported.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
old, current = [Path(p).resolve() for p in sys.argv[1:3]]
out = Path(sys.argv[3]).resolve()
out.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
for key in ("DIVE_COMPAT", "DIVE_CHAOS"):
    env.pop(key, None)
sha = lambda data: hashlib.sha256(data).hexdigest()
source = "conformance/probes/native-checkpoint-parallel/specs/CpParallel/CpParallel.tla"
config = source[:-4] + ".cfg"
index = {"schema_version": 1, "family_certified": False,
    "source_parent_commit": "54999da336d093ce6da52b4639879933b1b171a0",
    "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(),
    "binary_sha256": {"inline": sha(old.read_bytes()), "current": sha(current.read_bytes())},
    "source_sha256": {p: sha((root/p).read_bytes()) for p in (source, config,
        "crates/dive-check/src/checkpoint.rs", "crates/dive-check/src/engine.rs",
        "crates/dive-check/src/parallel.rs", "crates/dive-check/tests/native_checkpoint.rs")},
    "capture_script_sha256": sha(Path(__file__).read_bytes()), "controls": [], "archives": {}}
def save():
    (out/"index.json").write_text(json.dumps(index, indent=2)+"\n")
def run(binary, stem, args):
    r = subprocess.run([str(binary), *args], cwd=root, env=env, capture_output=True, timeout=45)
    (out/(stem+".stdout.json")).write_bytes(r.stdout)
    (out/(stem+".stderr")).write_bytes(r.stderr)
    index["controls"].append({"id": stem, "args": args, "exit": r.returncode,
        "stdout": stem+".stdout.json", "stdout_sha256": sha(r.stdout),
        "stderr": stem+".stderr", "stderr_sha256": sha(r.stderr)})
    save()
    return r.returncode, json.loads(r.stdout)
def observable(v):
    v = copy.deepcopy(v)
    v.pop("elapsed_ms", None)
    v.pop("checkpoint", None)
    for key in ("checkpoint", "workers", "profile"):
        v["options"].pop(key, None)
    return v
maximum = (1 << 63) - 1
for profile in ("dive", "tlc"):
    for store in ("fp", "exact"):
        base = ["check", source, "--config", config, "--store", store,
            "--seed", "7", "--fp", "0", "--output", "json"]
        if profile == "tlc": base += ["--compat=tlc"]
        snapshots = {}
        for label, binary in (("inline", old), ("current", current)):
            stem = f"{profile}-{store}-{label}-pause"
            path = out/(stem+".cp")
            paused = run(binary, stem, base+["--workers", "8", "--checkpoint", str(path),
                "--checkpoint-after-levels", "1", "--checkpoint-counters", "retain"])
            assert paused[1]["checkpoint"]["status"] == "paused-incomplete", stem
            assert paused[1]["options"]["workers"]["used"] == (1 if label == "inline" else 8), stem
            index["archives"][path.name] = sha(path.read_bytes())
            snapshots[label] = json.loads(path.read_bytes()[48:])
        for remaining in (0, 1, 2, 3, 5, 7):
            mutated = {}
            for label in snapshots:
                value = copy.deepcopy(snapshots[label])
                value["generated"] = maximum - remaining
                payload = json.dumps(value, separators=(",", ":"), ensure_ascii=False).encode()
                data = b"DIVECP01" + len(payload).to_bytes(8, "little") + hashlib.sha256(payload).digest() + payload
                path = out/f"{profile}-{store}-{label}-remaining{remaining}.cp"
                path.write_bytes(data)
                index["archives"][path.name] = sha(data)
                mutated[label] = path
            args = ["--recover", str(mutated["inline"]), "--checkpoint-counters", "retain"]
            reference = run(old, f"{profile}-{store}-r{remaining}-inline", base+["--workers", "1", *args])
            assert reference[1]["verdict"] == "resource-limit", reference
            assert reference[1]["statistics"]["generated"] == maximum, reference
            assert reference[1]["counterexample"] is not None, reference
            for width in (1, 4, 8):
                for chaos in (None, 19):
                    stem = f"{profile}-{store}-r{remaining}-w{width}-c{chaos}"
                    args = base+["--workers", str(width), "--recover", str(mutated["current"]),
                        "--checkpoint-counters", "retain"]
                    if chaos is not None: args += ["--chaos", str(chaos)]
                    resumed = run(current, stem, args)
                    assert (resumed[0], observable(resumed[1])) == (reference[0], observable(reference[1])), stem
                    assert resumed[1]["options"]["workers"]["used"] == width, stem
                    assert resumed[1]["checkpoint"]["status"] == "recovered", stem
                    print(stem, "inline-prefix-agrees", flush=True)
assert len(index["controls"]) == 176 and len(index["archives"]) == 56
save()
