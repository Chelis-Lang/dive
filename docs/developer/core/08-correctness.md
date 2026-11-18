# Correctness net

Status: golden comparison and independent trace validation gate Core. The
shadow comparison against the reference engine was retired with that engine
(#791). The current findings and source-bound qualification are in
`m1-status.md`.

A rewrite of the core is safe only if every behaviour is checked mechanically
against independent references. This repository already has those references.
This document says how the new core uses them.

## 1. References

| Reference | What it checks | Where |
|---|---|---|
| Pinned TLC goldens | Observable contract (ADR 0086) | `conformance/golden/`, compared by `crates/dive-conformance/src/compare.rs` |
| Conformance baselines | No regression per tier | `conformance/baselines/` (small and full tiers, both profiles) |
| Independent validator | Every counterexample is a real behaviour | `dive-trace`, running on the reference evaluator |
| Fixture suites | Fine-grained TLC semantics (laziness, order, errors) | `crates/dive-eval/tests/fixtures/`, the `crates/dive-check/tests/` probes and errors |

## 2. Engine selection

- Core is the only engine of `dive check`, `dive simulate`, `dive dfid` and
  `dive tlc` (ADR 0153). `--engine core` is accepted; `--engine reference`
  and `--engine auto` are usage errors. `dive_api::check_file` runs Core.
- **JSON report.** A check that ran on Core names it (`engine: core`).
  Simulation and DFID reports keep `engine_requested` and a `null`
  `engine_fallback` in schema 3.
- **Harness and bench.** They run any engine unchanged, because they invoke
  the binary as a black box and always pass `--engine`
  (`crates/dive-conformance/src/dive.rs`; `crates/dive-bench/src/main.rs`).

## 3. No fallback

- **Core never falls back.** A refusal is an `unsupported` result naming the
  feature, so tests and benchmarks cannot be satisfied by the reference
  engine. The former `--engine=auto`, which reran a refused check on the
  reference engine, was removed with the cutover; `--engine auto` is a usage
  error. What only the reference engine implemented is a named Core refusal:
  source and TLC coverage, granted external inputs (`--allow-io`), bounded
  final checking and reference-engine checkpoint archives. Disk seen sets, checkpoint pause and finite-mode
  random/register effects run on Core or refuse there by name.
- **Implementation.** `crates/dive-api/src/check.rs` (the dispatch).
  `crates/dive-api/tests/core_default.rs` runs every core fixture on the
  default engine: each run must equal the explicit Core result, and every
  refusal must be named.

## 4. Shadow comparison (retired)

`dive-conformance shadow` compared Core with the reference engine, and
`scripts/fixture-shadow-census.py` ran it over every test fixture. Both were
retired with the reference engine (#791). Core is measured against the TLC
goldens; `dive check --core-sample N` still compares sampled successors with
the evaluator's.

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
`2930547e` plus the harness's `--impl dive:<engine>`. The counts come from
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
certify the separate diagnostic replay's class or counts. The original strict harness refuted this 2103 wrapper/word pair; the
bounded source-bound diagnostic proof below is now required to admit it.
Matching native metadata alone cannot authorize agreement. The canonical classifier leaves the clean pinned stream
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
a named role returning a non-Boolean value. The initial slice left the latter unproved, with
`ErConstrNonBool`/`ErActConstrNonBool` explicit non-agreement controls; the
source-qualified role extension in §16 now proves that boundary. The inner
case checks its exact source and rejects an earlier actual constraint error. A separate
control makes diagnostic generation reach a genuinely different error; a
matching primary cannot certify the deciding code. The bounded VIEW and
non-Boolean extensions are described in §16; broader global visited/alias
obligations, unsupported materialized
fingerprint demand, effects and temporal models remain refused. This is focused
word evidence, not a fresh census or closure of the remaining error families.
Native version-4 reader migration and consumer notice are required before landing.

The initial typed-family proposal at `aca6fa43` was rejected by independent
review. Its 92 positive fixture runs did not prove the general diagnostic
contract, and the claim of 92 qualified pinned agreements is retracted.
`candidate-diagnostic-mismatch` retains two minimal sources, configurations,
normal pinned stdout and hash-bound oracle facts before the correction:
`LaterDivDiagnosticOverflow` reports primary NONE 2179 but the separate unchanged
jar's `TLC.process()` diagnostic returns 2178; the symmetric
`LaterOverflowDiagnosticDiv` reports primary NONE 2178 but returns 2179. Both
normal processes exit 255. The bridge is diagnostic evidence only, not a
canonical deciding-class golden. Those normal streams remain unclassified.
Both former producers published verified version4 words that fresh independent
replay refuted. Primary source framing and a shared physical exit do not prove
the diagnostic class. Subsequent admission requires actual controlled fresh
word and diagnostic execution; an unproved or different lifecycle must refuse.

The corrected producer requires a fresh controlled source/configuration replay
before retaining a verified candidate class or publishing its word. It uses the
actual file check's IR, configuration, options and armed budget. Prefix and
terminal runtimes are fresh; neither a caller's evaluator nor a serialized claim
supplies authority. Invalid or unproved diagnostic execution yields unsupported
with no formal word or TLC class, preserving actual search statistics/output and
the observed primary cause in notes. Owned interruption/resource/internal errors
keep their actual transports. Different diagnostic causes are not numerically
retagged into a new claimed result.

The typed assertion/division/ENABLED/TLC arithmetic controls and `ErPropSeen`
retain 92 independently valid fixture/profile/engine/worker words. Of those,
64 comparisons have a classified pinned oracle and must match every compared
observable. The remaining 28 implied-action division/overflow observations
remain explicit non-agreements because their canonical oracle streams are
Unclassified. Independent word validity cannot invent the oracle's deciding
class. The source833 whole-census totals are not refreshed.

The owned classifier admits only the unambiguous 2132/physical14 assertion
family under canonical 2113 framing and the demonstrated completed-Init,
finite-word and replay-stack lifecycle. Admission is closed to the demonstrated
warning-free code/severity pairs, with canonical framing on every launch,
progress, phase and state message. Unrelated/trailing causes at any severity,
additional errors, altered parents/classes/order and mutable flat transcripts
supply no authority. Codes 2178 and 2179 share255 and receive no NONE promotion, even when
the primary child and an independently valid native word agree. Context-only
initial errors (#685), old Init ownership rules, VIEW, temporal candidate and
unproved newness/receiver lifetimes retain their existing boundaries. Issue #697
tracks the still-ambiguous cases; it is not closed by this bounded repair.


## 16. Bounded ordered prefix and VIEW error families

ADR 0135 extends the existing independent ordered driver with a private bounded
admission history. Fresh execution exhausts exactly one admitted Init emission,
finishes that source's entire first expansion, then establishes the first
admitted noninitial source from its actual key. Actual primary insertions carry
into per-action diagnostic enumeration and checks. Scalar states and bounded
scalar/tuple VIEW keys admit at most 64 keys and 256 emissions; collisions,
shape/work excess, later source lifetimes, additional initial emissions,
effects, symmetry, ALIAS and temporal obligations explicitly refuse. Existing
owned controls retain interruption/resource/internal transport.

The six `ErViewChoose/Domain/NonBool/NonBoolIf/NonEnum/TypeErr` controls use native
version 6 and a source-final `view-source-terminal` endpoint. Required concrete
normalization precedes VIEW; the absent failing candidate is independently
reconstructed. Reader migration and advance consumer notice precede landing the
writer. Outer report schema 3 and native versions 1–5 retain their meanings.

Top-level `ErConstrNonBool` and `ErActConstrNonBool` now execute the shared
source-qualified `Tool.isValid` boundary and use existing candidate version 4.
The old anonymous words remain negative controls. `ErInvConstrained` and
`ErInvReplayError` retain named version-2 words and gain a separate nonserialized
fresh diagnostic certificate. Source-free mapping cannot grant that wrapper;
source-bound mapping and independent conformance word validation execute the
proof. Removing the secondary InvB error must invalidate the original diagnostic
claim even though the original named InvA word remains valid.

Focused tests cover these ten cases in both engines/profiles at one and four
workers, exact original one-worker counts, wrong names/causes/source/candidate,
wrapper and version mutations, earlier constraint order, source selection,
bounds, unproved demand and interruption. These tests establish their stated
word/report scope only. Canonical mixed invariant streams and #697's shared255
streams remain Unclassified without an owned unambiguous oracle fact; no golden
or classifier widening, whole-census pass or phase acceptance follows.
