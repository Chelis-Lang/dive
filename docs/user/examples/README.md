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

The checkpoint admission repair has
[fresh guide observations](../../../conformance/probes/native-parity-integration/current-controls/context-alias-repair/guide/receipt.json)
and [compiled Rust observations](../../../conformance/probes/native-parity-integration/current-controls/context-alias-repair/rust-guide/receipt.json).
The original observations above remain immutable and identify their original
runtime. The repaired source rechecks all eight commands plus the pure
checkpoint pause/recovery and original named refusal.
