# ADR 0020: Finite seeded simulation

- Status: Accepted
- Date: 2026-10-10

## Context

TLC's ordinary simulation generates all candidates of a chosen action, checks
all of them, then randomly selects a surviving candidate. Its JVM
`Tool.probabilistic` option instead short-circuits randomized enumeration and
is a separate algorithm. Simulation must never be mistaken for exhaustive
model checking, and a violation is not certified solely by the simulator.

## Decision

Activate the existing permitted dependency edges for `dive-sim` to `dive-model`,
`dive-eval` with its `enumerate` feature, `dive-ir`, `dive-stdlib` and `dive-value`.
`dive-api` consumes `dive-sim`. The allowed crate graph needs no new edges.
`dive-sim` does not depend on the checker or trace validator. The independent
validator remains forbidden from depending on this crate or the enumerator.

Ship ordinary finite safety simulation first. Require an explicit positive
trace count; depth defaults to TLC's 100 transitions. Use the pinned
`Simulator`/`SimulationWorker` order: enumerate initial states once, check
invariants before constraints, then select initial/action/successor using the
same sequence of random draws and prime-stride disabled-action retries.

TLC mode derives its worker seed with `RandomGenerator(seed, aril).nextLong()`
and retains that worker stream across traces. An omitted seed is generated and
reported; TLC ignores aril in this case. The default profile uses the existing
stable per-trace streams. Extra workers and spec-level effects are not yet
implemented: refuse them, preserving the architecture for later scaling.

The public API returns a separate versioned simulation report. Its terminal
sampled result always states `exhaustive: false` and `model_proved: false`.
Retain uncompressed concrete traces and validate them independently. Expose a
separate TLC display projection that drops finite stuttering. A validator
refusal becomes `unsupported`; a rejected candidate becomes `internal-error`.
Retain the observation, stage and completed-prefix counts without certifying it.

Bound candidate vectors, retained trace states and generated work. Report
limits as limits. Do not increase existing checking/liveness bounds or change
their gates. Keep unknown statistics unknown after a failed worker.

Initial states are collected in full before checking completeness or roles,
as in `Simulator.simulate`. Initialization errors leave TLC's generated counter
unassigned (zero); `initial_candidates` separately records the collected prefix.
All retained states, including failing candidates, count against the trace
budget. A violation lacking its complete trace becomes a resource limit with
pending diagnostic metadata, never a certified violation.

## Consequences and evidence

Thirty published traces and their counts match ten clean pinned TLC runs
across five seeds and two arils. Twelve more probes cover initial and successor
invariants, action properties, constraint precedence, deadlock, empty/excluded
initial sets, incomplete states, initialization error precedence, invariant
evaluation and false assumptions.
Fixtures retain raw transcripts and trace modules with jar/JRE provenance.

This is a partial simulation implementation. Liveness/fairness, spec-level
randomness/runtime effects, compound initial deep normalization, implied-initial
properties, action properties combined with state/action constraints, ALIAS/POSTCONDITION, generation, RL/probabilistic modes, continuation
and parallel/distributed execution remain explicit refusals. See
`crates/dive-sim/README.md` for the supported surface and reproduction.

The pinned `SimConstrainedAction` probe reports action-property violation 2112
before its candidate is excluded by a state constraint. The exhaustive trace
validator currently requires constraints on property-ending edges. This slice
refuses the combination by name before sampling rather than weakening that
validator or reporting a false internal error. The control report accounts for
22 agreements and this one named refusal separately.
