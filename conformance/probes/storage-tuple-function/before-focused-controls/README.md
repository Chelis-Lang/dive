The first focused test attempt expected a predecessor metadata object in memory
mode and a search report for an API-level pre-search rejection. The second used
an incorrect v2 JSON path (`stored.repr`, where the public shape is `repr`). These
new-test mistakes were corrected against the unchanged existing API/transport
contracts. No production semantics, oracle output or assertion scope was weakened.
The third run retains every cross-product case and passes5/5; this focused pass
is separate from the final frozen whole-workspace gate and release controls.
