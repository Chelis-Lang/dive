# Correctness net

Status: design. There is no code yet.

A rewrite of the core is safe only if every behaviour is checked mechanically
against independent references. This repository already has those references.
This document says how the new core uses them.

## 1. References

| Reference | What it checks | Where |
|---|---|---|
| Pinned TLC goldens | Observable contract (ADR 0086) | `conformance/golden/`, compared by `crates/dive-conformance/src/compare.rs` |
| Conformance baselines | No regression per tier | `conformance/baselines/` (small and full tiers, both profiles) |
| Reference engine | Every observable, plus intermediate results on any specification, including those with no golden | Frozen `dive-eval`, `dive-model`, `dive-check` |
| Independent validator | Every counterexample is a real behaviour | `dive-trace`, running on the reference evaluator |
| Fixture suites | Fine-grained TLC semantics (laziness, order, errors) | `crates/dive-eval/tests/fixtures/`, the `crates/dive-check/tests/` probes and errors |

## 2. Engine selection

- `dive check --engine=reference|core|auto`, or `DIVE_ENGINE`. The default
  stays `reference` until M4. The same choice is available through
  `dive_api::check_file_engine`.
- **JSON report.** It names the engine that ran (`engine`). Under `auto` it
  also carries `engine_requested` and the fallback (`engine_fallback`: stage
  and reason, or `null`).
- **Harness and bench.** They run any engine unchanged, because they invoke
  the binary as a black box (`crates/dive-conformance/src/dive.rs`;
  `crates/dive-bench/src/main.rs`).

## 3. Static fallback (`--engine=auto`, built in M1)

- **`--engine=core` never falls back.** A refusal is an `unsupported`
  result naming the feature, so tests and benchmarks cannot be satisfied by
  the reference engine. This is the role the design gave `--no-fallback`.
- **Refusal before the search (static).** The core refuses, naming the
  feature, any construct or run option it does not implement. Examples are
  `--compat=tlc`, `--store exact`, non-default evaluation options, disk
  spilling, `--chaos`, temporal properties, `VIEW`, `SYMMETRY`, a refused
  plan construct, and checkpoints. With `--engine=auto`, the reference
  engine then runs the check.
- **Refusal during the search (dynamic).** The core aborts when it meets an
  unsupported case during the run: a lazily represented value it cannot
  enumerate, a mixed-class set, or an error ending it cannot reproduce. Auto
  discards the core's result, counts and output (the core buffers all
  output), reruns the whole check on the reference engine under the same
  armed budget, and reports both facts. It never continues mid-run on
  another engine.
- **Implementation.** `crates/dive-api/src/check.rs` (the dispatch) and
  `core_engine::Fallback`. `crates/dive-api/tests/core_auto.rs` runs every
  core fixture under auto. Each run must equal the core's result, or the
  reference engine's result with a named fallback. Both stages are covered.

## 4. Shadow comparison

`cargo xtask shadow` is a new command, added in M1. It runs both engines on a
corpus selection, at 1 and 4 workers and in both profiles, and compares:

- verdict, error class, exit code and violated property;
- distinct and generated counts, and depth;
- trace validity under `dive-trace`, and trace length and ending;
- on a sample of explored states (seeded, reported), the multiset of
  successors of each sampled state, compared as canonical state keys.

It writes a compact JSON report. Any difference is a failure, unless an
expected-divergence entry explains it. In the default profile there should be
none.

CI runs it on the smoke gate. The full gate runs it on the small tier and cm1.

## 5. Differential fuzzing

Specification mutants are generated from corpus models by deterministic
mutations. The operators are:
- swapping operators;
- dropping conjuncts;
- perturbing constants;
- injecting errors such as division by zero in guards;
- adding invariants.

Each mutant runs on the core, the reference engine and TLC, and all
observables are compared. The existing mutation tool used for spot checks is
promoted into the repository for this, and its seeds are reported. Fuzzing runs
by hand, or in the deep tier.

## 6. Ablation runs

Every performance strategy has a switch:
- `--core-opt=-memo`, `-delta-hash` and similar, for S1–S8;
- the switches are listed in `09-performance-method.md`.

A test matrix runs the small tier with each switch off and asserts that all
observables are identical. A strategy that changes an observable is a bug in
that strategy.

## 7. Validator independence

The reference engine is frozen and stays the validator's evaluator. The
validator therefore shares no evaluator code with the core. That is stronger
than today, where the checker and validator share `dive-eval`, including the
stability analysis noted in ADR 0103.

Every core counterexample is validated in both profiles. A core counterexample
that the validator rejects is a core bug.

## 8. Gates per milestone

The thresholds are in `10-plan.md`. In short:

| Milestone | Required |
|---|---|
| M0 | Spike models match the goldens |
| M1 | Small tier at or above baseline with `--engine=core` (fallback allowed, and the fallback count is reported); shadow comparison clean on the small tier; ablation matrix clean |
| M2 | Full tier at or above the current 182 agreements with `--engine=core`, and at most 5% of models falling back |
| M4 | The core is the default; the reference engine remains for validation and comparison |

## 9. Next step

M0 needs only the golden check through `dive-bench` on the spike models. The
`--engine` flag and the fallback (`--engine=auto`, §3) are built in M1. The
shadow comparison (§4) is next.
