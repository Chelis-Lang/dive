# The new execution core

This directory is the design and working record for the new execution core of
dive: the value model, plan compiler, evaluator, successor generation and
exploration engine. It replaces the current engine for performance while
keeping TLC's observable results. The decision is ADR 0117
(`docs/decisions/0117-core-reboot.md`).

The documents exist so that any engineer or agent can take over at any point.
Read this page, then `10-plan.md` (status and handoff log), then the documents
for the workstream you are joining.

The current phase adds usable native finite modes and storage
building blocks: compiled safety simulation/DFID, exact safety membership and
ordered frontier/predecessor spill. Independent PlusCal translation/model
comparison (#728) is also merged. The default engine remains
`reference`; M1/M2 and RC/release acceptance remain open. Final Phase 5 runtime
main is `5688504e`, tree `a8ab9e3b`; all five implementation PRs passed the
three required CI checks before merge.

**Start here:** [m1-status.md](m1-status.md) separates landed capabilities,
source-bound evidence and the next implementation work. Actual source `5688504e`
passes its one full gate (physical/postflight 0; 3,118 nextest passes, six declared
skips, 189 doctests). The fixed census is unchanged at 1,218 default / 1,191 TLC
shadow agreements, with refusals/stops/four unconfirmed words per profile and
physical exit 1 retained. Both Auto baselines pass. Independent PlusCal retains
both 149-input / 59-configuration variants, real validated words, missing
configurations, strict qualification findings and default count difference #731;
both population exits remain 1. Both 60-second full-tier diagnostics exit 1:
actual-Core credit is 141 strict TLC and 137 strict + four expected default,
with refusals/stops/missing references retained and no deep/M2 acceptance. **Final archival, cleanup and qualification audit remain pending**
in the [checkpoint record](../../../conformance/reports/core/2026-11-02-phase5-checkpoint.json).
No M1/M2, clean-parity or RC acceptance follows.
Area guides describe current [finite modes](../../user/simulation.md),
[DFID](../../user/dfid.md) and [storage](../../user/storage-recovery.md).

ADR 0119 retains default throughput targets and bounds compatibility runtime
at 2× TLC per model, investigated above 1.5×. The separate matched old-bridge
CoffeeCan comparison accepts #651 at actual source `ccff7729`; the one S6
indexed-guard prototype is stopped unadopted after a Bakery regression. Neither
result qualifies the later runtime or the broad TLC performance panel. See the
[source-bound performance record](../../../conformance/reports/perf/2026-11-02-phase5-bridge-and-s6.json).

## Historical checkpoints before Phase 5

The following observations retain their actual sources and then-open findings.
Phase 4 subsequently accepted #650 and closed #713/#714; the current implementation
and acceptance boundaries are in `m1-status.md`. None of these historical full
gates, performance rows or cleanup receipts qualify the Phase 5 runtime.

The frozen phase 3 source is `fb48dbbf`, tree `b2d90500`. Native atom input-proof
reuse (#709) and bounded ordered error-prefix replay (#712) have fresh reviews
and passing required CI. Its [checkpoint record](../../../conformance/reports/core/2026-11-01-phase3-checkpoint.json)
records 1,215 default / 1,187 TLC fixture-shadow agreements, ten gains per
profile and no old agreement lost. All repaired core words validate, while the
ten canonical oracle streams remain Unclassified. Refusals, stops, unconfirmed
words and the TLC `LvPeriodic` difference remain findings. TLC Auto retains
SANY; both 156-model Auto baselines exit 0. The one full gate exits 0 in
78m07s, with 3,054 nextest passes, six source-declared ignored tests and 185
passing doctests. The census/joined chain retains exit 1 for its findings.
A separately reviewed standalone trace build exits 0 in 11.64s, with no
forbidden dependencies or evaluator enumeration; it is separate from the
full gate (#714). All 21,924 local archive members are uploaded, downloaded
and hash-verified.
The small-payload experiment #710 is a reviewed valid negative and is closed
unmerged. Fresh review approves recovered same-host original ABA default/W8
cell evidence after the focused controller's archival failure. Its candidate
completion has a censored baseline and does not qualify the final twelve-cell
capacity matrix, a speed factor or memory savings.
Final-source performance has independently audited 36-row focus and 60-row
breadth diagnostics, kept separate. All 96 rows are correct, including 64
actual-core runs. The default floor remains unmet; breadth's compatibility
GameOfLife/W1 ratio is 2.001332924, above 2, without a repeatability claim.
No formal 300-row campaign was started. Its archives are verified and the
performance node/root volume have verified cleanup. The final original
twelve-cell capacity matrix has eleven successes and one ABA TLC/W8 native
atom-analysis time stop at 900.076s, physical exit 5; its runner exits 1 and
the matrix is not accepted. ABA default/W8 succeeds at 833.645s and TLC/W1 at
895.382s, the latter with only about 4.618s of observed margin. All 147 final
archive members are verified. Both phase nodes and root volumes are gone, with
no active owned phase instance. Conservative compute is about $10.235, not an
invoice, excluding small storage/transfer costs and below the $40 ceiling.
#650 was not accepted by that Phase 3 source; Phase 4 later closed it.
M1/M2 and release qualification remain open. See the
[next bounded actions](m1-status.md#next-bounded-actions).

The historical [local record](../../../conformance/reports/core/2026-11-01-integrated-qualification.json)
at runtime base `16095d43` records a full-gate pass with 3,036 nextest tests,
six declared skips and 184 passing doctests; its whole tree was integrated as
`9a1d51a1`. That source's [performance diagnostics](../../../conformance/reports/perf/2026-11-01-integrated-performance.json)
miss the default floor, and its [capacity record](../../../conformance/reports/perf/2026-11-01-original-capacity.json)
retains nine successes and three ABA time stops, including a lost default/W8
completion. Its archive and node cleanup are verified. None of those historical
results or cleanup receipts qualify `fb48dbbf` or its current nodes.

The historical final fixture census on main `f1acc846` records 1,025 default /
1,002 TLC-profile shadow agreements and zero sampled-successor findings, with remaining
trace, refusal and stop findings. The completed source-`75f0ac4f` five-repeat
campaign has 300 correct rows:
default beats TLC on ten of ten models at both worker counts; TLC mode beats
it on eight of ten. Original runner quiet=false remains recorded. These are
PRE-C2 measurements, separate from later integration validation and milestone
acceptance; `reference` remains the default.

Main `2efcb3bf` includes #621's conservative C2 normalization refusal in
both profiles, closing wrong-success defect #619. Exact checked comparison
and normalization history remain M2 work. Source `75f0ac4f` and its full gate
(40m05s, 2,730 tests) precede that repair. Integration `e0cce6f5` separately
passes its fast gate (2,198 tests), eight-model smoke at 1/4 workers and
52 strict shadow rows; fresh review passes 891,600 comparison/cache cases,
409 native runs and 22 validated counterexample words; separately,
both-profile Sailfish 10k prefixes matched. See `m1-status.md` §3 for
attribution and the separate post-C2
three-row, one-repetition Sailfish diagnostic; it adds no population credit.
Bounded first-Init #620 diagnostic receiver replay is implemented (#683,
ADR 0133); broader receiver lifetimes remain unverified/refused. That later
repair does not refresh the historical measurements above.
M1 and M2 acceptance stay open.

The native temporal backend now retains persistent original rows and checked CSR,
uses controlled admission/on-demand reads with bounded exact-handle reuse for
closed functions, tuples and records with supported scalar leaves, and
releases native capture owners before fresh word validation. Every original
still receives controlled eager conversion. Ordinary core file requests use
actual run budgets and checked machine layouts; explicit Rust caller and
reference count limits remain. Whole-run byte accounting and TLC-relative
capacity/performance are still open. This does not alter the historical
measurements above or the engine default.

## Status board

| Workstream | Milestone | State | Branch / PR | Doc |
|---|---|---|---|---|
| Design docs (Phase 0) | P0 | done | #496 | this directory |
| Host-hook lock (#495), old engine | Day 0 | done | #497 | `01-motivation.md` |
| Rebaseline on x86 | Day 0 | done | #522 | `09-performance-method.md` |
| M0 spike | M0 | done: 1.81× TLC, 5.21× reference (x86 geomean); merged as `--engine=core` | #505 | `m0-results.md` |
| 1. Values and store | M1 | implemented M1 scope; remaining work below; done: inline node data, lazy values, big ints, model values, `isNorm`, intern-token strings, lock-free intern hits, Bags, most stdlib and CommunityModules operators, Print/PrintT/ToString, RandomElement, TLCSet/TLCGet, pure scalar sequence replacement (All overlaps refused), bounded owned literal/MAP/union histories and default S5 compound-membership indexing (#677/#698); next: broader construction, normalization demand/history and alias lifetime before widening checked comparison; conservative C2 refusal retained; retain §12 stdlib boundaries | `core/values-*` | `03-value-model.md` |
| 2. Plan compiler and laziness | M1 | implemented M1 scope; remaining work below; done: fold bound (R9), TLC lazy values, INSTANCE, error classes, operator arguments, lazy LET, failing ASSUME, `ENABLED` (#581), memo on and bounded (#575, #584); done: `ENABLED` with constant/operator INSTANCE parameters and identity variable mappings; bounded ordered VIEW/constraint/invariant diagnostic proof (#712), with canonical classes still Unclassified and the later #717 certificate closing primary-2111 forgery #713; next: raw construction/demand/history, exact scalar/tuple normalization and membership, then class/E7 widening; hidden/nonidentity INSTANCE queries and B2/B4 lifetimes remain | `core/compiler-*` | `04-plan-compiler.md`, `06-memoization.md` |
| 3. Successor generation and engine | M1 | implemented M1 scope; remaining work below; done: traces, error endings, budgets, parallel engine, `--engine=auto`, `--compat=tlc` at any worker count, implied properties, SYMMETRY (both profiles), ALIAS, VIEW, VIEW+SYMMETRY, liveness L1–L4 (#572, #573, #577, #582), #550, #551; L5 native retained roots/CSR, controlled eager admission/on-demand reads, bounded read reuse and graph-neutral validated reports; private exact product lookup/deterministic final ordering and selected original-target descriptors (#694/#701), exact-row atom input-proof reuse within one analysis (#709); native simulation/DFID (#730), exact safety membership/predecessor spill (#724) and ordered live-Store frontier spill (#729); next: native checkpoint/recovery, disk seen, temporal spill/accounting and remaining `P4SymLive` semantics; original twelve-cell capacity accepted on Phase 4 source, not rerun here | `core/engine-*` | `05-successor-generation.md`, `07-engine.md`, `11-liveness.md` |
| 4. Harness, shadow comparison, bench | M1 | Phase 5 full gate and Auto baselines pass; unchanged census findings and PlusCal #731/refusals/stops remain. Both actual-Core full-tier diagnostics are closed with findings; final archival/cleanup/audit remain pending, separately from acceptance; independent PlusCal comparison (#728) merged. #651 matched old-bridge runtime/RSS accepted only at `ccff7729`; indexed-guard S6 #726 stopped unadopted. Historical gates/census/capacity remain source-bound. Next: final preservation/audit, actual-core full-tier breadth, ADR 0119 deficits and release qualification; broader proof/accounting boundaries remain. | — | `08-correctness.md`, `09-performance-method.md`, `m1-status.md` |

The umbrella issue is #498. The M0 spike landed on `main` in #505; its results
are in `m0-results.md`. M1 streams 1–3 started 2026-10-26, each in its own
worktree, with module ownership as listed in `10-plan.md` §3. Update this table in every core PR.

## Documents

| Document | Contents |
|---|---|
| `01-motivation.md` | Measured evidence: profiles, benchmarks, why tuning the current design cannot reach ADR 0006 |
| `02-semantic-contract.md` | What the new core must reproduce, and what it may drop, each with evidence |
| `03-value-model.md` | Handles, persistent store, arenas, identity, lazy views, hashing, memory bound |
| `04-plan-compiler.md` | Binder handoff, static passes, demand-order laziness, specializations, interpreter |
| `05-successor-generation.md` | TLC's `getNextStates` rules as a specification; delta generation; dedupe before materialization |
| `06-memoization.md` | Memoized pure evaluation keyed on interned inputs: analysis, keys, soundness |
| `07-engine.md` | Breadth-first search, seen-set claims, determinism, traces, runtime names, scaling |
| `08-correctness.md` | Shadow comparison, mutation fuzzing, validator independence, fallback policy |
| `09-performance-method.md` | Benchmarks, hosts, ablations, recording format, gates |
| `10-plan.md` | Milestones, gates, workstreams, risks, decision and handoff log |
| `m1-status.md` | M1 summary at handover: what was built, results, decisions, open items, next steps |
| `11-liveness.md` | Liveness on the core: graph capture, conversion, analysis entry points, budgets, PR plan |
| `m0-results.md` | M0 spike: what was built, counts, timings, ablation, store scaling, watched risks |
| `tlc-baseline.md` | What TLC does per state, verified against the pinned TLC source |

## How to resume

1. Read the newest entry of the handoff log in `10-plan.md` and the status board
   above.
2. Check out the branch named on the status board in its own worktree, with its
   own `CARGO_TARGET_DIR` (see `AGENTS.md`).
3. Read the document for your workstream and its "Next step" section.
4. Before you stop, update that document's status and next step, the status
   board, and the handoff log. A PR that changes core behaviour without updating
   its document is incomplete.

## Rules specific to the new core

- **Contract first.** `02-semantic-contract.md` is the specification. If the
  new core needs behaviour that is not listed there, add it with evidence
  before you implement it.
- **Shadow comparison.** Every behavioural change is checked against the
  reference engine across the corpus (`08-correctness.md`).
- **Performance claims.** Every claim is a wall-clock measurement recorded with
  the method in `09-performance-method.md`. Instruction counts are diagnostic,
  not gates.
- **Unsafe code.** `unsafe` remains forbidden. A case for it needs measurements
  and an owner-approved ADR (`AGENTS.md`).
- **Frozen reference.** The reference engine (`dive-value`, `dive-eval`,
  `dive-model`, `dive-check`, `dive-sim`) receives bug fixes only.

## Glossary

- **Reference engine.** The existing evaluator and checker, frozen as oracle,
  validator evaluator and fallback.
- **Handle.** A 64-bit value word: inline scalar, or an index into a store or
  arena.
- **Persistent store.** The hash-consed, sharded store of values that are
  reachable from retained states.
- **Arena.** A per-worker bump region for transient values, reset after each
  state expansion.
- **Intern after claim.** A successor's values enter the persistent store only
  after its fingerprint wins the seen-set claim.
- **Lazy view.** The TLC representation class (interval, `SUBSET`, function
  set, and others) and normalization state of a set. It is kept beside the
  value's identity, because membership errors observe it.
- **Plan.** The compiled, slot-resolved, specialized form of the specification
  that the evaluator executes.
- **Position.** The (state, action, ordinal) key that orders successors and
  events deterministically (ADR 0015).
- **S1–S9.** The performance strategies listed in `01-motivation.md`.
