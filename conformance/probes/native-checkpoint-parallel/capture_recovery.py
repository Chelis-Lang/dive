"""Capture ordered parallel configured-key recovery and independent concrete witnesses.

Usage: python3 -I capture_recovery.py DIVE FRESH_OUTPUT
Goldens and primary captures are unchanged. Every invocation records raw output.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[3]
binary = Path(sys.argv[1]).resolve()
out = Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=False)
env = os.environ.copy()
for key in ("DIVE_COMPAT", "DIVE_CHAOS"):
    env.pop(key, None)

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

source_paths = ["crates/dive-check/src/checkpoint.rs", "crates/dive-check/src/engine.rs", "crates/dive-check/src/parallel.rs", "crates/dive-check/tests/native_checkpoint.rs",
    "crates/dive-check/src/lib.rs", "crates/dive-model/src/symmetry/prepare.rs",
    "crates/dive-api/src/check.rs", "crates/dive-api/src/trace.rs",
    "crates/dive-cli/src/main.rs", "crates/dive-model/src/lib.rs",
    "crates/dive-model/src/bind.rs", "crates/dive-model/src/tables.rs",
    "crates/dive-trace/src/validate.rs", "crates/dive-value/src/codec.rs",
    "crates/dive-api/tests/native_checkpoint_keys.rs", "crates/dive-check/tests/checkpoint_keys.rs",
    "crates/dive-api/tests/native_checkpoint.rs", "crates/dive-check/tests/public_ir.rs",
    "crates/dive-check/src/frontier.rs", "crates/dive-check/src/options.rs"]
index = {"schema_version": 1, "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(), "source_parent_commit": "54999da336d093ce6da52b4639879933b1b171a0",
    "native_binary_sha256": sha(binary), "capture_script_sha256": sha(Path(__file__)),
    "source_sha256": {path: sha(root/path) for path in source_paths},
    "controls": [], "archives": {}, "groups": 0}

def save():
    (out/"index.json").write_text(json.dumps(index, indent=2)+"\n")

def run(stem, stage, args):
    result = subprocess.run([str(binary), *args], cwd=root, env=env, capture_output=True, timeout=45)
    leaf = f"{stem}-{stage}"
    (out/(leaf+".stdout.json")).write_bytes(result.stdout)
    (out/(leaf+".stderr")).write_bytes(result.stderr)
    entry = {"stem": stem, "stage": stage, "args": args,
        "exit": result.returncode, "stdout": leaf+".stdout.json", "stderr": leaf+".stderr",
        "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
        "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()}
    if args[0] == "validate-trace":
        trace_input = Path(args[args.index("--trace")+1])
        entry["trace_input"] = trace_input.name
        entry["trace_input_sha256"] = sha(trace_input)
    index["controls"].append(entry)
    save()
    return result.returncode, json.loads(result.stdout)

def observable(value):
    value = copy.deepcopy(value)
    value.pop("elapsed_ms", None)
    value.pop("checkpoint", None)
    for key in ("checkpoint", "workers", "profile"):
        value["options"].pop(key, None)
    return value

for case in ("CpView", "CpSymView", "CpSymConcrete"):
    source = f"conformance/probes/native-checkpoint-keys/specs/{case}/{case}.tla"
    config = source[:-4]+".cfg"
    for path in (source, config): index["source_sha256"][path] = sha(root/path)
    for profile in ("dive", "tlc"):
        for store in ("fp", "exact"):
            for width in (1, 4, 8):
                for chaos in (None, 19):
                    for boundary in (0, 1):
                        stem = f"{case}-{profile}-{store}-w{width}-c{chaos}-b{boundary}"
                        archive = out/(stem+".cp")
                        args = ["check", source, "--config", config, "--seed", "7", "--fp", "0",
                            "--store", store, "--workers", str(width), "--output", "json"]
                        if profile == "tlc": args += ["--compat=tlc"]
                        if chaos is not None: args += ["--chaos", str(chaos)]
                        full = run(stem, "full", args)
                        pause = run(stem, "pause", args+["--checkpoint", str(archive),
                            "--checkpoint-after-levels", str(boundary), "--checkpoint-counters", "retain"])
                        retained = run(stem, "retain", args+["--recover", str(archive), "--checkpoint-counters", "retain"])
                        reset = run(stem, "tlc-reset", args+["--recover", str(archive)])
                        assert full[1]["verdict"] == "invariant-violation", stem
                        assert pause[1]["verdict"] == "resource-limit" and pause[1]["counterexample"] is None, stem
                        assert pause[1]["checkpoint"]["status"] == "paused-incomplete", stem
                        assert (retained[0], observable(retained[1])) == (full[0], observable(full[1])), stem
                        assert retained[1]["checkpoint"]["status"] == "recovered", stem
                        for value, actual in ((pause[1], 1 if boundary == 0 else width), (retained[1], width), (reset[1], width)):
                            assert value["options"]["workers"]["used"] == actual, stem
                            assert value["options"]["checkpoint"]["parallel_recovery"] is True, stem
                            assert value["options"]["workers"]["requested"] == width, stem
                            assert value["options"]["checkpoint"]["independent_certificate"] is False, stem
                        assert reset[0] == full[0] and reset[1]["counterexample"] == full[1]["counterexample"], stem
                        for key in ("distinct", "depth", "left_on_queue"):
                            assert reset[1]["statistics"][key] == full[1]["statistics"][key], stem
                        assert reset[1]["statistics"]["generated"] == full[1]["statistics"]["generated"] - pause[1]["statistics"]["generated"] + pause[1]["statistics"]["distinct"], stem
                        index["archives"][archive.name] = sha(archive)
                        trace = retained[1]["counterexample"]["trace"]
                        if case == "CpSymConcrete":
                            assert all(state["values"]["owner"] == {"model": "p2"} for state in trace["states"]), stem
                        trace_path = out/(stem+".trace.json")
                        trace_path.write_text(json.dumps(trace, indent=2)+"\n")
                        val_args = ["validate-trace", source, "--config", config, "--trace", str(trace_path),
                            "--seed", "7", "--fp", "0", "--output", "json"]
                        if profile == "tlc": val_args += ["--compat=tlc"]
                        positive = run(stem, "witness-valid", val_args)
                        assert positive[0] == 0 and positive[1]["result"] == "valid", stem
                        if case != "CpSymView":
                            for at in (0, 1, len(trace["states"])-1):
                                bad = copy.deepcopy(trace)
                                key, value = ("n", 99) if case == "CpView" else ("owner", {"model": "p1"})
                                bad["states"][at]["values"][key] = value
                                bad_path = out/(stem+f".mutant-{at}.trace.json")
                                bad_path.write_text(json.dumps(bad, indent=2)+"\n")
                                bad_args = list(val_args)
                                bad_args[bad_args.index("--trace")+1] = str(bad_path)
                                negative = run(stem, f"witness-equivalent-bad-{at}", bad_args)
                                assert negative[0] == 1 and negative[1]["result"] == "invalid", stem
                                assert negative[1]["failure"]["check"] == ("init" if at == 0 else "next"), stem
                        # Keep the exact valid input beside raw validation outputs.
                        trace_path.write_text(json.dumps(trace, indent=2)+"\n")
                        index["groups"] += 1
                        save()
                        print(stem, "agree", flush=True)
assert index["groups"] == 144 and len(index["controls"]) == 1008
save()
