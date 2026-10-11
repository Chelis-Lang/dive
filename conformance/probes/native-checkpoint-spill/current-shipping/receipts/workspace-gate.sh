#!/bin/bash
set -euo pipefail
export CARGO_TARGET_DIR=/Users/jeff/code/dive-checkpoint-spill-shipping-target
export DIVE_CACHE_DIR=/Users/jeff/code/dive-checkpoint-spill-shipping-cache
export CARGO_BUILD_JOBS=2
export RUST_TEST_THREADS=2
cargo fmt --all --check
for check in test-validation-import.py test-frontend-campaign.py check-parity-ledger.py test-parity-ledger.py check-tlc-planning.py check-tlc-tableau.py check-temporal-action-order.py check-phase-l.py check-phase-l-graph.py check-phase-execution.py check-tlc-final.py check-state-if-planning.py check-predecessor-spill.py check-native-checkpoint.py check-native-parity-integration.py; do
  python3 -I "scripts/$check"
done
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked --profile ci --test-threads 2
cargo test --workspace --doc --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo build -p dive-trace --locked
cargo deny --locked check
