# 0106: Temporal analysis bounds as a profile setting

- **Status:** accepted
- **Date:** 2026-10-19

## Context

Final temporal analysis enforces fixed entry and work counts at every stage:
planned properties (256), formula and tableau nodes (4096 each), expansion
steps, capture states/edges/generated states, atom label cells, product
vertices/edges/steps, and the compact acceptance-buffer bytes and work of ADR
0098. Exhausting any of them is a named resource refusal, never a verdict.

TLC has no such counts. Its liveness checking stops only when memory or time
runs out. In the TLC profile these bounds therefore turned models that TLC
checks into refusals. On the small tier at `1f5ae5fa`, measured with the
bounds raised and nothing else changed: `CoffeeCan/CoffeeCan1000Beans`
(product vertices; 501,500 distinct states, 130 s, 1.4 GB RSS) and
`FiniteMonotonic/MCCRDT` (atom cells; 25,000 distinct, 255 s, 0.9 GB) both
agree with the pinned golden. ADR 0010 requires `--compat=tlc` to select every
TLC-equivalent setting, and every option to name its TLC-equivalent setting.

## Decision

`--temporal-bounds default|unbounded` (Rust: `CheckRequest::temporal_bounds`,
`TemporalBounds`) selects the bounds of file-based temporal analysis, both on
the ordinary route and with `--lncheck final --lasso short`.

- `default` keeps `analyze::Limits::default()` and
  `AcceptancePolicy::compact_default()`. It is the default profile's setting:
  an explicit resource refusal stays preferable to unbounded growth there.
- `unbounded` selects `analyze::Limits::unbounded()` and
  `AcceptancePolicy::unbounded()`: no fixed count in planning, capture,
  labelling, product construction or acceptance materialization. This is the
  TLC-equivalent setting, and `--compat=tlc` selects it unless the flag is
  given. TLC is bounded by its heap instead, so `unbounded` keeps one memory
  stop: a resident-memory ceiling of a quarter of physical memory (the JVM's
  default maximum heap; `dive_liveness::resident`). The tableau, label and
  product loops sample the process's resident set size and refuse with
  `resident memory` above it; the covered acceptance buffers are limited to the
  same number of bytes. Where the platform does not report resident memory
  (not Linux), no sample is taken.

The unbounded per-binder limit (`usize::MAX`) also lifts the work ceilings of
pure predicate replay in `dive-eval` (100,000 finite-binding traversal visits
and 50,000 lazy query work units per atom invocation). `LoopInvariance/MCQuicksort`
needs both: it agrees in 517 s with them lifted, against TLC's 5 s. The
structural nesting-depth limit (256) stays in both settings. JSON reports the
resolved setting as `options.temporal_bounds.setting`.

The difference is registered as D-025.

Measured before the ceiling existed: with this setting and the companion
preflight fix, `MultiCarElevator/ElevatorLivenessMedium` (TLC: success) grew
past 77 GB resident in product construction before it was stopped. The ceiling
turns that into a named refusal well before it threatens the machine.

## Consequences

The TLC profile checks larger temporal models instead of refusing them, at the
cost of TLC-like memory and time. A model that would have refused quickly can
now run until the harness timeout; a timeout is never a holding result. The
default profile is unchanged. Whether the default profile should also lift
these bounds is an owner question, not decided here. Some liveness models are
far slower than in TLC once unbounded (pure predicate replay); that is a
performance issue, not a bound.
