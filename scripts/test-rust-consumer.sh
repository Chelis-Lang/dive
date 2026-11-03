#!/usr/bin/env bash
# Compile an independent public-API consumer, then run with no CLI/Java on PATH.
set -euo pipefail
cd "$(dirname "$0")/.."
consumer_root="$PWD"
export CARGO_TARGET_DIR="$consumer_root/target"
cargo fmt --manifest-path crates/dive-api/tests/fixtures/rust-consumer/Cargo.toml --all --check
consumer_artifacts=$(mktemp)
trap 'rm -f "$consumer_artifacts"' EXIT
cargo test --manifest-path crates/dive-api/tests/fixtures/rust-consumer/Cargo.toml \
    --locked --no-run --message-format=json-render-diagnostics --target-dir "$consumer_root/target" "$@" > "$consumer_artifacts"
python3 - "$consumer_artifacts" <<'PY'
import json
import os
from pathlib import Path
import subprocess
import sys

executables = []
for line in Path(sys.argv[1]).read_text().splitlines():
    event = json.loads(line)
    if (event.get("reason") == "compiler-artifact"
            and event.get("target", {}).get("kind") == ["test"]
            and event.get("executable")):
        executables.append(event["executable"])
if len(executables) != 1:
    raise SystemExit(f"expected one independent consumer test binary, got {executables}")
env = dict(os.environ)
env["PATH"] = ""
subprocess.run([executables[0], "--test-threads=1"], env=env, check=True)
PY
