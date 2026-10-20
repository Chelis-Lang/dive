# 0106: Temporal analysis bounds as a profile setting

- **Status:** proposed (awaiting owner sign-off; see `docs/open-questions.md`,
  "Temporal bounds under `--compat=tlc`")
- **Date:** 2026-10-19
- **Amends:** [ADR 0098](0098-compact-acceptance-buffer-bounds.md), whose
  shared file-checking resource policy now applies to the `default` setting only

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
  default maximum heap; `dive_check::resident`, re-exported as
  `dive_liveness::resident`). Graph capture samples the process's resident set
  size every 4,096 retained states and edges
  (`dive_check::capture::Limits::resident_bytes`), on whichever thread retains
  them. The analysis thread samples it every 4,096 units of planning, tableau
  expansion, state labelling and product construction. Above the ceiling each
  stage refuses with `resident memory`; the covered acceptance buffers are
  limited to the same number of bytes. Where the platform does not report
  resident memory (not Linux), no sample is taken.

Samples fall between units of work, never inside one. One successor
computation during capture, and one atom evaluated for one state during
labelling, are not interrupted: an evaluation that alone exhausts memory (for
example, an atom that enumerates a huge `SUBSET`) ends the process the way an
`OutOfMemoryError` ends TLC, with no verdict and no `resource-limit` result.
The ceiling bounds accumulated growth, not a single evaluation.

The ceiling depends on the host, as TLC's default heap does: on a 16 GB
machine it is 4 GB. A model whose analysis needs more than a quarter of
physical memory can therefore agree on a large machine and refuse on a small
one. The JSON report states the ceiling in effect as
`options.temporal_bounds.resident_ceiling_bytes`.

The unbounded per-binder limit (`usize::MAX`) also lifts the work ceilings of
pure predicate replay in `dive-eval` (100,000 finite-binding traversal visits
and 50,000 lazy query work units per atom invocation). `LoopInvariance/MCQuicksort`
needs both: it agrees in 517 to 692 s with them lifted, against TLC's 5 s. The
structural nesting-depth limit (256) stays in both settings. JSON reports the
resolved setting as `options.temporal_bounds.setting`.

The difference is registered as D-025.

Measured before the ceiling existed: with this setting and the companion
preflight fix, `MultiCarElevator/ElevatorLivenessMedium` (TLC: success) grew
past 77 GB resident in product construction before it was stopped. With the
ceiling, product construction refuses with `resident memory` at about the
ceiling (34 GB peak on a 125 GB machine) instead of exhausting the machine.

In the full gate's TLC-profile run, five models move from unsupported to
agree (139 to 144 of 156): `CoffeeCan1000Beans` (121 s), `MCCRDT` (268 s),
`MCQuicksort` (692 s), `AllocatorImplementation` (511 s) and
`MisraReachability/MCParReach` (1,530 s, against the manifest's 30 s for TLC and a
1,800 s gate timeout).

## Consequences

The TLC profile checks larger temporal models instead of refusing them, at the
cost of TLC-like memory and time. A model that would have refused quickly can
now run until the harness timeout; a timeout is never a holding result. The
default profile is unchanged. Whether the default profile should also lift
these bounds is an owner question, not decided here. Some liveness models are
far slower than in TLC once unbounded (pure predicate replay); that is a
performance issue, not a bound.

ADR 0098 said that both semantic profiles use the same file-checking resource
policy and that acceptance-buffer exhaustion remains an explicit refusal. Under
this decision that holds for `default` only; `unbounded` replaces the
acceptance-buffer byte and work allowances with the resident ceiling. Because
this reverses part of an accepted ADR, it stays proposed until the owner signs
off.
