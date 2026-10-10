# TLC parity ledger

Inventory baseline: main `217cebfa8bd35735bf14d38f28a4ae17628739aa`, 2026-10-10.
Implementation rows also reflect subsequent recorded increments.

The [delivery plan](design/tlc-parity-plan.md) covers the entire pinned TLC
capability set through a native Rust checker, including simulation, DFID,
distribution, recovery and PlusCal. This ledger records the denominator and
evidence without converting partial implementations into completed features.

## What the denominator means

The maintained [semantic ledger](../conformance/parity-features.json) has **37
