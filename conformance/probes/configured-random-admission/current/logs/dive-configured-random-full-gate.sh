#!/bin/bash
set -euo pipefail
export CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2
export CARGO_TARGET_DIR=/Users/jeff/code/dive-configured-random-target
export DIVE_CACHE_DIR=/Users/jeff/code/dive-configured-random-cache
start=$(date +%s)
date -u
cargo fmt --all --check
for file in test-validation-import.py test-frontend-campaign.py check-parity-ledger.py test-parity-ledger.py check-tlc-final-safety.py check-tlc-state-if.py check-tlc-temporal-if.py check-tlc-temporal-if-final.py check-tlc-final-scopes.py check-tlc-planning.py check-tlc-tableau.py check-temporal-action-order.py check-phase-l.py check-phase-l-graph.py check-phase-execution.py check-tlc-final.py check-state-if-planning.py check-predecessor-spill.py check-native-checkpoint.py check-native-parity-integration.py test-workspace-gate.py; do python3 -I scripts/$file; done
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -I scripts/check-test-partitions.py
stage=$(date +%s)
cargo nextest run --workspace --locked --profile ci --test-threads 2
printf 'nextest elapsed seconds: %s\n' "$(( $(date +%s) - stage ))"
cargo test --workspace --doc --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo build -p dive-trace --locked
cargo deny --locked check
printf 'full gate elapsed seconds: %s\n' "$(( $(date +%s) - start ))"
date -u
