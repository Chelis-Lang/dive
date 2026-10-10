"""Capture native finite safety full/pause/actual recovery controls.

Usage: python3 -I capture_controls.py PATH_TO_DIVE FRESH_OUTPUT_DIRECTORY
This never changes oracle goldens. Run with DIVE_COMPAT/DIVE_CHAOS unset.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parents[3]
binary = pathlib.Path(sys.argv[1]).resolve()
out = pathlib.Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=False)
run_environment = os.environ.copy()
for key in ("DIVE_COMPAT", "DIVE_CHAOS"):
    run_environment.pop(key, None)
index = {"schema_version": 1, "parent_commit": "69c672ac11085dea75bc2eedca086cdf7d28d2f7",
         "native_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "controls": []}
for path in ["crates/dive-api/src/check.rs", "crates/dive-api/src/config.rs",
             "crates/dive-check/src/checkpoint.rs", "crates/dive-check/src/engine.rs",
             "crates/dive-check/src/lib.rs", "crates/dive-cli/src/main.rs",
             "crates/dive-model/src/lib.rs", "crates/dive-value/src/codec.rs"]:
    index.setdefault("source_sha256", {})[path] = hashlib.sha256((root/path).read_bytes()).hexdigest()

def observable(value):
    value = json.loads(json.dumps(value))
    value.pop("elapsed_ms", None)
    value.pop("checkpoint", None)
    for key in ("checkpoint", "workers", "profile"):
        value["options"].pop(key, None)
    return value

for case in ("DuplicateSuccessors", "MidLevelViolation", "DeadlockMidLevel",
             "EvalErrorMidLevel", "ConstrainedViolation", "ActionConstraintNoDeadlock"):
    source = f"crates/dive-check/tests/fixtures/tlc-probes/specs/{case}/{case}.tla"
    for profile in ("dive", "tlc"):
        for store in ("fp", "exact"):
            for width in (1, 4, 8):
                for chaos in (None, 13):
                    stem = f"{case}-{profile}-{store}-w{width}-c{chaos}"
                    archive = out / f"{stem}.cp"
                    args = ["check", source, "--seed", "7", "--fp", "0", "--store", store,
                            "--workers", str(width), "--output", "json"]
                    if profile == "tlc": args += ["--compat=tlc"]
                    if chaos is not None: args += ["--chaos", str(chaos)]
                    results = {}
                    for stage, extra in (("full", []),
                        ("pause", ["--checkpoint", str(archive), "--checkpoint-after-levels", "0", "--checkpoint-counters", "retain"]),
                        ("retain", ["--recover", str(archive), "--checkpoint-counters", "retain"]),
                        ("tlc-reset", ["--recover", str(archive)])):
                        run = subprocess.run([str(binary), *args, *extra], cwd=root, capture_output=True, timeout=60, env=run_environment)
                        leaf = f"{stem}-{stage}"
                        (out / f"{leaf}.stdout.json").write_bytes(run.stdout)
                        (out / f"{leaf}.stderr").write_bytes(run.stderr)
                        value = json.loads(run.stdout)
                        results[stage] = (run.returncode, value)
                        entry = {"case": case, "profile": profile, "store": store, "width": width,
                                 "chaos": chaos, "stage": stage, "args": args + extra, "exit": run.returncode,
                                 "stdout": f"{leaf}.stdout.json", "stderr": f"{leaf}.stderr",
                                 "stdout_sha256": hashlib.sha256(run.stdout).hexdigest(),
                                 "stderr_sha256": hashlib.sha256(run.stderr).hexdigest()}
                        index["controls"].append(entry)
                    full, pause, retained, reset = (results[key] for key in ("full", "pause", "retain", "tlc-reset"))
                    assert pause[1]["verdict"] == "resource-limit", stem
                    assert pause[1]["checkpoint"]["status"] == "paused-incomplete", stem
                    assert pause[1]["counterexample"] is None, stem
                    assert (retained[0], observable(retained[1])) == (full[0], observable(full[1])), stem
                    assert retained[1]["checkpoint"]["status"] == "recovered", stem
                    assert retained[1]["options"]["workers"]["used"] == 1, stem
                    assert retained[1]["options"]["workers"]["requested"] == width, stem
                    assert retained[1]["options"]["checkpoint"]["independent_certificate"] is False, stem
                    assert reset[0] == full[0] and reset[1]["verdict"] == full[1]["verdict"], stem
                    assert reset[1]["statistics"]["distinct"] == full[1]["statistics"]["distinct"], stem
                    expected = full[1]["statistics"]["generated"] - pause[1]["statistics"]["generated"] + pause[1]["statistics"]["distinct"]
                    assert reset[1]["statistics"]["generated"] == expected, stem
                    index.setdefault("archive_sha256", {})[archive.name] = hashlib.sha256(archive.read_bytes()).hexdigest()
                    print(stem, "agree", flush=True)
(out / "index.json").write_text(json.dumps(index, indent=2) + "\n")
