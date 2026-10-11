"""Capture pure runtime-name recovery, precise store metadata and independent witness boundaries.

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
    "crates/dive-check/src/frontier.rs", "crates/dive-check/src/options.rs", "crates/dive-api/tests/native_checkpoint_spill.rs"]
index = {"schema_version": 1, "source_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip(), "source_parent_commit": "425474bc61210d3a178f5f373d332c72a01d8d1c",
    "native_binary_sha256": sha(binary), "capture_script_sha256": sha(Path(__file__)),
    "source_sha256": {path: sha(root/path) for path in source_paths},
    "family_certified": False, "controls": [], "archives": {}, "groups": 0}

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
    value.pop("frontier_storage", None)
    value.pop("predecessor_storage", None)
    for key in ("checkpoint", "workers", "profile", "frontier_memory_bytes_per_queue", "frontier_record_bytes_limit", "frontier_codec_nodes_per_value_limit", "predecessor_memory_bytes", "predecessor_path_steps_limit"):
        value["options"].pop(key, None)
    return value

def storage(value, family):
    if value.get("checkpoint") is not None:
        assert value["checkpoint"]["transient_storage_cleanup_completed"]
    if family in ("frontier", "both"):
        e = value["frontier_storage"]
        assert e["mode"] == "encoded-spill" and e["failures"] == []
        assert e["peak_retained_bytes"] == e["retained_bytes"] == 0
        assert e["committed_spill_bytes"] > 0
    if family in ("predecessor", "both"):
        e = value["predecessor_storage"]
        assert e["spilled"] and e["cleanup_completed"] and e["failures"] == []
        assert e["peak_retained_bytes"] == e["retained_bytes_before_cleanup"] == 0
        assert e["committed_records"] > 0
def metadata(value, store, profile):
    assert value["collision"] is None, "no completed collision estimate at an invariant/paused stop"
    assert value["options"]["store"] == ("exact" if store == "exact" else "fingerprint")
    expected = None if store == "exact" else {"polynomial_index":0,"polynomial_source":"flag","scheme": "tlc" if profile == "tlc" else "dive"}
    assert value["options"]["fingerprint"] == expected

def cross_store(value):
    value = observable(value)
    value["options"].pop("store")
    value["options"].pop("fingerprint")
    return value

references = {}
width_references = {}
semantic_references = {}
for case, family in ((case, family) for case in ("RuntimeStrings", "RuntimeChoose", "RuntimeSequence", "RuntimeSymView", "RuntimeDuplicates") for family in ("memory", "frontier", "predecessor", "both")):
    source = f"conformance/probes/native-checkpoint-runtime-names/specs/{case}/{case}.tla"
    config = source[:-4]+".cfg"
    for path in (source, config): index["source_sha256"][path] = sha(root/path)
    for profile in ("dive", "tlc"):
        for store in ("fp", "exact"):
            for width in (1, 4, 8):
                for chaos in (None, 19):
                    for boundary in (0, 1, 2):
                        stem = f"{case}-{family}-{profile}-{store}-w{width}-c{chaos}-b{boundary}"
                        archive = out/(stem+".cp")
                        args = ["check", source, "--config", config, "--seed", "7", "--fp", "0",
                            "--store", store, "--workers", str(width), "--output", "json"]
                        if profile == "tlc": args += ["--compat=tlc"]
                        if chaos is not None: args += ["--chaos", str(chaos)]
                        if family in ("frontier", "both"): args += ["--frontier-memory-bytes", "0"]
                        if family in ("predecessor", "both"): args += ["--predecessor-memory-bytes", "0"]
                        full = run(stem, "full", args)
                        metadata(full[1], store, profile)
                        wk = (case,profile,store)
                        expected_width = (full[0],observable(full[1]))
                        if wk in width_references: assert expected_width == width_references[wk], stem
                        else: width_references[wk] = expected_width
                        sk = (case,profile)
                        expected_semantic = (full[0],cross_store(full[1]))
                        if sk in semantic_references: assert expected_semantic == semantic_references[sk], stem
                        else: semantic_references[sk] = expected_semantic
                        ref = (case, profile, store, width, chaos, boundary)
                        if family == "memory": references[ref] = (full[0], observable(full[1]))
                        else: assert (full[0], observable(full[1])) == references[ref], stem
                        pause = run(stem, "pause", args+["--checkpoint", str(archive),
                            "--checkpoint-after-levels", str(boundary), "--checkpoint-counters", "retain"])
                        recovery_args = list(args)
                        resumed_width = 4 if width == 8 else 8
                        recovery_args[recovery_args.index("--workers")+1] = str(resumed_width)
                        retained = run(stem, "retain", recovery_args+["--recover", str(archive), "--checkpoint-counters", "retain"])
                        reset = run(stem, "tlc-reset", recovery_args+["--recover", str(archive)])
                        assert full[1]["verdict"] == "invariant-violation", stem
                        assert pause[1]["verdict"] == "resource-limit" and pause[1]["counterexample"] is None, stem
                        assert pause[1]["checkpoint"]["status"] == "paused-incomplete", stem
                        assert (retained[0], observable(retained[1])) == (full[0], observable(full[1])), stem
                        assert retained[1]["checkpoint"]["status"] == "recovered", stem
                        for value, actual, requested in ((full[1], 1 if profile == "tlc" else width, width), (pause[1], 1 if profile == "tlc" or boundary == 0 else width, width), (retained[1], 1 if profile == "tlc" else resumed_width, resumed_width), (reset[1], 1 if profile == "tlc" else resumed_width, resumed_width)):
                            assert value["options"]["workers"]["used"] == actual, stem
                            assert value["options"]["checkpoint"]["parallel_recovery"] is True, stem
                            assert value["options"]["workers"]["requested"] == requested, stem
                            assert value["options"]["checkpoint"]["independent_certificate"] is False, stem
                        assert reset[0] == full[0] and reset[1]["counterexample"] == full[1]["counterexample"], stem
                        for key in ("distinct", "depth", "left_on_queue"):
                            assert reset[1]["statistics"][key] == full[1]["statistics"][key], stem
                        assert reset[1]["statistics"]["generated"] == full[1]["statistics"]["generated"] - pause[1]["statistics"]["generated"] + pause[1]["statistics"]["distinct"], stem
                        index["archives"][archive.name] = sha(archive)
                        for value in (full[1], pause[1], retained[1], reset[1]):
                            metadata(value,store,profile)
                            storage(value, family)
                        trace = retained[1]["counterexample"]["trace"]
                        if case == "RuntimeSymView":
                            assert all(state["values"]["owner"] == {"model": "p2"} for state in trace["states"]), stem
                        trace_path = out/(stem+".trace.json")
                        trace_path.write_text(json.dumps(trace, indent=2)+"\n")
                        val_args = ["validate-trace", source, "--config", config, "--trace", str(trace_path),
                            "--seed", "7", "--fp", "0", "--output", "json"]
                        if profile == "tlc": val_args += ["--compat=tlc"]
                        positive = run(stem, "witness-valid", val_args)
                        if profile == "tlc":
                            assert positive[1]["result"] == "unsupported", (stem,positive)
                            assert positive[1]["unsupported"] == "runtime string encounter order: the independent validator cannot establish names created outside the supplied trace (#124)", (stem,positive)
                        else:
                            assert positive[0] == 0 and positive[1]["result"] == "valid", (stem,positive)
                            bad = copy.deepcopy(trace)
                            bad["states"][0]["values"]["n"] = 99
                            bad_path = out/(stem+".mutant-init.trace.json")
                            bad_path.write_text(json.dumps(bad, indent=2)+"\n")
                            bad_args = list(val_args)
                            bad_args[bad_args.index("--trace")+1] = str(bad_path)
                            negative = run(stem, "witness-bad-init", bad_args)
                            assert negative[0] == 1 and negative[1]["result"] == "invalid", (stem,negative)
                            assert negative[1]["failure"]["check"] == "init", (stem,negative)
                        # Keep the exact valid input beside raw validation outputs.
                        trace_path.write_text(json.dumps(trace, indent=2)+"\n")
                        index["groups"] += 1
                        save()
                        print(stem, "agree", flush=True)
assert index["groups"] == 1440 and len(index["controls"]) == 7920
save()
