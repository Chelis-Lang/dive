#!/bin/bash
set -eu
export CARGO_BUILD_JOBS=2
export CARGO_TARGET_DIR=/Users/jeff/code/dive-final-safety-target
cargo fmt --all --check
python3 scripts/test-validation-import.py
python3 -I scripts/test-frontend-campaign.py
python3 -I scripts/check-parity-ledger.py
python3 -I scripts/test-parity-ledger.py
for script in check-tlc-planning check-tlc-tableau check-temporal-action-order check-phase-l check-phase-l-graph check-phase-execution check-state-if-planning check-predecessor-spill check-native-checkpoint check-native-parity-integration check-tlc-final check-tlc-final-safety; do
 python3 -I scripts/$script.py
done
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo nextest run --workspace --locked
cargo test --workspace --doc --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
cargo xtask check-layering
cargo build -p dive-trace --locked
cargo deny --locked check
