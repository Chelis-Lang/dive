# Sprint status, 2026-10-10

M0 and M2–M4 are complete. The safety checker, independent trace validator
and deterministic parallel search are implemented and protected by the CM1
PR gate at 1, 4 and 8 workers. M1 and M5 remain open for the work below.
M6 has independent temporal-word evaluation and graph-search foundations.
Explicit final/short temporal checking is available; ordinary strict
`dive check` still refuses temporal model checking by name. Pure safety
symmetry now canonicalizes keys and retains concrete validated traces;
unsupported symmetry startup, value and temporal interactions still refuse.

The approved [full TLC parity plan](design/tlc-parity-plan.md) now covers the
remaining checker features, all checking modes, native recovery/distribution
and PlusCal. The [feature ledger](tlc-parity.md) inventories 37 semantic
families and their source surfaces. Its implemented/partial labels and current
model counts do not certify complete families or establish an overall percent.

## Current delivered increments, 2026-10-10

Since the preserved standard report below, the verified increments now include:

- Pure safety symmetry, with concrete independently validated counterexamples;
  unsupported value/startup and temporal combinations still refuse by name.
- Bounded pure deferred domains and caller-scope preservation for supported
  temporal arguments. Recursive and exported INSTANCE cases remain explicit gaps.
- Seeded finite safety simulation and native DFID. Each mode reports its actual
  scope and named refusals; neither implies complete simulation/DFID parity.
- Ordered parallel temporal graph capture, plus a bounded TLC normalization,
  DNF, order-of-solution and particle/tableau projections. These planning artifacts do not lift
  ordinary strict temporal model-checking refusals.
- A native PlusCal source parser with field-level AST controls against the pin.
  Source parsing does not imply translation, rewrite or checksum parity. A
  separate bounded C/P translation slice compares generated operator trees and
  checks admitted enclosing modules, sequential algorithms and disjoint finite
  processes with pinned fairness. Bounded macros now expand with checked source origins and enclosing-module
  admission; procedures, general renaming,
  rewriting and checksums remain pending.
- Opt-in ordered disk frontiers for safety BFS. The 21 CM1 and 16 generated
  ValidationTestSuite models agree at 1/4/8 workers both with memory queues and
  disk forced from the start. Current/next encoded frontiers are bounded;
  visited sets, candidates, traces and temporal graphs are still outside that
  budget, and checkpoint/recovery is not implemented by these transient files.

The [combined parity increment](native-parity-integration.md) preserves the
original source and probe histories for planning, concrete graphs, checkpoints,
public IR admission and bounded process fairness. Graph insertion remains an
explicit staged API, not ordinary strict temporal checking.

The implementation census is seven implemented cores, 26 partial families and
four missing families out of 37. Every family remains audit-required. The
retained whole-selection standard report still provides the strict baseline;
these smaller acceptance studies do not replace it or establish a percentage
of complete TLC parity.

## Preserved standard strict model evidence

Built from main `7ceb59f`, with binary SHA-256
`715ddf1bc54f7c85d2aa0c7d4926751e8fa93c1900f2761d2ec3552b94a10366`,
against the pinned TLC bundle and Examples `6a7b26a`, at one worker:

- The [full report](../conformance/reports/20261009T122115782-dive-small.json)
  selected all 154 Examples and probe models: 82 agree, 71 unsupported,
  one timeout, no disagreements and no pending trace validations.
- [The isolated timeout recheck](../conformance/reports/20261009T122713440-dive-small.json)
  agrees on EWD998PCal: 2,808,396 generated states, 321,370 distinct states,
  depth 47, in 297.5 seconds. The initial four-job run hit its fixed
  1,200-second timeout while other host workloads ran.
- Together, these two reports establish 83 agreements and 71 explicit
  refusals across the selected models. The timeout remains in the original
  generated report and in issue #53; it is not rewritten into agreement.

The evaluator-core borrow cleanup subsequently passed in one complete
[standard run](../conformance/reports/20261009T134501007-dive-small.json):
83 agree, 71 explicit refusals, no disagreement, pending validation, timeout
or oracle error, with two jobs and a fixed 2,400-second timeout. The generated
report records its binary hash and pins. Paired busy-machine measurements
establish no speed-up; the performance targets remain open.

Seeded random trace validation now requires a proof that predicate and
state-generation draw orders agree. Unknown alignment is refused even for
coincidental matches. P9Random now fully agrees, resolving the previous
pending trace validation. Worker-introspecting TLC-profile runs use pinned
references at the same worker count, comparing verdict and error class.

The repaired nightly workflow passed on Linux, including TLC's goldens and
dive's front end. That run preceded the error-matrix and replay fixes above;
it supplies cross-platform evidence, not a substitute for their validation.

## Remaining M1 and M5 work

