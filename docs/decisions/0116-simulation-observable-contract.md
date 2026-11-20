# 0116: Simulation contract under observable parity

- **Status:** accepted; §4 superseded by [ADR 0162](0162-simulation-liveness-and-scale.md)
- **Date:** 2026-10-26
- **Supersedes:** [ADR 0020](0020-finite-seeded-simulation.md) (its parity target and refusal rules)

## Context

ADR 0020 (2026-10-10) defined simulation parity as reproducing TLC's random
stream: the same worker seed derivation, the same sequence of
initial/action/successor draws and the same prime-stride retries, accepted on
published traces that match pinned TLC runs draw for draw. Two days later,
ADR 0086 redefined parity as observable results and excluded nondeterministic
TLC internals. TLC's random selection order is such an internal: TLC's own
simulation counts vary across runs at one worker with a fixed seed (D-003,
Examples `EWD840_anim`). The simulation issues were rescoped under #241 (#111,
#178, #239: exact TLC draws are "best effort, not a parity gate"), but ADR 0020
and `dive-sim` were not.

The stream target is the source of simulation's largest refusals (#491):

- any `Set` or `Fcn` value in an initial state refuses, because matching TLC's
  draws would require modelling the lazy-cache side effects of
  `Simulator.deepNormalize` (`crates/dive-sim/src/lib.rs`);
- a `SPECIFICATION` with fairness conjuncts refuses even when only invariants
  are checked.

Almost every real specification has a function, record, sequence or set in its
initial state. Simulation therefore works only on toy models in both profiles,
including the default profile, which never used TLC's stream.

## Decision

Simulation is held to observable, checkable properties, not to TLC's draws.

1. **Soundness.** Every reported violation is a concrete behavior that the
   independent validator (`dive-trace`) accepts. Its verdict class, violated
   property, error class and exit code follow the same rules as `dive check`.
   A run without a violation is always reported as non-exhaustive
   (`exhaustive: false`, `model_proved: false`).
2. **Semantics.** Initial states, enabled actions, successors, invariants,
   action properties, constraints, deadlock and evaluation errors have exactly
   the semantics of `dive check` under the same profile. Only the choice among
   them is random.
3. **Reproducibility.** The seed is always reported. The same seed, options and
   worker count reproduce the same traces. Per-trace streams (D-003) make the
   result independent of the worker count where that is implemented.
4. **Scope.** Simulation accepts what `dive check` accepts for safety. This
   includes compound initial values and `SPECIFICATION`s whose fairness only
   constrains infinite behaviors. Temporal `PROPERTY` obligations under
   simulation remain a named refusal until designed separately.
5. **TLC's random stream is not a parity obligation.** `--compat=tlc` keeps
   TLC's semantic rules (value order, comparisons, integers), not its draw
   sequence. The existing Java-stream selection may be kept as a diagnostic
   mode where it already works, but no feature is refused to preserve it.
6. **Conformance.** Simulation corpus entries are checked on validity, not on
   trace equality. For every reported violation, the trace validates, and its
   violation kind is one the oracle's exhaustive check also reports for that
   model. A model whose exhaustive golden has a violation at depth *d* is
   expected to surface it within a documented trace and depth budget. That is
   a coverage check, tracked as a quality metric, not a parity failure.

## Consequences

- The compound-initial-value refusal and the fairness-only refusal are removed
  (#491). TLC-stream fidelity tests become diagnostics of that mode, not
  acceptance gates.
- ADR 0020's remaining engineering decisions stand: crate edges, an explicit
  positive trace count, TLC's default depth of 100, the separate versioned
  simulation report, bounded retained traces and independent validation.
- Multi-worker simulation and run budgets (#350) become ordinary feature work
  under this contract.
- `docs/user/simulation.md`, `crates/dive-sim/CONTRACT.md` and D-003 are updated
  with the implementation change, not by this ADR alone.
