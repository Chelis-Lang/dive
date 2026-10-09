#!/usr/bin/env python3
"""Generate a pinned, deliberately bounded ValidationTestSuite corpus.

Run with the pinned Python environment described in docs/conformance-corpora.md.
The generator executes in a disposable copy; no oracle or reference checker is
executed by this script. Oracle results must come from dive-conformance.
"""

import argparse
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tempfile

COMMIT = "fe99cff1ca48535bdbfc99fd95bd2706f056c12f"
SOURCE = "https://github.com/tlaplus/ValidationTestSuite"
CASES = ("Eq", "Let", "InstanceNamedWith", "Enabled", "BoolTrue")
PLUGS = ("BoolTrue", "BoolFalse")
PACKAGES = {"Jinja2": "3.1.2", "PyYAML": "6.0.3", "MarkupSafe": "2.1.5"}


def dump(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def relative(value):
    """Reject absolute paths, traversal, and platform-dependent path syntax."""
    if not isinstance(value, str) or "\\" in value:
        raise ValueError("invalid source path")
    path = PurePosixPath(value)
    if path.is_absolute() or ".." in path.parts or ":" in value:
        raise ValueError("source path escapes corpus")
    return path


def verified_file(root, value, digest):
    path = root / relative(value)
    if path.is_symlink() or not path.is_file() or root.resolve() not in path.resolve().parents:
        raise ValueError("missing or escaping generated file")
    content = path.read_bytes()
    if hashlib.sha1(content).hexdigest() != digest:
        raise ValueError("generated file digest mismatch")
    return content


def import_cases(spec_root, metadata, destination):
    """Account for every selected case; admit only pinned single-worker TLC sides."""
    if set(metadata) != {"workers_options", "testcases"} or metadata["workers_options"] != ["1", "2"]:
        raise ValueError("unexpected specification schema or worker options")
    cases = metadata["testcases"]
    if not isinstance(cases, dict) or not cases:
        raise ValueError("empty specification")
    census = []
    for case_id, case in sorted(cases.items()):
        if len(case_id) != 40 or any(c not in "0123456789abcdef" for c in case_id) or case["id"] != case_id:
            raise ValueError("invalid case identity")
        row = {"id": case_id, "description": case["desc"], "type": case["type"]}
        if case["type"] == "skipped":
            row.update(status="upstream-skipped", reason=case["reason"])
        else:
            if case["type"] not in {"ref-tlc", "ref-apalache"}:
                raise ValueError("unhandled testcase type")
            model = case["tlc"]
            options = model["cmd_options"]
            if options not in (["-workers", "1"], ["-workers", "2"]):
                raise ValueError("unhandled TLC options")
            row["tlc_options"] = options
            row["reference_status"] = "not-executed"
            if options == ["-workers", "2"]:
                row.update(status="out-of-scope", reason="alternate-worker reference comparison; single-worker TLC sides only")
            else:
                if model["search_paths"]:
                    raise ValueError("module search-path adaptation is not implemented")
                folder = destination / "specifications" / case_id
                folder.mkdir(parents=True)
                source_folder = spec_root / relative(model["path"])
                if spec_root.resolve() not in source_folder.resolve().parents:
                    raise ValueError("generated model escapes specification root")
                modules = []
                names = set()
                for module in model["modules"]:
                    name = module["name"]
                    if not name.isidentifier() or name in names or module["path"]:
                        raise ValueError("unsupported or duplicate module path")
                    names.add(name)
                    filename = name + ".tla"
                    content = verified_file(source_folder, filename, module["digest"])
                    (folder / filename).write_bytes(content)
                    modules.append({"path": "specifications/" + case_id + "/" + filename,
                                    "features": [], "models": []})
                main = model["main_module"]
                cfg = model["cfg"]
                if main not in names or cfg["file"] != main + ".cfg":
                    raise ValueError("configuration does not match main module")
                content = verified_file(source_folder, cfg["file"], cfg["digest"])
                (folder / cfg["file"]).write_bytes(content)
                config_path = "specifications/" + case_id + "/" + cfg["file"]
                next(m for m in modules if m["path"].endswith("/" + main + ".tla"))["models"] = [{
                    "path": config_path, "runtime": "unknown", "mode": "exhaustive search", "result": "unknown"}]
                dump(folder / "manifest.json", {"sources": [SOURCE + "/tree/" + COMMIT],
                                               "authors": ["NVIDIA CORPORATION & AFFILIATES"],
                                               "tags": ["validation-test-suite", "generated-smoke"], "modules": modules})
                row.update(status="imported", model=config_path)
        census.append(row)
    counts = {status: sum(row["status"] == status for row in census)
              for status in ("imported", "upstream-skipped", "out-of-scope")}
    if not counts["imported"] or sum(counts.values()) != len(cases):
        raise ValueError("incomplete or empty import census")
    return {"schema_version": 1, "counts": counts, "cases": census}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.out.absolute()
    if destination.exists():
        raise ValueError("destination exists; generate a new corpus and review the diff")
    if os.environ.get("PYTHONHASHSEED") != "0":
        raise ValueError("generation requires PYTHONHASHSEED=0")
    if sys.version_info[:2] != (3, 9):
        raise ValueError("generation requires Python 3.9, as used by this upstream pin")
    for package, version in PACKAGES.items():
        if importlib.metadata.version(package) != version:
            raise ValueError("unpinned generator dependency: " + package)
    actual = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    dirty = subprocess.check_output(["git", "-C", str(source), "status", "--porcelain", "--untracked-files=all"], text=True)
    if actual != COMMIT or dirty:
        raise ValueError("source must be a clean checkout at the pinned commit")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="dive-validation-") as scratch:
        scratch = Path(scratch)
        copy = scratch / "source"
        shutil.copytree(source, copy, ignore=shutil.ignore_patterns(".git", "tools", "__pycache__"))
        sys.dont_write_bytecode = True
        sys.path.insert(0, str(copy))
        from testgen.spec import render_spec, set_workers_options
        set_workers_options(["1", "2"])
        spec = scratch / "specification"
        spec.mkdir()
        render_spec(str(spec), str(spec / "specification.json"),
                    lambda case, plug: case in CASES and plug in PLUGS, symmetry=False, anomalous=False)
        metadata = json.loads((spec / "specification.json").read_text())
        with tempfile.TemporaryDirectory(prefix=".validation-import-", dir=destination.parent) as staged:
            staged = Path(staged)
            census = import_cases(spec, metadata, staged)
            dump(staged / "selection.json", census)
            dump(staged / "specification.json", metadata)
            dump(staged / "provenance.json", {"schema_version": 1, "repo": SOURCE, "commit": COMMIT,
                 "case_features": CASES, "plug_features": PLUGS, "symmetry": False, "anomalous": False,
                 "workers_options": ["1", "2"], "python": "3.9", "dependencies": PACKAGES,
                 "oracle_results": "generated separately by dive-conformance; no result assumed",
                 "reference_checkers": "not executed", "license": "Apache-2.0"})
            shutil.copyfile(copy / "LICENSE.md", staged / "LICENSE.md")
            (staged / "NOTICE").write_text("Generated from tlaplus/ValidationTestSuite at " + COMMIT + "\n"
                "Copyright (c) 2022 NVIDIA CORPORATION & AFFILIATES. All rights reserved.\n"
                "Generated TLA+ and configuration files are preserved; dive adds manifests and a selection census.\n")
            staged.rename(destination)
        print(json.dumps(census["counts"], sort_keys=True))


if __name__ == "__main__":
    main()
