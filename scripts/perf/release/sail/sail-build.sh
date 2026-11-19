#!/bin/bash
# Builds the Sail measurement binaries on the Sail node (run as bench).
set -eux
source ~/.cargo/env
export CARGO_NET_GIT_FETCH_WITH_CLI=true
cd ~/sail
git log -1 --format=%H
cargo build -q --release --locked -p sail-dive --example temporal_bench
mkdir -p ~/inst/bin
cp target/release/sail target/release/sail-runtime-worker ~/inst/bin/
python3 scripts/stage-temporal-worker.py ~/inst/bin/sail-runtime-worker ~/inst/bin
~/inst/bin/sail runtime capabilities | head -c 1500
REV=$(grep -A2 'name = "dive-api"' Cargo.lock | grep -o '#[0-9a-f]*' | tr -d '#')
echo "dive rev $REV"
cd ~/dive && git fetch -q && git checkout -q "$REV" && cargo build -q --release --locked -p dive-cli --bins
mkdir -p ~/bins && cp target/release/dive ~/bins/dive
~/bins/dive --version
sha256sum ~/inst/bin/* ~/bins/dive ~/sail/target/release/examples/temporal_bench
touch ~/BUILD_DONE
