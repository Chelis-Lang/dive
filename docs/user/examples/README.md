# Runnable guide examples

Run guide commands from the repository root after building `dive-cli` in release
mode. `Counter.tla` has five reachable states and a self-loop at four.

| Configuration | Expected supported result |
| --- | --- |
| `Counter.cfg` | Safety success: five distinct, six generated, depth five |
| `CounterViolation.cfg` | `StrictBound` invariant violation at four; TLC-profile exit 12 |
| `CounterTemporal.cfg` with final/short flags | Liveness violation: stuttering at zero falsifies eventual four |
| `CheckpointCounter.cfg` | Pure safety pause/recovery; five states after recovery |
| `Algorithm.tla` | Admitted in-memory native sequential PlusCal artifact |

The sample commands for parse, analysis, safety, violation, simulation, DFID,
translation and explicit temporal checking were executed with the recorded
combined native runtime. Their [command observations](observations.json) record
arguments, exits and dispositions. The observation is bounded example evidence,
not certification of any TLC family or a general performance measurement.
Runtime/source and binary identity are linked from that record to the immutable
combined controls. Future functionality updates must recheck changed examples
and retain failed observations rather than changing their expected status silently.

The [additional observations](command-observations/checkpoint-pause.observation.json)
retain actual checkpoint pause/recovery output and the initial named purity
refusal from `Counter.tla`; the original source was not weakened to hide it.

`MacroExample.tla` and `MacroExampleC.tla` are the checked C/P examples in
[the bounded macro guide](../../pluscal-macros.md). The macro delivery retains
their original runtime observations; current integration controls remain separate.
The checkpoint admission repair has
[fresh guide observations](../../../conformance/probes/native-parity-integration/current-controls/context-alias-repair/guide/receipt.json)
and [compiled Rust observations](../../../conformance/probes/native-parity-integration/current-controls/context-alias-repair/rust-guide/receipt.json).
The original observations above remain immutable and identify their original
runtime. The repaired source rechecks all eight commands plus the pure
checkpoint pause/recovery and original named refusal.

`ModuleExample.tla` and its sibling configuration exercise `Naturals`,
`Sequences` and `FiniteSets`. [Current module observations](../../../conformance/probes/native-parity-integration/current-controls/macros-integration/module-guide/receipt.json)
retain analysis and both-profile checks: three distinct states, four generated,
depth three. The [macro integration receipt](../../../conformance/probes/native-parity-integration/current-controls/macros-integration/verification.json)
also binds fresh C/P macro examples separately from their original observations.
