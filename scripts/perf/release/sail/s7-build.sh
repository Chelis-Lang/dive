#!/bin/bash
# build.sh <label> <sail-ref>: build sail, the staged worker and temporal_bench into ~/inst-<label> (as bench).
set -eux
L=$1; REF=$2
source ~/.cargo/env
export CARGO_NET_GIT_FETCH_WITH_CLI=true
D=~/src-$L
[ -d $D ] || git clone -q ~/sail $D
cd $D && git -C ~/sail fetch -q origin && git fetch -q ~/sail '+refs/remotes/origin/*:refs/remotes/origin/*' && git checkout -q "$REF"
git log -1 --format=%H > ~/inst-$L.rev
mkdir -p crates/sail-dive/examples && cp ~/tools/temporal_bench.rs crates/sail-dive/examples/temporal_bench.rs
cargo build -q --release --locked -p sail-solve --bins
cargo build -q --release --locked -p sail-dive --example temporal_bench
rm -rf ~/inst-$L; mkdir -p ~/inst-$L/bin
cp target/release/sail target/release/sail-runtime-worker ~/inst-$L/bin/
cp target/release/examples/temporal_bench ~/inst-$L/
python3 scripts/stage-temporal-worker.py ~/inst-$L/bin/sail-runtime-worker ~/inst-$L/bin
rm -f crates/sail-dive/examples/temporal_bench.rs
sha256sum ~/inst-$L/bin/* ~/inst-$L/temporal_bench > ~/inst-$L.sha
touch ~/BUILD_$L