- **M1:** ValidationTestSuite ingestion and TLC model-corpus coverage (#2),
  plus minimal wrong-answer regressions for the upstream-change register
  (#16). Oracle roles and the register itself are implemented.
- **M5 errors:** value-construction provenance is still needed to reproduce
  ten source-dependent TLC deciding error classes (#61). These classes
  remain unverified and cannot count as agreement.
- **M5 storage/IR:** bounded predecessor spill and pure safety native checkpoints
  are implemented as explicit options. General visited/candidate/value/graph
  spill, temporal/runtime checkpoint recovery and distribution remain pending.
  Current checkpoints recover saved discoveries/frontiers, run inline, and
  explicitly refuse unproved interactions.
- **M5 performance:** the benchmark runner is implemented; the recorded
  smoke timings come from a busy machine and establish correctness and
  reproducibility of measurement, not achievement of the performance
  targets. Profiling and paired baseline work remain (#28, #53).
- **M5 coverage:** unsupported library operators and deadlocks lacking finite
  independent covers remain honest limitations. M6 covers temporal and remaining
  symmetry interactions; refusals must not disappear merely to raise coverage.

## M6 critical path

Follow [the M6 plan](design/m6-plan.md): temporal formula/tableau planning,
behavior-graph capture, then integrated accepting-lasso search and
property attribution. A bounded formula/tableau language kernel now checks
boolean combinations, always and eventually over supplied state/action atoms.
Pure bound predicates lower to that kernel with retained scopes and source
diagnostics and WF/SF enabledness expansion. Pure caller-scoped temporal
arguments and bounded TLC normalization/DNF planning are implemented; dynamic
contexts, recursive/exported INSTANCE arguments and exact execution remain
pending.
Independent fairness and successor-existence checking
(#63), exact TLC lasso choice, remaining symmetry interactions, scheduling, spill and performance
remain outstanding. The independent truth-vector and graph APIs are
foundations. The graph/tableau product preserves per-edge atom truth and
projects accepting witnesses without discarding repeated obligations;
a bounded final-only pure coordinator now captures real model graphs.
Named pure concrete periodic counterexamples validate
independently, including closing edges and the complete property set.
Recurring non-stuttering action occurrences now certify WF/SF independently
on a supplied cycle. Ten pinned counterexamples validate; finite covers independently decide
ENABLED and WF/SF without recurring occurrences. Missing covers and
VIEW/symmetry closing evidence remain refused.
The final-only bounded library coordinator now matches ten pinned violations
and three successes in both stores, with complete property attribution.
The public Rust temporal API returns periodic violations only after independent
validation of the in-memory word and lossless document, retaining full safety
stop evidence. The ordinary `dive check` path still refuses
temporal models. Full phase-L/parallel capture, broader temporal/scoped
constructors, strict CLI conformance and TLC-exact planning/selection remain pending.

Approved defaults are recorded in ADRs and the differences log: explicit
host-I/O grants, short deterministic lassos, unreduced symmetry/liveness,
rejection of malformed permutations, an opt-in symmetry audit, and separate
default/compat scheduling paths. They remain planned until implemented and
proven. See [the maintained TLC differences log](tlc-differences.md).


The next L1/L4 increment supports finite constant-domain quantifiers, tuple
bindings and inspected pure configured constants. Original configured PROPERTY
formulas now retain unambiguous claim names when the binder splits checking
roles. Five additional pinned probes match verdict truth/counts in both profiles
and stores. The harness now classifies finite-prefix temporal violations correctly (#77);
full checker finite-prefix stopping/timing remains open.

Pure state- and temporal-guarded `IF` formulas now lower pointwise and validate
independently, including nested guards and closing-edge positions. Three new
pinned probes match truth and counts in both profiles/stores. The oracle
rejects action-guarded temporal IF (2214) and temporal CASE (2213); dive retains
named planning refusals for these cases, even though SANY accepts their syntax.

The bounded final Rust backend now has explicit CLI/report access with
`--lncheck final --lasso short`. Temporal violations return independently
validated lossless lassos, complete property claims and actual worker usage.
The mixed TLC profile is labeled and makes no exact temporal-class/stopping
claim. Full phase-L, parallel capture, exact TLC selection and ordinary strict
conformance integration remain pending.

Pure constant-argument temporal operators now retain scalar/tuple bindings
through nested and quantified calls. Five pinned probes match final truth and
counts in both profiles and stores; every returned periodic claim independently
validates. Lazy state arguments, higher-order operators and missing finite
ENABLED covers remain explicit refusals.

Independent finite ENABLED covers now support pure constant-argument action
operators and value-bound locals. They agree with pinned TLC on 108 decisions
across 36 initial states. Three new final temporal probes match truth/counts
in both profiles/stores; the violation requires independent disabledness to
validate fairness. General lazy/higher-order contexts remain open.

Pure state arguments now retain their caller scopes through temporal operators.
Six pinned probes match final truth/counts in both profiles/stores; direct
short-word controls also cover nested forwarding, tuple/state guards and
priming across the closing edge. Action/temporal and higher-order arguments
remain refusals. Exact TLC temporal planning/timing/selection is still open.

The separate final/short LM1 study now records all fifteen models at requested
widths 1, 4 and 8: ten distinct models match truth/counts, five refuse by name,
with no findings. See [the study guide](design/final-temporal-study.md) and its
raw report. Actual graph capture remains inline; this is not strict TLC agreement
or parallel scaling. The next scoped-planning increments are guided by the
remaining substitution, domain, tableau and independent-validation refusals.

ValidationTestSuite now has a reproducible bounded smoke import: all 36
selected cases accounted for, 16 imported models and 20 modules, with generated
pinned goldens. All 16 models fully agree under the TLC profile at 1, 4 and 8
workers and all 20 modules agree with SANY. The required conformance check
includes this set. Broader generator and JUnit-model coverage remain open in
#2; see `docs/conformance-corpora.md`.

The deep frontend campaign is now a reproducible workflow job: all 340 local
SANY cases and 768 seeded Examples mutations, with grammar measured separately
from semantic/level acceptance. All 1,108 cases agree in each phase locally;
the frontend-only Linux run 37989445200 passed. Issues #29 and #98 are closed.

Bounded state-dependent predicate domains now retain actual state/edge context.
Eight pinned probes match final truth/counts in both profiles/stores, including
independently validated enabledness and closing-edge priming. The latest LM1
final/short study has 39 matches and 6 explicit refusals (13 of 15 models
supported), with no findings. FourQueens matches its safety stop and ChangRoberts
its final success. DistributedReplicatedLog retains an opaque scoped-level
refusal; DiningPhilosophers reaches the tableau-node bound. Actual graph capture
is still inline, and none of this is strict TLC temporal parity.

Finite CHOOSE now works inside final/short predicates, including Min/Max-style
operators and retained constant/state-dependent domains. Five pinned probes
match final truth/counts in both profiles/stores; exhaustive word and domain
level controls pass. LM1 stays at 39 matches and 6 refusals, with zero findings:
DistributedReplicatedLog gets past Min/Max and now reaches the explicit
action-argument refusal; DiningPhilosophers remains at the tableau bound.

Pure value/action helpers now retain primed caller arguments, including nested,
captured and sequence-prefix calls. Four pinned `Aa*` models match final truth
and counts in both profiles/stores and independently validate. The latest LM1
study still has 39 matches and 6 named refusals with no findings, but both
remaining models now reach the tableau-node bound rather than a scoped-language
refusal. Exact TLC planning/timing/selection and parallel capture remain open.

The fairness/product increment factors pure recurring conditions out of temporal
memory and retains only reachable products, keeping resource defaults unchanged.
Three five-process pinned probes match final truth/counts in both profiles/stores,
including mixed weak/strong fairness and an independently rejected unfair mutant.
The latest LM1 final/short study now records 42 truth/count matches (14 models)
and three VIEW refusals (one model), with zero findings at requested widths 1/4/8.
DiningPhilosophers matches 336 generated states, 67 distinct states and depth 29.
MCDistributedReplicatedLog now reaches the explicit VIEW graph-capture restriction.
Actual capture remains width 1; strict TLC temporal conformance is still pending.

## Symmetry safety increment and remaining contracts

The bounded model-value permutation kernel matches all 1,329 direct pinned
Java inputs in order, including malformed maps under TLC behavior. Checked
mode rejects non-bijections, with omitted identities included. The following
declaration and safety stages build on this kernel.


The S1 declaration stage now preserves the configured definition before constant
evaluation and validates pure enum-set/function-record representations. Four
early errors match pinned TLC messages; substitutions, assignments, identities,
effectful/state-dependent refusals and absent declarations have regression
controls. Supported pure source-backed warnings, model-value checks, canonical
keys and concrete trace handling now run through the checker.

The staged [safety study](../conformance/reports/studies/symmetry-safety.md)
fully agrees on all eleven intended symmetry targets and the additional Voting
selection at four workers. The original single-worker FastPaxos timeout remains
in its raw report. These 12/12 results are separate from the historical standard
83/71 totals above; neither report is rewritten or arithmetically combined.
The additional default-profile CheckpointCoordination and Voting study records
2/2 agreements. Both stores and profiles have pinned key/warning/trace controls
at 1/4/8 workers. CM1 now requires the original Paxos Voting symmetry case,
without removing any of its previous models: 21/21 agree at those widths.

Unverified startup exceptions, unsupported declaration shapes, deferred/lazy
or infinite permutations, randomness under symmetry and symmetry with liveness
remain named refusals. The optional symmetry audit is unimplemented. #104 and
ledger families F-017/F-018 remain partial; no complete-family certification
or throughput claim follows from these selected passing models.


The expanded [53-model final/short study](temporal-study.md) records 26 matching
final truth/count results, 22 named refusals and five 600-second timeouts. No
selected model is dropped; there are no disagreements or pending validations.
The completed checks use one actual worker. This separate evidence does not
increase strict temporal conformance. Ten finite-domain binding refusals and
five graph-capture timeouts give concrete language/scaling targets.
