#!/usr/bin/env python3
"""Emit an isolated Rust BFS from the existing finite relation compiler."""

import argparse
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "scripts/symbolic"))
from engine import SourceModel, Unsupported  # noqa: E402
from packed_codegen import Rust  # noqa: E402


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exporter", type=Path)
    parser.add_argument("module", type=Path)
    parser.add_argument("config", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--type-invariant", default="TypeInvariant")
    args = parser.parse_args()
    bound = subprocess.run([args.exporter, args.module, args.config], check=False, capture_output=True)
    if bound.returncode != 0:
        parser.exit(2, f"Binder exited {bound.returncode}: {bound.stderr.decode(errors='replace')}")
    doc = json.loads(bound.stdout)
    try:
        rust = Rust(SourceModel(doc, args.type_invariant, state_bit_limit=32))
        worker = rust.emit(Path(__file__).with_name("finite_worker.rs.in").read_text())
    except Unsupported as error:
        parser.exit(2, f"Unsupported: {error}\n")
    with args.output.open("x") as output:
        output.write(worker)
    print(json.dumps(rust.metadata))


if __name__ == "__main__":
    main()
