# ADR 0020: Finite seeded simulation

- Status: Superseded by [0116](0116-simulation-observable-contract.md) for its parity target and refusal rules; its engineering decisions stand
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
stable per-trace streams. Extra workers remain unsupported. Ordinary Print/PrintT is checked when reached;
other runtime host effects remain named refusals, preserving the architecture
for later scaling.

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
randomness outside the direct scalar Init subset and other runtime effects,
compound initial deep normalization, implied-initial
properties, action properties combined with state/action constraints, ALIAS/POSTCONDITION, generation, RL/probabilistic modes, continuation
and parallel/distributed execution remain explicit refusals. See
`crates/dive-sim/CONTRACT.md` for the supported surface and reproduction.

The pinned `SimConstrainedAction` probe reports action-property violation 2112
before its candidate is excluded by a state constraint. The exhaustive trace
validator currently requires constraints on property-ending edges. This slice
refuses the combination by name before sampling rather than weakening that
validator or reporting a false internal error. The control report accounts for
22 agreements and this one named refusal separately.


## Runtime Print extension

Ordinary Print/PrintT preserves evaluator argument, rendering and return-value
semantics in direct, captured and configured calls. The producer retains actual
binding output separately from runtime output. A fallible host policy bounds
retained runtime Print events/text. Fresh independent validation evaluates the
same operations but discards replay output before any sink callback; reached
random outside the supported Init subset, register/I/O refusals and the private
Init register contract remain.
No callback purity certificate, random replay proof or scheduler is added.
Nine pinned tiny primary probes distinguish real startup, Init and step output;
seven are admitted words and two remain named reached-effect refusals.
See `crates/dive-sim/CONTRACT.md` for limits; M/#239 remains partial.


## Shared evaluator simulation ownership

One simulation owns a bound evaluator core through host cleanup. Low-level
`simulate(&Model, ...)` calls on that same core, including cloned models, refuse
overlap or reentry without waiting: `unsupported` names concurrent/reentrant
simulation, with preflight stage, zero work and no output or host change. Existing
preflight option/feature refusals retain priority. Separate bound cores, including
separate API loads, can run concurrently. Callers must not mix foreign evaluator
or host operations with an active simulation; ordinary evaluator APIs are unchanged.

The run lease is operational ownership of the actual shared core. It releases
after the original host is restored, including error, limit and unsupported
returns. It introduces no TLS, callback admission or cache authority. The earlier
external overlap diagnostic and each scoped qualification retain their original
source bindings.

## Direct scalar Init random extension

Reuse the existing startup `RunHost`/`Streams` for one actual native invocation
while enumerating Init; retain the existing worker-selection stream unchanged.
The phase is an invocation-scoped runtime guard, not predicate identity or an
effect certificate. Native arguments execute before consuming the allowance.
The file/API boundary uses the existing independent direct scalar membership
relation to decide words; it does not establish seed-stream provenance.
Nested/correlated, wrapped, captured and configured Init relations remain
refused at that boundary. Low-level sampling returns raw candidates, while
reached Next/property random calls remain refused by the runtime host.

Caller-host startup history cannot be recovered through `Host`. Low-level models
with a retained host refuse a reached Init draw; absent-host callers must not
have performed startup random draws. File loading retains startup-random
refusals. Host restoration and shared-core lease ordering are unchanged. Eleven
pinned primary observations precede implementation; see the simulation guide
and compact random-init fixture. Full M/#239 remains open.
