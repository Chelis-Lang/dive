# 0162: Simulation checks temporal properties and runs at TLC's scale

- **Status:** proposed
- **Date:** 2026-11-20
- **Supersedes:** [ADR 0116](0116-simulation-observable-contract.md) §4 (temporal `PROPERTY` under simulation stays a named refusal), and the owner's 2026-11-06 answer in `docs/open-questions.md` that kept simulation with liveness a named refusal

## Context

`dive simulate` refuses 110 of the 235 configurations of the pinned Examples
corpus (`conformance/reports/2026-11-20-simulate-refusals-before.json`).
Temporal properties account for 73, SYMMETRY for 15, implied initial
properties for 11 and VIEW for 3. TLC simulates all of them. The owner now asks
for at least TLC's simulation capability, liveness included, with no deferral.

The pinned TLC, measured in `-simulate` mode
(`crates/dive-sim/tests/fixtures/temporal/oracle.json`):

- checks each finished trace for lassos, with fairness, on a graph of the
  trace's states plus stuttering self-loops. That is sound when node identity
  is concrete;
- reports success, without saying so, when no trace could decide a liveness
  property (a depth that never closes a loop);
- closes loops on view fingerprints (a spurious counterexample under `VIEW`)
  and on symmetric fingerprints (a missed violation under `SYMMETRY`);
- never checks implied initial properties in simulation;
- runs `num` traces per worker, and reports a trace and counts that depend on
  the worker count and on timing;
- enumerates every successor in `-simulate`, every initial state in both
  `-simulate` and `-generate`, and fails with GENERAL 1000 on domains over
  `Nat`.

dive today also stops long or many-trace runs at a fixed retained-state bound,
validates every sampled trace, and refuses workers.

## Decision

The full design, with evidence and complexity, is
`docs/developer/simulation-design.md`. In short:

1. **Temporal properties.** Each trace defines a trace graph: its distinct
   states under exact equality, its sampled steps, and a stuttering self-loop
   per state. A violation is a path of that graph on which every fairness
   conjunct holds and the property is false, found with the checker's existing
   planning and product search and validated by `dive-trace` before it is
   reported. `[]P`, `[][A]_v` and state-level conjuncts are checked per state
   and step as today; a defined safe temporal fragment is checked by
   progression on each step. Simulation never reports that a property holds.
2. **Coverage is reported.** Each trace has a class (closed, terminal,
   truncated, violated, cancelled), and each property has checked and
   unchecked trace counts with reasons. A truncated trace that holds no fair
   behavior counts as unchecked, never as passed.
3. **Truncated traces can be extended** with `--liveness-extend`, until they
   close or terminate within a step budget. Opt-in now; proposed as the
   default.
4. **SYMMETRY and VIEW** do not affect simulation: no deduplication, no
   canonical forms, loops close only on concrete equality, in both profiles.
   The validator accepts lassos under VIEW and SYMMETRY that carry concrete
   closing evidence. **ALIAS** renders a separate view of validated states.
5. **Implied initial properties** are checked in simulation as in `dive check`;
   implied action properties under constraints keep TLC's order, which the
   validator already accepts.
6. **Workers.** Trace ids come from a shared counter; every per-trace result is
   a function of seed and id; the run reports the lowest violating trace id and
   statistics over the ids up to it, so the report is identical at any worker
   count. `--traces` is the total. Output is released in id order.
7. **Scale.** The current trace is O(d) per worker; the report keeps the
   deciding trace plus K samples (`--keep-traces`, default 10), validates only
   those, and honors a byte bound. Successors are chosen by reservoir sampling;
   fixed candidate and generated-state bounds become budgets.
8. **Random operators** in Init and Next use a draw stream keyed by trace
   position, coordinated with #910; validation treats each draw as a choice of
   some element of its set.
9. **Trace output** gains per-trace streaming files, TLC's dump formats with
   ALIAS, and replay of selected trace ids.

Divergences D-031 to D-035, and an update of D-003, are registered with the
design.

## Consequences

- The simulate refusal buckets for temporal properties, SYMMETRY, VIEW,
  implied properties and ALIAS validation disappear after phases S1 to S3; the
  acceptance metric is the rerun histogram.
- dive answers correctly where TLC's simulation is unsound or silent (VIEW
  and SYMMETRY lassos, unchecked liveness, implied initial properties), and
  says how much of each property a sample checked.
- The checker's liveness planning and search gain a second caller through
  their public API; their owners' internals are unchanged.
- The default report shape changes (kept traces, new statistics); callers
  that read every trace must ask for `--keep-traces all` or `--trace-dir`.
- Revisit `--liveness-extend` and `--keep-traces` defaults after the owner's
  answers in `docs/open-questions.md`, and the `RandomElement` validation
  semantics with #910.
