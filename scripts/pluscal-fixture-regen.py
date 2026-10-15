#!/usr/bin/env python3
"""Regenerate missing expected modules and label documents from pinned pcal.trans.

Usage: python3 -I scripts/pluscal-fixture-regen.py crates/dive-pluscal/tests/fixtures
Only normalization, fairness, process and multiple tests read label documents.
Other families retain translated modules only. Before committing new fixtures,
verify that each output is read by a passing test and is at most 100 KB.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
CACHE = Path(os.environ.get("DIVE_CACHE_DIR", Path.home() / ".cache/dive"))
PINS = tomllib.loads((ROOT / "conformance/oracles.toml").read_text())
ORACLE = next(pin for pin in PINS["tlc"] if pin.get("default"))
JAR = CACHE / "sha256" / ORACLE["sha256"] / "tla2tools.jar"
HOST = {"Linux": "linux", "Darwin": "macos"}[platform.system()]
ARCH = {"x86_64": "x86_64", "aarch64": "aarch64", "arm64": "aarch64"}[platform.machine()]
JRE = CACHE / "jre" / PINS["jre"]["platforms"][f"{HOST}-{ARCH}"]["sha256"]
JAVA = next(iter(sorted(JRE.glob("**/bin/java"))), None)
LABEL_READERS = {"normalization", "fairness", "process", "multiple"}


def run(source, options, report):
    with tempfile.TemporaryDirectory() as scratch:
        name = source.name
        shutil.copy(source, Path(scratch) / name)
        extra = ["-reportLabels"] if report else []
        result = subprocess.run(
            [str(JAVA), "-XX:+UseSerialGC", "-cp", str(JAR), "pcal.trans", "-nocfg"]
            + extra + options + [name], cwd=scratch, capture_output=True, text=True, timeout=60,
        )
        text = (Path(scratch) / name).read_text() if result.returncode == 0 else None
        return result, text


def write(path, text):
    if len(text.encode("utf-8")) > 100_000:
        raise ValueError(f"fixture exceeds 100 KB: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


def main(root):
    if JAVA is None:
        raise ValueError("missing pinned JRE; run the harness oracle fetch")
    if hashlib.sha256(JAR.read_bytes()).hexdigest() != ORACLE["sha256"]:
        raise ValueError("pinned oracle SHA-256 mismatch")
    made = 0
    failed = 0
    for manifest in sorted(root.glob("**/manifest.json")):
        base = manifest.parent
        family = manifest.relative_to(root).parts[0]
        for row in json.loads(manifest.read_text()).get("rows", []):
            if row.get("primary", {}).get("status") != "translated":
                continue
            source = base.parent / "cases" / row["input"]
            if not source.exists():
                source = base / row["input"]
            translated = row.get("translated", {})
            module = base / translated["path"] if translated.get("path") else None
            normalized = row.get("normalized_reference_adapter", {}).get("document", {})
            document = base / normalized["path"] if normalized.get("path") and family in LABEL_READERS else None
            need_module = module is not None and not module.exists()
            need_document = document is not None and not document.exists()
            if not need_module and not need_document:
                continue
            result, text = run(source, row.get("options", []), need_document)
            if text is None:
                print(f"oracle rejected {row['id']}: {result.stdout}\n{result.stderr}", file=sys.stderr)
                failed += 1
                continue
            if need_module:
                write(module, text)
                made += 1
            if need_document:
                labels = re.findall(r"^  (\S+) at (line \d+, column \d+.*)$", result.stdout, re.M)
                body = text[text.find("BEGIN TRANSLATION"):]
                write(document, json.dumps({
                    "inserted": [label[0] for label in labels],
                    "locations": [label[1] for label in labels],
                    "default_initialization": "CONSTANT defaultInitValue" in body,
                }, indent=1) + "\n")
                made += 1
    print(f"generated {made}; oracle failures {failed}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(Path(sys.argv[1])))
