# Sprint status, 2026-10-09

M0 and M2–M4 are complete. The safety checker, independent trace validator
and deterministic parallel search are implemented and protected by the CM1
PR gate at 1, 4 and 8 workers. M1 and M5 remain open for the work below.
M6 has independent temporal-word evaluation and graph-search foundations;
`dive check` still refuses temporal model checking and symmetry by name.

## Latest strict model evidence

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
- **M5 storage/IR:** disk spill, bounded-memory frontiers/candidates, and
  the remaining IR cache work in the work-package plan.
- **M5 performance:** the benchmark runner is implemented; the recorded
  smoke timings come from a busy machine and establish correctness and
  reproducibility of measurement, not achievement of the performance
  targets. Profiling and paired baseline work remain (#28, #53).
- **M5 coverage:** unsupported library operators and deadlocks lacking finite
  independent covers remain honest limitations. M6 covers temporal and symmetry
  refusals; they must not be removed merely to raise the agreement count.

## M6 critical path

Follow [the M6 plan](design/m6-plan.md): temporal formula/tableau planning,
behavior-graph capture, then integrated accepting-lasso search and
property attribution. A bounded formula/tableau language kernel now checks
boolean combinations, always and eventually over supplied state/action atoms.
Pure bound predicates lower to that kernel with retained scopes and source
diagnostics and WF/SF enabledness expansion; dynamic contexts and TLC-exact
planning remain pending.
Independent fairness and successor-existence checking
(#63), exact TLC lasso choice, symmetry, scheduling, spill and performance
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
constructors, CLI integration and TLC-exact planning/selection remain pending.

Approved defaults are recorded in ADRs and the differences log: explicit
host-I/O grants, short deterministic lassos, unreduced symmetry/liveness,
rejection of malformed permutations, an opt-in symmetry audit, and separate
default/compat scheduling paths. They remain planned until implemented and
proven. See [the maintained TLC differences log](tlc-differences.md).


The next L1/L4 increment supports finite constant-domain quantifiers, tuple
bindings and inspected pure configured constants. Original configured PROPERTY
formulas now retain unambiguous claim names when the binder splits checking
roles. Five additional pinned probes match verdict truth/counts in both profiles
and stores. Full TLC finite-prefix class/trace timing remains open (#77).

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
