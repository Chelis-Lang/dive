# Correctness net

Status: implemented shadow harness, sampled successor checks and repeatable
fixture census. Acceptance is still open; the current findings and source-bound
qualification are in `m1-status.md`.

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
  Liveness name lists may differ only when both actual words independently
  validate with complete attribution and their reported names match those words
  exactly (ADR 0126). The proof-free field comparison stays strict.
- The core's counterexample, if any, must validate with `dive-trace`
  against the reference result's ending, as the harness validates dive's
  counterexamples against a golden.
- Successor sampling (`core/engine-sample`). The core run gets
  `--core-sample N` (`shadow --sample N`, default 32; 0 turns it off). The
  explorer keeps a uniform reservoir sample of `N` expanded states, seeded
  by the run's seed, so it picks the same states at any worker count
  (`dive-explore/src/sample.rs`). Each sample also retains the expanded
  source's actual TLC level (initial states are level 1), supplied to both
  regenerated evaluators for `TLCGet("level")`. The reference host's previous
  level, including its absence, is restored afterward; its other capabilities
  and the reservoir's selection algorithm are unchanged. `LevelChain` pins
  the varying-level convention against normal TLC execution, while direct
  regeneration controls reject a shared wrong constant level (#600).
  After the search, `dive-api`
  (`core_sample.rs`) regenerates each sampled state's successors on the
  core, action by action, and compares them with the reference evaluator's
  `next_states` on the same state. They are compared as multisets of
  canonical state keys (`dive_store::state::state_key`, with a marker for
  an unassigned variable), so multiplicities must agree too. When both
  raise an error, that agrees: classifying it is the ending's job. The
  report's `core.sample` gives the counts and the first eight differences.
  A difference makes the row `successors-differ`. Sampling changes no
  result.

Statuses are `agree`, `refused`, `differ`, `trace-refuted`,
`trace-unconfirmed`, `successors-differ` and `timeout`. `differ`,
`trace-refuted`, `trace-unconfirmed` and `successors-differ` are findings: the command exits 1 and prints
one line per finding. With `--strict`, `refused` and `timeout` are findings
too, for a fixed list of models the core is expected to run. The JSON
report (`schema: 1`) is written only with `--report-dir`. The summary,
printed as JSON, counts each status.

An unclassified or internally contradictory report from either engine is a
`differ` finding, naming the engine and preserving its reason. Matching unusable
reports cannot establish agreement. Classified evaluation errors still compare
normally, and time or memory stops retain their separate `timeout` status.

An absent mandatory counterexample, a missing/unreadable word, or a named
validator refusal is `trace-unconfirmed`, never agreement, even if both engines
report identical observables. Safety/action/liveness violations and deadlocks
require a word; an error ending also requires one when the reference reports a
counterexample. An existing observable or successor difference keeps its status
and additionally records the validation detail. This closes issue #590; earlier
shadow measurements were not rerun by this change.

How to run it:

```sh
cargo build --release -p dive-cli -p dive-conformance
target/release/dive-conformance shadow --corpus examples --tier small \
  --workers 1,4 --tlc-budgets --budget-cap 900 --jobs 2
scripts/shadow-gate.sh conformance/gates/shadow-smoke.txt   # CI's list
```

Where it runs:

- CI's `Conformance gate` job runs `scripts/shadow-gate.sh` on
  `conformance/gates/shadow-smoke.txt`, under `--strict`. The list has 26
  examples models the core runs, invariant violations (stops, whose counts
  are compared at one worker only) and complete searches, each under 0.7 s
  for both engines at both worker counts on an M3, where the whole list
  takes about 2 s. The step should stay within about 30 s on CI.
- `cargo xtask gate --full` shadows `conformance/gates/cm1.txt` at 1, 4 and
  8 workers, then the whole small tier (the four corpora of the small-tier
  runs) at 1 and 4 workers, both without `--strict`, so models the core
  refuses are counted, not failed. On cm1 (21 models), 10 agree at all
  three worker counts and 11 are refused (2026-10-27).
- Results on the small tier (2026-10-27, after model values, #533; 156
  models at 1 and 4 workers): 50 models agree at both worker counts and
  106 are refused by the core. There is no difference, refuted trace,
  successor difference or timeout. The sample covered 1,642 states, 27,010
  (state, action) pairs and 6,498 successors.
- The sample found one core bug on its first run (#540): integers outside
  the core's inline range were reported as `<<>>` in counterexamples and
  wherever a core state is converted to the reference engine's values.

- `--impl-profile tlc` shadows the two engines under `--compat=tlc` (M2):
  the core runs one worker, the sample compares successors with the
  reference evaluator bound in TLC's profile, and the report's `profile`
  says which profile ran. Small tier at one worker (2026-10-27, M2 first
  piece): 49 models agree and 107 are refused, with no finding. The one
  model that agrees in the default profile but not under compat is
  `P1Overflow`, whose numeral `2147483648` fails TLC's start-up. The
  binding failure is refused instead of being reported as the reference
  engine reports it.

Not built yet: sampling from the initial-state predicate (only expanded
states are sampled).

### Fixture census

`scripts/fixture-shadow-census.py` stages the complete `crates/*/tests/fixtures`
population outside the checkout and runs the same shadow command. Its fixed
seed and fingerprint are both 0, supplied by the harness to both engines.
It compares ordinary checking of each configuration, even when a fixture's
other tests exercise simulation or DFID. It does not claim those mode tests ran.

```sh
python3 scripts/fixture-shadow-census.py --out /tmp/dive-fixture-shadow \
  --harness target/release/dive-conformance --dive-bin target/release/dive \
  --profile both --workers 1 --jobs 2 --timeout 20 --max-memory 3g
```

The output directory must be fresh and outside the checkout. Selection prefers
the `.tla` matching the configuration's stem, then the sole adjacent `.tla`.
Missing or ambiguous modules are explicitly skipped; `dive-config` parser
fixtures are explicitly excluded. Every config has exactly one outcome per
worker count and profile, including these unrun cases. Missing, duplicate or
extra report rows fail accounting. No filter or implicit retry narrows the
population, and no oracle golden is generated or edited.

The existing harness supplies the observable comparison, successor sampling
and independent trace validation. The recorder preserves every child command,
stdout, stderr and physical exit under `raw/`, with the harness logs and JSON
reports alongside them. `census.json` names native `stopped` and `resource-limit`
runs, their phases and exits separately from processes that never completed.
Neither timeout nor refusal earns agreement credit. The true candidate binary
hash and the fixture checkout's revision/dirty status are in `census.json`; the
shadow report's executable hash names the recording wrapper. The timeout is the
native time budget; the harness permits its existing 30-second termination grace.
Runtime measurement through this recorder earns no performance credit.

Preparation at source `e2183389` finds 1,450 configurations: 99 parser exclusions,
1,337 selected models and 14 ambiguous skips. These are selection counts, not
new agreement results. The handover's 845 agreements, 493 refusals and ten
differences sum to 1,348, rather than its stated 1,269 fixtures; that historical
summary remains unreconciled without its original report. A fresh run records
its own complete population and does not retroactively repair that measurement.

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
- `DIVE_CORE_OPT=-memo`, `-delta-hash` and similar, for S1–S8;
- the switches are listed in `09-performance-method.md`.

M1 acceptance requires a recorded small-tier matrix with each actual switch
off, asserting that all observables are identical. Focused unit matrices do not
substitute for that population. A strategy that changes an observable is a bug in
that strategy. The small-tier matrix exposed #603: replay compared compound
allocation handles when hash-consing was disabled and lost ten invariant
verdicts. Reconstruction now retains the exact handle fast path with interning,
and uses structural value equality without it. `core_trace_ablation` checks the
three-state sequence counter against pinned TLC (Inv/2110/exit 12, counts and
depth 3), checks 1/4/8 workers in both profiles, independently validates every
word, and rejects a changed middle state. A direct negative control also rejects
genuinely unequal nested values and unequal state arity.

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

Invariant evaluation-error witnesses retain the configured invariant name and
underlying cause independently of TLC's deciding wrapper. For invariant-site
2111, both engines emit a named `invariant-error` claim. The harness requires the
2111 report wrapper and independent replay of that exact name/cause. The
validator checks the finite word, earlier states and earlier configured
invariants before admitting the final error. It does not accept an arbitrary
last-state error or replace the observed cause with 2111. The pinned
`ErAssertInv`, `ErBagCardNonBag` and `ErBagCupNonBag` fixtures exercise both engines,
both profiles and workers 1 and 4; forged names/causes, absent errors and earlier
failures are negative controls. Generic expansion, constraint and action-property
error claims remain separate (#490); this is not closure of that whole family.

The pinned `ErInvReplayError` fixture separates a named invariant's error from
a different error encountered during diagnostic replay. InvA raises division
by zero (2179), the first report is 2111, and later replay raises a CHOOSE error;
the native checker reports deciding 2103, exit 75, five distinct/four generated
states, depth three. The finite word is `(0,0), (1,0), (2,0)`. Its logical
claim is still the named InvA error, representable by the existing version-2
ending. The producer uses its typed invariant-site/first-report context to
distinguish this case from an anonymous Next error. Independent
validation must check the exact name/cause and all earlier checks; it does not
certify the separate diagnostic replay's class or counts. The strict harness
still refutes this 2103 wrapper/word pair; matching native metadata cannot
authorize agreement. The canonical classifier leaves the clean pinned stream
unclassified, which must remain unknown rather than receive TLC agreement
credit. This bounded producer repair does not certify generic final expansion
(#434), missing partial-state words (#607), or the other #606 sites.

The separate `constraint-error-endings` corpus pins eight controls before the
#499 validator repair. TLC checks constraints in configuration order and stops
their evaluation at the first false result. It still checks invariants and
implied properties on the excluded generated state. `ConstraintAction` and
`ActionConstraintAction` have exact action-violation goldens (exit 13, two
generated/one distinct/depth one); `InitialConstraintProperty` has the initial
property violation golden. The first-error constraint control has an evaluation
error golden. Four invariant-error streams remain unclassified, so those
controls establish only qualified logical-demand observations, not primary
class or count agreement. The Init error prints no word. Full raw invocations
remain external; no classifier or error-ending wrapper exemption is introduced.

## 10. First terminal Next replay slice

ADR 0120 adds the optional `next-before-successor` error endpoint, using the
existing version-1 endpoint pattern. Its untrusted source context is captured
only from a typed native Next error. Validation reconstructs the prefix privately
and checks all earlier obligations, then drops that runtime before fresh
terminal binding, including the actual source-bound configured action split.
The shared evaluator enumerates actual actions in order with a sink stopping
at the first candidate. Any complete/partial candidate or composition work
before the claimed error stays unsupported. The first genuine error must match
the requested underlying class and original source, not its message text.
The generator checks its host composition capability before private intermediate
collection in both modes. TLA-mode intermediate states are absent from generated
counts, so counters or the outer successor sink cannot establish this refusal.

This needs no checker functor, visited set, graph or successor retention.
Existing evaluator controls and recursion bounds apply; new endpoint-owned work
retains fallible transport and phase/progress. Structural construction and
existing quiet/bulk allocation units are not a hard memory ceiling. Print,
registers, random operations and other unproved host effects retain refusals,
including new-lane startup reconstruction. Borrowed-model and infallible readers
cannot grant this fresh-source capability.

`EnabledIntervalFirstError` and `ErWrapFcnGuard` provide pinned source controls.
A valid underlying word does not authorize a 2103 wrapper or certify diagnostic
counts. Unknown canonical oracle streams remain unknown. The remaining #434,
#606 and #607 families are not closed by this conservative slice, and new
agreement credit requires a separately measured real harness outcome.

## 11. Partial terminal assignments

ADR 0123 retains observed partial assignments in both producer adapters rather
than parsing their display text. Native trace version 3 holds only complete
prefix rows in `states`, and a closed `next-partial-state` terminal descriptor
with the exact assignment partition and observed action source. Logical length
counts both. Report schema 3 and native v1/v2 meanings/output remain unchanged.

Fresh source-bound replay validates the complete prefix and reconstructs actual
configured actions on another private runtime. It stops at the first incomplete
candidate and compares exact source/assigned/unassigned bindings. An earlier
evaluation error or missing candidate refutes the word; an earlier complete
candidate remains unsupported until its ordered demand obligations are proved.
No seen set, checker callback, metadata waiver or value for an empty slot is used.
Typed interruptions/resource failures yield no completed validation report.

The ten historical absent-document fixtures now preserve genuine partial words;
eight validate in the first slice, with `RoActions`/`SplitThrows` still refused.
These are independent-word results only, not a new TLC census or closure of
#607/#434/#606. VIEW, symmetry, temporal, effects and composition remain bounded
by existing independent-replay refusals.


## 12. Ordered terminal replay

ADR 0125 consumes one pure driver in Next-error and partial Init/Next replay.
Complete candidates before a terminal event now execute ordered demand checks;
unknown visited/newness and alias history remain refusals, with no checker/store
import. The first actual Init candidate is constructively new. Materialized
fingerprint demand is cooperatively polled and never used as a seen-set proof.
The ten historical partial Next fixtures now validate, including `RoActions`
and `SplitThrows`; this supersedes section 11's first-slice eight-of-ten limit.

The new closed `next-ordered-terminal` stage proves actual primary exception
provenance and executes the real per-action diagnostic sequence, establishing
reported and deciding codes separately. Twenty-five Next/generator wrapper
fixtures plus exact assertion cases are tested in both profiles and engines.
The old before-first endpoint remains unchanged. Typed source origin attaches
at the actual generator step only when missing; native/classifier results and
canonical unclassified outcomes are unchanged. Other wrapper sites and raw
unproved value-source classification remain open (#606/#620).

Version-3 `partial-state` also admits `init-partial-state` with EC2102. Actual
Init action identity and scalar partial assignments can produce optional schema-3
`error_witness`, never a formal TLC behavior counterexample or verified flag.
Fresh source replay proves that request independently. No whole-census, global
BFS-count, TLC parity or milestone completion claim follows from these focused
word controls. Constraints, implied safety and VIEW terminal endpoints remain
separate acceptance work (#434/#490/#499/#606/#607 remain partial).


## 13. Candidate-side terminal replay

ADR 0128 adds a separate native version-4 `candidate-ordered-terminal` endpoint.
The complete final row is the generated candidate. Fresh validation checks the
complete source prefix on one private runtime, then reconstructs another to
observe actual configured action generation and ordered candidate checks. The
first real error must match site, source, cause and final bindings before actual
per-action diagnostic replay proves reported/deciding identities. Descriptor
metadata selects no operation. Plain errors and old Next stages keep strict
header checks and their earlier semantics.

The bounded sites are constraint, action constraint and implied action. Sixteen
existing fixtures validate in both profiles/engines at one and four workers:
Choose/Domain/NonBoolIf/NonEnum/TypeErr at each site, plus `ErPropNonBool`.
Their pinned primary headers remain unchanged. The last case raises an actual
source-qualified `StepBox` error inside `[Err(x)]_<<x, y>>`; it is distinct from
a named role returning a non-Boolean value. The latter remains unproved, with
`ErConstrNonBool`/`ErActConstrNonBool` explicit non-agreement controls. The inner
case checks its exact source and rejects an earlier actual constraint error. A separate
control makes diagnostic generation reach a genuinely different error; a
matching primary cannot certify the deciding code. VIEW, unproved top-level
non-Boolean role results, global visited/alias obligations, unsupported materialized
fingerprint demand, effects and temporal models remain refused. This is focused
word evidence, not a fresh census or closure of the remaining error families.
Native version-4 reader migration and consumer notice are required before landing.
