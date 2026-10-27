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

**Built (M1, `core/engine-shadow`).** `dive-conformance shadow` runs both
engines on a corpus selection in the default profile and compares them.
It needs no golden, so any corpus model can be shadowed
(`crates/dive-conformance/src/shadow.rs`).

For every selected model and worker count (default 1 and 4):

- `dive check --engine core` runs first. A model the core refuses
  (`unsupported`, with the feature named) is counted as `refused` and not
  run on the reference engine.
- Otherwise `dive check --engine reference` runs the same model, and the
  two must agree on the verdict, error class and code, exit code and
  violated property. The distinct and generated counts and the depth must
  agree too, at one worker or when the search is complete. At a stop with
  several workers, which state ends the search depends on scheduling, so
  the counts are not compared there.
- The core's counterexample, if any, must validate with `dive-trace`
  against the reference result's ending, as the harness validates dive's
  counterexamples against a golden.

Statuses are `agree`, `refused`, `differ`, `trace-refuted` and `timeout`.
`differ` and `trace-refuted` are findings: the command exits 1 and prints
one line per finding. With `--strict`, `refused` and `timeout` are findings
too, for a fixed list of models the core is expected to run. The JSON
report (`schema: 1`) is written only with `--report-dir`. The summary,
printed as JSON, counts each status.

How to run it:

```sh
cargo build --release -p dive-cli -p dive-conformance
target/release/dive-conformance shadow --corpus examples --tier small \
  --workers 1,4 --tlc-budgets --budget-cap 900 --jobs 2
scripts/shadow-gate.sh conformance/gates/shadow-smoke.txt   # CI's list
```

Where it runs:

- CI's `Conformance gate` job runs `scripts/shadow-gate.sh` on
  `conformance/gates/shadow-smoke.txt`, under `--strict`. The list has ten
  fast examples models the core runs, covering an invariant violation, a
  deadlock, a parallel stop and successful searches.
- `cargo xtask gate --full` shadows the whole small tier (the four corpora
  of the small-tier runs) at 1 and 4 workers, without `--strict`, so models
  the core refuses are counted, not failed.

Not built yet: in-place successor sampling (comparing the successors of
sampled states as canonical state keys), shadowing under `--compat=tlc`,
and the cm1 list.

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
| M1 | Small tier at or above baseline with `--engine=core` (fallback allowed, and the fallback count is reported: `--impl dive:auto`, §8.1); shadow comparison clean on the small tier; ablation matrix clean |
| M2 | Full tier at or above the current 182 agreements with `--engine=core`, and at most 5% of models falling back |
| M4 | The core is the default; the reference engine remains for validation and comparison |

### 8.1 Fallback census (M1, 2026-10-27)

This is the small tier: 156 models in the examples, probes, validation-smoke
and configured-temporal-operators corpora, default profile, 1 worker. It ran
under `dive-conformance run --impl dive:auto --impl-profile dive --no-sany`
with `--baseline conformance/baselines/small-dive-w1.json`, on `main` at
`431d6f95` plus the harness's `--impl dive:<engine>`. The counts come from
the report's `summary.by_engine`.

| Engine that ran | Models |
|---|---|
| core | 29 |
| reference, static fallback | 119 |
| reference, dynamic fallback | 1 |
| none (ended before the search, or refused by both engines) | 7 |

Outcomes: 144 agree, 7 expected divergences, 5 unsupported (refused by both
engines), **0 disagree**. That is 0 regressions against the reference
engine's baseline, so the M1 gate's first clause holds with fallback.

Fallback reasons (the first run's `engine_fallback.reason`, grouped):

| Reason | Models |
|---|---|
| temporal properties (`PROPERTY`, liveness) | 68 |
| model values | 18 |
| `SYMMETRY` | 11 |
| value overrides (`<-` in the config) | 5 |
| standard operators (`TLCGet`, `RandomElement`, `ToJson`, `Trace`) | 5 |
| non-constant split arguments | 5 |
| `VIEW` or `ALIAS` | 4 |
| a split action inside nested `INSTANCE` substitutions | 1 |
| `x' \in` a lazily represented set | 1 |
| a false or failing `ASSUME` | 1 |
| dynamic: an error the reference evaluator does not reproduce | 1 |

The dynamic fallback is `LearnProofs/MCFindHighest`. The core raises an
error on `Len` of a function over `1..0`, which TLC treats as `<<>>` (issue
#527). After temporal properties, model values and `SYMMETRY` are the
largest wins for core coverage. The M2 target of at most 5% falling back
needs the liveness work.

To reproduce, run the command above. `jq '.summary.by_engine'` gives the
first table, and `jq '.models[].runs[0].engine_fallback'` gives the
reasons.

## 9. Next step

M0 needs only the golden check through `dive-bench` on the spike models. The
`--engine` flag and the fallback (`--engine=auto`, §3) are built in M1. The
shadow comparison (§4) is next.
