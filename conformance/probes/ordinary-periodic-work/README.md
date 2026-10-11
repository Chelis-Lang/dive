# Pinned periodic-work callback controls

All 296 selected observations run the actual pinned
`ModelChecker.doPeriodicWork` method after `FastTool` and `ModelChecker`
construction. The oracle SHA-256 is recorded in `capture.json`. The observing
Java subclass supplies queue suspension, graph-growth and SCC callback outcomes.
It observes checkpoint calls and ratio-update elapsed arguments; it does not
execute actual checkpoint persistence or ratio arithmetic.

The configured `_PERIODIC` expression is evaluated by TLC itself. Its PrintT
marker occurs once during constant evaluation at startup on stderr, and when
the periodic callback runs it occurs on stdout at the deciding position. Both
streams are retained. `independent-readback.json` verifies exact branch/callback
order, message codes, force retention, marker position and all 2,089 artifacts.
Only nonnegative elapsed clock arguments are abstracted; no measured-ratio proof
is claimed.

`candidate2.tar.gz` preserves the closed Java producer, compiler output, classes,
every selected spec/config, raw streams, receipt and independent readback. The
initial producer used `PERIODIC` instead of the pinned `_PERIODIC` keyword;
`initial-wrong-keyword.tar.gz` retains all 296 rows and its 200 setup failures.
Those failed rows receive no scheduling credit. The first independent observer
incorrectly looked for runtime markers on stderr; its exact source and failure
remain alongside the corrected observer. Raw producer files are unchanged.

The generated Rust cases in
`crates/dive-liveness/tests/fixtures/periodic_cases.rs` retain every selection and
ordered observation. `crates/dive-liveness/tests/periodic.rs` checks the controller
against them and rejects failures at every exercised callback boundary and a
negative elapsed clock. No case is removed because it returns a liveness or
PERIODIC error.

This is a callback-boundary study. No whole-model, ordinary scheduling,
checkpoint/recovery, worker-width, performance or family certification is claimed.
Actual SCC graph growth and source-bound quiescence remain backend obligations.
The [library guide](../../../docs/ordinary-periodic-controller.md) describes the
implemented contract.
