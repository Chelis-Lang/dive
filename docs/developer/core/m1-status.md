# M1 acceptance status (2026-10-31)

M1 has broad implementation across the main workstreams, but **acceptance remains open**.
The default engine remains `reference`. Use `10-plan.md` for the milestone
contract and this page for current evidence and the next bounded steps.

The owner-approved next checkpoint is trusted core and temporal capacity
(`10-plan.md` §1). ADR 0119 keeps default throughput targets but replaces
compatibility's requirement to beat TLC everywhere with runtime/TLC ≤2 per
model at 1/8 workers, investigated above 1.5. Semantic, independent-word and
resource obligations are unchanged. Historical qualifications below remain
attached to their original sources; no acceptance is granted retrospectively.

The 2026-10-31 implementation checkpoint includes native closed-action evaluation
(#674), actual periodic capture metadata (#675), function-receiver error provenance
(#676), owned construction histories (#677), initial reference/Auto tuple/record diagnostic
replay (#683), and independently replayed post-Next temporal source errors (#679).
The version 5 consumer notice was posted before landing. These are bounded
capabilities with explicit refusal boundaries, not complete value-method or
liveness parity. Store/native byte accounting and broader history producers
remain unfinished.

Focused candidate-error replay additionally covers Assert/DivZero/Enabled at
constraint, action-constraint and implied-action sites, plus TLC arithmetic
overflow and the seen-target implied-action control. Native version 4 retains its
meaning. Exact typed primary/diagnostic replay and parser-owned implied-action
cause classification (#697) are checked on both engines/profiles at 1/4 workers.
These targeted controls do not refresh the source833 census totals below;
unproved receiver lifetimes, VIEW and temporal candidate scopes remain open.

### Current checkpoint: implemented versus unqualified

| Area | Current state |
|---|---|
| Native storage, budgets and capture reports | Persistent original rows/CSR, controlled eager admission, bounded closed-function reuse, checked growth and actual budgets. Periodic stops report their real capture stage and completion facts (#675). |
| Construction history | Owned literal/MAP/union construction order, raw demand and sparse state handoff are implemented with independent replay controls (#677, ADR 0131). Broader #623 remains open. |
| Error reporting and trust | Function receiver provenance (#676) and bounded first-Init reference/Auto tuple/record replay (#683, ADR 0133) are implemented; native comparison refusals remain. Initial diagnostic states keep a null formal counterexample and carry separate `diagnostic_witness.trace`; the earlier partial-Init `error_witness` route stays closed. |
| Temporal source errors | Version 5 independently replays the bounded one-state source, actual Next order, temporal row and diagnostic lifecycle (#679, ADR 0132). Later rows, multiple initial emissions and unproved scopes still refuse; broader #502 remains open. |
| Actual-word liveness names | Complete false-property list in configured order, independently proved on the actual word (#658, ADR 0126). Other observables remain strict. |
| Performance regression | Integrated `9fc88333` qualification exposed quadratic history interning (#686). Merged #687 changes only the private structural index key, preserving semantic hashes and history authority. Source-bound development and independent review restore original SimpleRegular counts; quiet performance and capacity must be measured on the corrected source. |
| Integrated qualification | The `9fc88333` census completed with findings; its full gate was stopped after the performance regression was confirmed. Its small Auto panel has one real model timeout in both profiles. These results are retained, not promoted to a passing release gate. The corrected base `83356b01` passes its full gate and both Auto baselines; census findings and remote/performance acceptance remain open. |

### Current local qualification at `83356b01`

The [source-bound local record](../../../conformance/reports/core/2026-10-31-local-qualification.json)
records a full-gate pass in **72m22s**: 2,997 nextest tests pass with six declared
skips, documentation/layering pass, CM1 and validation smoke pass at 1/4/8
workers, and the small corpus and existing shadow gates pass. Conditional
cargo-deny is not applicable because dependency inputs did not change.
The raw archive was uploaded, downloaded and verified member by member.
Gate and census builds keep their separate recorded ELF hashes.

The complete fixture census again selects 1,512 of 1,629 configurations,
retaining all 99 parser exclusions and 18 ambiguous selections. Both profiles
preserve every agreement and the exact non-agreement ID sets from `9fc88333`.
Default records 1,191 agreements, 285 refusals, 21 refuted words, eight time
stops and seven unconfirmed words. TLC records 1,161 agreements, 311 refusals,
24 refuted words, seven time stops and nine unconfirmed words. The sole status
change is known TLC `LvPeriodic`: its earlier difference now reaches the
20-second temporal-analysis budget. That stop gains no parity credit.

All **5,437** physical invocations have exit receipts. Seventeen physical runs
are incomplete: 15 core time stops and two core resource limits; no reference
run hits a resource limit. The census correctly exits 1 for its remaining
findings. The full gate itself exits 0; the wrapper's overall exit 1 is not
reported as a failed full gate.

Both original 156-model Auto profiles pass their committed baselines:

| Profile | Agree | Expected divergence | Unsupported | Actual core strict agree |
|---|---:|---:|---:|---:|
| TLC | 150 | 0 | 6 | 124 |
| default | 144 | 7 | 5 | 120 |

All 458 TLC-profile SANY comparisons agree. Default adds four core expected
divergences; they are not strict agreements. Each profile has three dynamic
reference fallbacks: MCPaxos and MCPaxosTiny on unproved nested raw equality,
and P5TLCGetSet on its nonsuccess route. Static fallback and frontend-only
agreements are likewise separate from native checking credit. SimpleRegular
now completes on the actual core without fallback in both profiles.

This is local correctness and breadth evidence on `83356b01`, not a release
certificate or qualification of later optimizations. Its [performance attempt](../../../conformance/reports/perf/2026-10-31-history-performance-diagnostic.json)
was stopped after 140 correct rows: the first three Sailfish compatibility/W8
runtime ratios, 3.742/3.642/3.515, force any completed five-pair median above the
2x ceiling. The remaining 160 cells are absent or interrupted; no five-pair
median is invented. Independent review also found unexplained turbostat pidstat records alongside
reserved-CPU fields, so no external quiet certificate is granted.

Two measured follow-up changes retain their own source and checks. Merged
#688 removes the redundant default normalized-membership class scan only after
actual ownership/phase proof; the complete local Sailfish comparison improves
48.10s to 35.06s with exact counts. #689 implements shared authority-table reads with
exclusive origin/snapshot writes and the same exact permission predicate;
initial full W8 comparisons improve about 2x in both profiles. Neither result
is quiet-host or whole-panel acceptance. The combined source `cdaf7e13` passes independent focused integration checks
and required CI, and is merged as `dfb251c0`. Its [two-model, two-repeat development check](../../../conformance/reports/perf/2026-10-31-focused-history-diagnostic.json)
completes all 24 rows correctly, with all 16 native runs
using the core and requested workers. Sailfish compatibility/W8 is 1.60–1.63x
paired TLC; default still takes 1.25x TLC at W1 and 1.42–1.47x at W8.
SimpleRegular default remains 1.13–1.35x TLC. These observations do not meet
the default speed floor, provide five-repeat medians or qualify the ten-model
panel. Reserved-CPU frequency telemetry is explicitly absent in this lean
diagnostic, and no quiet certificate is claimed. Old-source evidence is not
relabeled. Sailfish W1 peak RSS is about 150 MiB native versus 5.5 GiB TLC on these
observations. This is a selected-model memory result, not original temporal
capacity certification. The benchmark node is terminated and its root volume
deleted.

The [bounded current-source profile](../../../conformance/reports/perf/2026-10-31-remaining-history-cost.json)
ranks the already planned S5 normalized
membership index before S6 finer keys. Of 28 valid stack observations, seven
are membership/filter work and seven are construction projection/normalization;
other costs include hashing, scratch, interning and evaluation. This does not
assign exact time shares or justify removing history. S6's lost eligibility is
intentional until history-sensitive bodies are proved safe. The next change
must preserve current admission and fallback, query demand, collision checks
and error behavior rather than restore memo sites blindly.

### Original temporal capacity at `83356b01`

The [complete twelve-cell record](../../../conformance/reports/perf/2026-10-31-original-capacity.json)
retains independently audited outcomes and verified archival/cleanup. All
twelve original cells completed with unchanged model/configuration,
PROPERTY declarations, 900-second native time limits, 16-GiB requested memory budgets and
930-second external backstops. **Ten pass; the two ABA TLC-profile cells stop.**
CoffeeCan3000Beans and EWD998Chan N=3 complete in both profiles at W1/W8.
ABA N4/T1/F1 completes only in default mode. This matrix is capacity evidence
on a VM, not quiet throughput or qualification of later source `cdaf7e13`.

| Original model | Profile/workers | Outcome | Seconds | Final stage |
|---|---|---|---:|---|
| CoffeeCan3000Beans | tlc-w1 | success | 323.727 | complete |
| CoffeeCan3000Beans | default-w1 | success | 309.446 | complete |
| CoffeeCan3000Beans | tlc-w8 | success | 407.998 | complete |
| CoffeeCan3000Beans | default-w8 | success | 392.514 | complete |
| EWD998Chan | default-w1 | success | 702.035 | complete |
| EWD998Chan | default-w8 | success | 663.757 | complete |
| EWD998Chan | tlc-w8 | success | 424.082 | complete |
| aba_asyn_byz | tlc-w1 | time stop | 900.109 | product |
| aba_asyn_byz | default-w1 | success | 768.296 | complete |
| aba_asyn_byz | tlc-w8 | time stop | 900.105 | atoms |
| aba_asyn_byz | default-w8 | success | 889.180 | complete |
| EWD998Chan | tlc-w1 | success | 459.846 | complete |

Success requires verified TLC_SUCCESS, physical exit 0, exact counts/depth,
complete capture and exhausted analysis. The stopped ABA arms retain full
capture counts (5,843,977 distinct / 85,612,896 generated / depth45), but do
not establish temporal success. Default ABA/W8's 889-second result has only
about eleven seconds of margin and does not establish robust performance.
The recorded `product` stage spans construction and SCC/lasso search; the W1
stop does not identify a single hot instruction. W8 stops in atom analysis, so
product optimization alone is not proved sufficient.

### Integrated fixture census at `9fc88333`

This core/reference shadow census retains all previous fixture selections:
1,629 configurations, 1,512 selected, 99 parser exclusions and 18 ambiguous
skips. Each profile gains 20 agreements among previously selected IDs versus
`68dd8266`, and 60 versus `c21e5097`, with zero lost prior agreements against
either source. Fifteen new
selected configurations are accounted separately. These are shadow results,
not pinned-TLC parity totals.

| Profile | Shadow agree | Refused | Native stop | Trace refuted | Unconfirmed | Differ |
|---|---:|---:|---:|---:|---:|---:|
| default | 1,191 | 285 | 8 | 21 | 7 | 0 |
| TLC | 1,161 | 311 | 6 | 24 | 9 | 1 |

All 5,438 physical invocations have exit records. Seventeen physical runs are
incomplete (14 core stops, two core resource limits and one reference resource
limit). The existing TLC `LvPeriodic` capture-limit difference remains unwaived.
The newly selected InitialTool case exposes a pre-existing anonymous-error word
failure (#685); it was independently reproduced on the older `cb0e0e3f` binary.
No golden or validator was weakened to count it as agreement.

The [compact source-bound record](../../../conformance/reports/core/2026-10-31-superseded-local-qualification.json)
retains all findings and the downloaded/hash-verified external archive.

The original 156-model Auto panel at this source records 149 TLC agreements,
six refusals and one timeout; default records 143 agreements, seven registered
expected divergences, five refusals and one timeout. Both timeouts are original
SimpleRegular and motivate #687. Actual backend accounting is separate from
verdict totals: each profile uses the core on 123 models; three use dynamic
fallback. The TLC invocation also mistakenly omitted SANY against a baseline
containing 458 modules. Those 458 missing comparisons are a command defect,
not a frontend result; the raw failed comparison is preserved.

The superseded quiet-host performance run contains real native Mutex time
stops and slower Sailfish observations. It is a failed partial diagnostic,
not a throughput qualification. Its independent observer and raw archive are
preserved. The separate superseded capacity source passes all four CoffeeCan
cells and EWD default/W1; one subsequent EWD cell was interrupted and six were
unstarted. That node is terminated and its root volume deletion verified.
Neither campaign is relabeled as corrected-source evidence.

### Historical integrated fixture census at `68dd8266`



This completed core/reference shadow census is bound to exact runtime
`68dd8266740740bb6dce4f4d916b724762732a15`, tree
`47d18ebc5e39ed4a37c8fdbb7287e05aa784f7ac`: 1,614 configurations,
1,497 selected, 99 parser exclusions and 18 ambiguous skips. Relative to
`c21e5097`, all original selections remain, with 13 added configurations:
ten selected and three newly ambiguous configs (`ClosedFunctionsHolds`,
`ClosedFunctionsViolation` and `NoTemporal`). These are shadow counts, not a
TLC parity percentage or a refreshed historical corpus table.

| Profile | Shadow agree | Refused | Native stop | Trace refuted | Unconfirmed | Differ |
|---|---:|---:|---:|---:|---:|---:|
| default | 1,161 | 287 | 6 | 36 | 7 | 0 |
| TLC | 1,135 | 309 | 4 | 39 | 9 | 1 |

In each profile, 42 old selected IDs improved: 27 trace-refuted and 15
unconfirmed rows now agree. No old agreement was lost or moved to refusal.
New selected rows add five agreements/five refusals in default and four/six in
TLC. TLC `LvPeriodic` separately changes refusal to `differ`: the core reports
a liveness violation while the reference reaches its artificial graph-state
limit. The original early comparison earns no word-agreement credit and remains an
unwaived finding (#664); it is not a TLC-bug or parity claim. All 5,382 invocations have
physical exit records; the census exits 1, honestly retaining its findings.

Separately, the actual retained core TLC word now passes fresh CLI validation
against unchanged original source/config in both profiles (exit 0, logical
length 3). This proof uses exact `68dd` binary `8555de2d`, not the gate's
separate build; its actual core counts are 2,253,003 distinct, 4,503,005 generated,
depth 3,001. The verified archive binds that operation. It does not change the
original census difference/counts, waive #664 or establish TLC parity.

Remaining refuted rows include 24 underlying-code/deciding-wrapper header
mismatches and two named-invariant/anonymous-header mismatches per profile,
plus ten default/thirteen TLC terminal constraint/action/property replay
rejections. Seven/nine unconfirmed rows retain explicit temporal-Next, VIEW and
other ending refusals. These trace rows have no whole-checker observable
verdict difference, but matching results do not certify either verdict or word.
No missing/partial-word shape error remains in these reported findings. The
compact record preserves exact remaining IDs/reasons, changed old IDs and
source/ELF/report hashes. Raw census evidence now has a downloaded/hash-verified S3 archive, recorded
separately from earlier benchmark archives. Issues #434/#490/#499/#606/#607/#620 are not blanket-closed.

The full gate at `68dd8266` completed in 66m12s, exit 1: 13 phases pass,
one fails and cargo-deny is skipped because dependency inputs are unchanged.
The only failure is Clippy's combined-function 101/100-line limit. All 2,891
executed nextest tests pass, with six declared skips; 167 doctests pass.
Documentation/layering, CM1 (21 models) and validation smoke (16 models) pass
at 1/4/8 workers. Reference-engine small-tier TLC has 150 agreements/six refusals and 458 SANY
agreements; default has 144 agreements/seven registered divergences/five refusals.
There are zero baseline regressions or changes. Existing nonstrict shadow gates
pass: CM1 has 57 agreements/six refusals among 63 rows; small tier has 254
agreements/58 refusals among 312 rows (29 refused models at 1/4 workers), with
no mismatches or timeouts. Those refusals are not successful-checking credit.

Helper extraction `7adbdee0` merged as `a79a024d` without semantic or lint-allow
changes. Exact merged-source conformance all-target Clippy and all three required
CI checks pass separately (run `41070771048`). The completed `68dd` runtime
phases and separately bound repair checks are reused within those scopes;
the original failed command is never relabeled as a clean full-gate pass.
Only the after-build ELF hashes qualify `68dd`; before-build release hashes
belong to warm `1857` preparation. Census, gate and separate capacity binaries
retain their own hashes even when their source revision is the same.

The [compact development record](../../../conformance/reports/perf/2026-10-30-native-development.json)
preserves exact source/host/budget and archive bindings. Uncached `f8e627c4`
completed original CoffeeCan3000Beans at default/W1: 4,504,500 distinct,
18,000,002 generated, depth 1, 317.652 seconds and 4,667,887,616 peak RSS bytes.
Its original EWD998Chan default/W1 run stopped after 900.084 seconds in periodic
atom analysis: 792,928 distinct, 5,922,913 generated, depth 16 and 569,479,168 RSS
bytes. Pinned TLC on that host completed EWD in 608.358 seconds. The native run
included a ten-second 49-Hz attach sample; this is development evidence, not a
quiet performance comparison. After authentication renewal, original EWD at `7aa3d37a` also stopped:
900.055 seconds, 607,862,784 RSS bytes, 792,928 distinct, 5,922,913 generated
and depth 16. Recursive read reuse did not achieve the required completion.
The same reported checkpoint does not prove equal progress inside atom analysis
or measure throughput improvement. A separate sampled 60-second diagnostic
at `7aa3d37a` has 99.965% read-cache hits and 1.890 seconds in conversion;
reference predicate evaluation remains prominent in exclusive samples. This
does not establish inclusive call-chain costs or a shipped predicate change.
A predicate experiment is not shipped and earns no EWD completion credit.
Actual `a79a024d` ABA TLC/W1 stops after 900.063 seconds during atoms,
with capture complete: 5,843,977 distinct, 85,612,896 generated, depth 45 and
4,478,537,728 RSS bytes. Complete counts alone do not prove temporal success.

Original CoffeeCan3000Beans passes all four actual `a79a024d` capacity arms,
retaining 4,504,500 distinct/18,000,002 generated/depth 1:

| Profile/workers | Seconds | Peak RSS bytes |
|---|---:|---:|
| TLC/1 | 329.869 | 4,840,296,448 |
| default/1 | 314.796 | 4,828,897,280 |
| TLC/8 | 418.240 | 4,832,780,288 |
| default/8 | 400.356 | 4,827,889,664 |

Its binary is `add40aff`; ABA's separate node/build uses `9a0c215d`, both at
source `a79a024d`. Five of twelve matrix arms are observed: four successes
and the ABA stop; the other seven are pending. Capacity archives are only
scheduled, not yet verified. This remains incomplete capacity acceptance and
supplies no quiet five-repeat performance claim.

The one-repeat VM Coffee1000 screen preserves both experiments. Candidate
`e3f859e8` failed the time screen (ratios 1.122/1.138/1.129/1.142). Later
`1e02a967`, paired with old bridge `b5f6af4c`, passes all four development points:
time ratios 1.043/1.049/1.036/1.039 and RSS ratios .622/.627/.624/.603 in
TLC1/default1/TLC8/default8 order; all ten observations are correct. Neither
source earns integrated acceptance. New slow-path-only `convert_ms` excludes
one-variable cache hits and is not comparable with older all-read stage timing.

Remaining exit work is the original CoffeeCan3000Beans/EWD998Chan/aba_asyn_byz
matrix at both profiles and 1/8 workers under native 900-second/16-GiB budgets,
including every required observable and independently validated words; the quiet
ten-model five-repeat ADR 0119 panel; and repeated original-Coffee old-bridge
time/RSS comparison. Startup, scaling, whole-run memory/accounting and remaining
cooperative units stay explicit obligations. No new population, waiver, baseline
or default-engine switch is part of this update.

## 1. Source-bound qualification history

- Frozen `75f0ac4f769323de7be53a439ff5445ef4975b68` completes all 300
  both-profile five-repeat observations correctly, with actual requested workers.
  Paired TLC/core geometric means at 1/8 workers are 2.620916× / 2.678114×
  default and 2.209552× / 2.358382× TLC mode. This PRE-C2 population and its
  original runner quiet=false do not establish M1/M2 acceptance (§3).
- Main `2efcb3bf` merges #621 after required checks: unproved compound-set
  normalization now refuses in both profiles, fixing #619's default wrong
  success. Exact C2 normalization and broader comparison-error parity remain.
- Source `75f0ac4f` passed its own full gate in 40m05s (2,730 tests), external
  archive SHA-256 `830296f05b7f5eb46c639e67038473ff47b3c7c7c35288c3078609cd92724152`.
  That is PRE-C2 evidence. Integration `e0cce6f5` has separate fast/smoke/shadow
  and fresh-review evidence (§3), with no new full-gate claim.
- #620 triage at `eb91c523` covers 16 pinned-TLC controls and 192 API
  executions. Reference `tlc.verified=false` and the actual native reader's
  `Refused` outcome retain missing E7/Q3 diagnostic parity. This is not a
  falsely certified agreement or permission for a classifier/code override.
- Historical main `f1acc846` includes the `ENABLED` cache-boundary fix (#593), Init/Next
  ending fixes (#589), sampled-level fix (#601), named invariant-error trace
  validation (#604), and compound trace reconstruction without hash-consing
  (#605). The reference literal-bodied operator bug is fixed (#580), with
  a passing full gate and corrected checked example (#599).
- Required words must validate independently. The stricter harness (#594)
  rejects missing/unconfirmed words and matching unclassified outcomes;
  earlier measurements earn no retrospective credit through that change.
- The prior assertion that no wrong result was known was disproved by the
  `ENABLED` step-cache leak: it caused both false and missed violations.
  The repair has real path and source-mutation controls. Equality between
  two engines alone is not an independent correctness oracle.
- The default engine remains `reference`. No default flip is proposed while
  the acceptance findings remain. No new full-tier core result is claimed.

### Historical fixture baseline at `91742b3c`

Exact merged source `91742b3c9cb4b7021785975f888d5cf9dd43625c`, both
profiles, one worker, seed/fp 0, 32 sampled states, 20-second native limit
and 3-GiB ceiling. The runner selects 1,412 of 1,525 configurations, retaining
99 parser exclusions and 14 ambiguous modules. Both profile harnesses exit 1.

| Profile | Shadow agree | Refused | Native stops | Trace refuted | Unconfirmed | Successor findings |
|---|---:|---:|---:|---:|---:|---:|
| default | 1,047 | 269 | 6 | 63 | 27 | 0 |
| TLC | 1,023 | 286 | 4 | 66 | 33 | 0 |

All 1,363 previously selected configuration IDs retain their historical status.
There are 49 added configurations: 22 agree and 27 refuse in default mode;
21 agree and 28 refuse in TLC mode. No configurations were removed. The raw
agreement increase therefore comes entirely from the added population, rather
than repair of the old trace findings. Newly added refusal controls include the
conservative C2 normalization boundary; they receive no semantic parity credit.

This is core/reference shadow evidence, not a pinned-TLC parity percentage or
M1 acceptance. The shared gate host supplies no performance evidence. Compact
source/binary identities, complete totals and external archive hash are in
`conformance/reports/core/2026-10-29-fixture-census.json`; its comparison lists
the added IDs and separate population counts. Earlier evidence remains intact.
The approved execution order remains in §6 and `10-plan.md`.

### Historical final fixture census (2026-10-28)

Exact source `f1acc846885f6764d2ce3b40c8c757d9912b25c8`, both profiles, one
worker, seed/fp 0, 32 sampled states, 20-second native limit and 3-GiB ceiling.
The repeatable runner selects 1,363 of 1,476 configurations, with all 99 parser
exclusions and 14 ambiguous module selections explicitly retained.

| Profile | Shadow agree | Refused | Native stops | Trace refuted | Unconfirmed | Successor findings |
|---|---:|---:|---:|---:|---:|---:|
| default | 1,025 | 242 | 6 | 63 | 27 | 0 |
| TLC | 1,002 | 258 | 4 | 66 | 33 | 0 |

The job correctly exits **1**: this is not clean M1 acceptance and these are
not pinned-TLC parity counts. Against the original 1,361 common fixture IDs,
37 default / 38 TLC invariant-error findings and all three successor findings
per profile move to shadow agreement. Every other original fixture status
stays unchanged. The two added fixtures, LevelChain and CompoundTrace, agree
in both profiles and receive new-population credit only.

All native stop/resource-refusal identities and physical exits remain unchanged.
Remaining refutations include 50 wrapper/ending metadata cases per profile and
13 default / 16 TLC constraint/action cases. Unconfirmed outcomes include
17 / 23 final-expansion errors and ten missing native documents per profile.
Each remains a finding; no equality-only or metadata certificate replaces a word.

Source/binary identities, every non-agreement ID, complete accounting and the
external archive hash are in
`conformance/reports/core/2026-10-28-fixture-census-final.json`, with a per-ID
comparison beside it. The failed starting run remains intact at
`conformance/reports/core/2026-10-28-fixture-census-v1.json`.

### Fresh small-tier control

Exact source `0f0c6ad1`, one worker, all four small-tier corpora, seed/fp 0,
TLC-derived native budgets capped at 900 seconds and 8 GiB per run. Both
profiles complete with raw harness exit 0 and zero baseline changes. The
qualification is `conformance/reports/core/2026-10-28-small-auto-source0f.json`.

| Profile | Strict agree | Expected divergence | Refused | Strict agreements actually on core | Static / dynamic fallback | Early agreements |
|---|---:|---:|---:|---:|---:|---:|
| default | 144/156 | 7 | 5 | 121 | 23 / 1 | 2 |
| TLC | 150/156 | 0 | 6 | 125 | 22 / 1 | 2 |

Default's 125 core executions include four expected divergences; its 23 static
fallbacks include three. Thus 125 core executions is not 125 strict default
agreements. TLC also has 458/458 SANY agreements. There are no disagreements,
budget stops or pending required-word validations in these controls. This
establishes `auto` baseline preservation, not universal core parity. The fallback
rate still exceeds M2's 5% target.

Five refusals are common: MCDistributedReplicatedLog and EWD998ChanID need VIEW
quotient/closing-state evidence; SpanTreeRandom needs effects/dynamic-binding
support in temporal planning; TLC/TestGraphs needs effects in finite-binding
preflight; EWD998ChanTrace needs IOEnv. TLC also refuses P4SymLive's SYMMETRY
quotient liveness. Packed L5 storage alone cannot implement these semantics.

### Ablation matrix qualification

All twenty distinct combinations (ten real switches × both profiles) have
complete 156-model records. The source-bound matrix is
`conformance/reports/core/2026-10-28-core-ablation-matrix.json`.
At source `0f0c6ad1`, eight switches preserve normalized observables and actual
engine/fallback records. Disabling symbolic membership preserves auto outcomes
through five explicit >2^24-materialization fallbacks; those five are not proof
that the disabled core completed the models. Disabling hash-consing originally
lost ten invariant traces in each profile (#603). Corrected source `f582d791`
then completes both full populations with native exit 0, zero baseline/tuple/
engine changes versus all-on, and all twenty original failures repaired.
Per-case before/after observations are in
`conformance/reports/core/2026-10-28-hashcons-trace-repair.json`.

Original TLC model-only invocations mistakenly used the full SANY baseline:
their raw exit 1 is preserved as a protocol error. Separate model-only
comparisons retain every original 156-model record and observable, with exact
baseline hashes and negative controls; corrected continuation invocations use
that explicitly scoped input and exit 0. The full all-on TLC control separately
passes all 458 SANY modules. Snapshot-only duplicate tails receive no credit.
This is explicit accounting across recorded sources, not a claim that one final
executable ran every matrix slot or that the broader fixture census is clean.

### Trace candidate qualification

The qualification is `conformance/reports/core/2026-10-28-invariant-error-candidate.json`,
with an explicit per-ID comparison beside it. The complete census at candidate
`c831d615` has the same 1,361 fixture IDs and
identical fixture bytes as the starting run. All 37 default / 38 TLC-profile
invariant-error findings change to shadow agreement; every untargeted fixture
status remains unchanged. Default is now 1,020 agree, 242 refused, six stops,
63 trace-refuted, 27 unconfirmed and three sampler findings; TLC is
997 / 258 / 4 / 66 / 33 / 3. This candidate predates the sampler fix and is
**not a final-main run**. It adds no pinned-TLC agreement credit.

PR #604 passed its full gate (37m41s, 2,688 whole-suite Rust tests, both small
tier baselines and the required shadow selections) and a fresh independent
adversarial review. It uses trace version 2 only for named invariant errors,
with existing endings still writing version 1 and outer reports still schema 3.
It merged as `390882b9` after the required
[known-consumer notice](https://github.com/Chelis-Lang/oyster/issues/70#issuecomment-6475966845).
The later actual-main census is recorded separately; this earlier run keeps its
candidate source attribution.
The 50 residual ending-metadata findings per profile compare a reported
`TLC_NESTED_EXPRESSION` wrapper with an inner cause; they need separate typed
context and independent replay, not a blanket wrapper exemption.

## 2. What was built (about 70 PRs since #505)

**Values and store (stream 1).**
- Inline size-classed node columns (#510).
- Lazy function, set-predicate and union values (#517), and TLC's lazy
  values (#520).
- Integers of any size (#519, D-004) and model values (#533).
- `isNorm` and literal view bits (#535), and TLC intern-token string order
  (#526).
- Lock-free intern hits (#567).
- Library operators, through a standard-library hook (#554, #553 and #564):
  - Bags (#525, #554);
  - the TLC module (#556), with `Print`, `PrintT` and `ToString` (#568),
    `RandomElement` (#571) and `TLCSet`/`TLCGet` (#583);
  - CommunityModules `SequencesExt`, `Functions`, `Bitwise`, the folds,
    `FoldSet` and `FoldFunctionOnSet` (#561, #564, #569).
- Pure materialized sequence replacement with one scalar class across all
  arguments, with a bounded partition-definition oracle and independent
  trace validation (`03-value-model.md` §12). All overlaps, mixed, nested, lazy and effectful
  sequence forms remain refused; this is no new census or milestone credit.

**Compiler and laziness (stream 2).**
- TLC comparison errors (#509) and error classes (#531).
- The constant-fold bound, R9 (#512), and arena rollback, R10 (#514).
- INSTANCE substitutions (#521).
- Operator arguments (#539), lazy `LET` in split actions (#541) and
  conjoined initial predicates (#537).
- Lazy infinite unions (#544) and `UNION {S}` compiled as `S` (#559).
- A failing `ASSUME` is reported (#547).
- Faster rectangle-domain functions and small set differences (#578).
- `ENABLED` (#581).
- Memo tables: back on by default (#575, after #530 had turned them off) and
  bounded by the memory budget (#584).

**Generation and engine (stream 3).**
- The complete `getNextStates` rules (#508), traces (#511), error endings
  (#516), budgets (#518) and the parallel engine (#523).
- `--engine=auto` with fallback (#524, #549).
- `--compat=tlc` on the core at any worker count (#545, #548).
- Binding failures (#546), fingerprint errors (#536) and D-010 notes (#543).
- Implied properties (#560), SYMMETRY in both profiles (#563, #574), ALIAS
  (#565), VIEW (#570) and VIEW with SYMMETRY (#576).
- The string and transient memory fixes (#562, #566).
- Faster step predicates (#579).
- Liveness on the core (#572 design, #573, #577, #582). The core builds the
  behaviour graph, and the reference engine's `dive-liveness` and
  `dive-trace` check it, so verdicts and counterexamples are the reference
  engine's.

**Harness, correctness and bench (lead).**
- The harness runs on a chosen engine (#529).
- Shadow comparison of the core with the reference engine, in CI and the
  full gate (#534, #538), with successor sampling (#542).
- The x86 checkpoint and memo A/B (#552, #585).
- The AWS bench and gate scripts (`scripts/aws/`) and the shared gate lock
  (`scripts/gate-lock.sh`).

## 3. Performance measurements

### Completed checked-membership campaign (PRE-C2)

The frozen source is `75f0ac4f769323de7be53a439ff5445ef4975b68`.
All 300 planned cells are correct: ten explicitly selected models × six
TLC/default-core/TLC-profile-core arms at 1/8 workers × five repetitions.
Every requested worker count was used. The compact record is
`conformance/reports/perf/2026-10-28-core-checked-membership-x86.json`
(SHA-256 `9c69758013bc10915764cb23370cc0a285f76f5d410547ecc81eea51273373f1`).
Original runner `quiet_window_certified=false` remains unchanged; separately
observed preflight load, fixed-frequency, SMT/turbo and CPU/NUMA controls do
not replace that missing certificate. Data validation passed; qualification
remains incomplete and M1/M2 acceptance is not assessed.

| Profile | Paired TLC/core geomean 1w / 8w | Models faster than TLC 1w / 8w |
|---|---:|---:|
| default | 2.620916× / 2.678114× | 10/10 / 10/10 |
| TLC | 2.209552× / 2.358382× | 8/10 / 8/10 |

Selected median wall seconds and median paired scaling:

| Model | Profile | Core 1w / 8w | TLC 1w / 8w | Core scaling |
|---|---|---:|---:|---:|
| GameOfLife | default | 8.2430 / 1.5965 | 12.1021 / 2.5992 | 5.1478× |
| GameOfLife | TLC | 20.9598 / 3.2763 | 12.1021 / 2.5992 | 6.3970× |
| TLCSailfish1 | default | 43.8845 / 7.6144 | 51.3167 / 8.3403 | 5.7498× |
| TLCSailfish1 | TLC | 61.9935 / 9.9963 | 51.3167 / 8.3403 | 6.2063× |

GameOfLife and Sailfish remain slower than TLC in compatibility mode (#597).
For the large-state population, default/TLC scaling is only 1.307× / 1.334×
Bakery, 1.562× / 1.563× CoffeeCan and 4.061× / 3.865× Mutex. The all-model
scaling geomeans are not acceptance for that population. Memory and broader
capacity targets remain open. These timings precede #621 and do not validate
its C2 repair or a later executable.

Integration `e0cce6f534870898d261dddb7cc789a894429065` composes the five
performance commits onto `2efcb3bf`, preserving #621's C2 boundary and all
12 consumed cases. Its own complete fast gate passes 2,198 tests (34 declared
profile skips), smoke passes eight models at 1/4 workers, and strict shadow
passes 52 rows. Fresh review passes 891,600 differential comparison/cache
cases, 409 native runs and 22 validated counterexample words. Separately,
both-profile Sailfish 10k prefixes matched. Those are integration correctness
controls, not a new full gate or five-repeat performance population.

### Separate post-C2 Sailfish diagnostic

A separate source-`e0cce6f5` diagnostic uses the release executable with
SHA-256 `e0f365155354ffebe2d50d01374e390c9e71bded4c57e9c85148e7564e50182f`.
All three one-worker runs succeed with actual workers used 1, 109,604 distinct /
314,144 generated / depth 16. Wall times are 52.693730415 s pinned TLC,
45.938790702 s default core and 61.311964963 s TLC-profile core. Raw evidence
was archived and download hashes verified. The compact record is
`conformance/reports/perf/2026-10-28-core-post-c2-sailfish-x86.json`.
This is one repetition of three arms, with original runner quiet=false;
it is separate from the PRE-C2 300-row campaign and supplies no eight-worker,
whole-population, five-repeat or M1/M2 acceptance claim.

### Historical checkpoint (x86, c7i.metal-24xl)

| Model | ×TLC, 1 worker | ×TLC, 8 workers |
|---|---|---|
| MCbtree | 9.4 | 7.4 |
| MCDisruptor_MPMC | 5.1 | 4.7 |
| CoffeeCan1000BeansSafety | 2.3 | 2.5 |
| SimpleRegular | 2.0 | 3.2 |
| MCBakery | 1.8 | 1.7 |
| MCLamportMutex | 1.8 | 1.7 |
| EWD998PCal | 1.5 | 1.6 |
| GameOfLife | 1.5 | 1.6 |
| TLCSailfish1 | 1.2 | 1.06 |
| BufferedRandomAccessFile | not yet measured on x86 | — |

- **Sources.** Core times are from the memo A/B at `fdaf57a2`
  (`conformance/reports/perf/2026-10-28-core-memo-ab-x86.json`). TLC times
  are from the checkpoint at `f1ed932a`
  (`conformance/reports/perf/2026-10-27-core-m1-x86.json`), measured on a
  separate node of the same type, so the ratios are indicative.
- **Changes since those runs.**
  - #579 later cut EWD998PCal's refinement check from 14 s to 5.5 s.
  - #578 cut GameOfLife's cycles by 27%.
  - BufferedRandomAccessFile runs on the core only since #581: 10.5 s
    against 79 s for the reference engine on the M3, indicative.
- **Memory.** The core's peak RSS is far below TLC's: tens to a few hundred
  MB, against 0.8–1.5 GB. With memo on, 8-worker RSS roughly doubled on some
  models (MCLamportMutex 184 → 419 MB). The cap in #584 brought that down on
  the M3 (308 → 185 MB) but had not been checked on x86 at that checkpoint. The completed measurement below records the later cap and its remaining tradeoffs.

### Historical matched default campaign

At exact source `2d55317a`, the dedicated c7i.metal-24xl run completes all
300 planned rows: ten models × three arms (core/reference/TLC) × 1/8 workers ×
five repetitions. Every row is correct; every requested worker count was used.
SMT and turbo were off, frequency fixed at 2.4 GHz, CPUs 0–15 and NUMA node 0.
The original runner's `quiet_window_certified=false` is retained; machine setup
is not substituted for a missing certificate. Later main changes never relabel
the measured executable. CoffeeCan uses the exact pinned safety benchmark
configuration, not its original liveness property set.

Paired per-repetition ratios give geometric means of **2.649× / 2.645× TLC**
and **7.295× / 6.747× the reference** at 1 / 8 workers. All ten models beat TLC
at both worker counts, exceeding M1's default-profile throughput threshold for
this measured source. At that source the five-repeat TLC-profile campaign had not yet run;
one both-profile repetition exposed the Sailfish compat regression (#597).
The completed PRE-C2 source-`75f0ac4f` population above supersedes that
measurement status, without changing this historical source qualification. This is not full semantic M1 acceptance or M2 performance acceptance.

| Model | Core seconds 1w / 8w | ×TLC 1w / 8w | Core RSS MiB 1w / 8w | Reference RSS MiB 1w / 8w | Core scaling 1→8 |
|---|---:|---:|---:|---:|---:|
| GameOfLife | 8.117 / 1.622 | 1.49 / 1.64 | 51.6 / 141.6 | 83.9 / 90.1 | 5.01× |
| TLCSailfish1 | 43.380 / 7.890 | 1.19 / 1.05 | 74.3 / 184.8 | 627.3 / 643.2 | 5.50× |
| EWD998PCal | 11.152 / 2.349 | 3.76 / 3.29 | 39.2 / 172.1 | 142.8 / 162.1 | 4.75× |
| MCBakery | 6.010 / 4.606 | 1.84 / 1.70 | 123.3 / 216.2 | 231.5 / 241.8 | 1.30× |
| MCLamportMutex | 10.021 / 2.474 | 1.70 / 1.78 | 108.8 / 243.7 | 298.8 / 332.6 | 4.05× |
| SimpleRegular | 2.750 / 0.719 | 1.98 / 3.23 | 53.3 / 192.1 | 187.3 / 228.8 | 3.83× |
| MCbtree | 4.279 / 1.120 | 9.58 / 7.02 | 56.6 / 175.9 | 142.4 / 163.3 | 3.82× |
| CoffeeCan1000BeansSafety | 1.020 / 0.669 | 2.23 / 2.50 | 107.9 / 206.1 | 147.8 / 155.4 | 1.53× |
| MCDisruptor_MPMC | 0.894 / 0.343 | 4.97 / 4.66 | 35.8 / 88.1 | 53.1 / 65.3 | 2.61× |
| BufferedRandomAccessFile | 13.180 / 2.173 | 3.86 / 3.69 | 39.5 / 146.0 | 31.5 / 41.6 | 6.07× |

All core RSS peaks are below TLC's, but memory is not uniformly below the
reference engine: at eight workers, BRAF is 3.51×, GameOfLife 1.57× and CoffeeCan
1.33× reference RSS. For the ≥0.4-million-state population, measured core
scaling is 1.30× MCBakery, 4.05× MCLamportMutex and 1.53× CoffeeCan; the broader
ADR 0006 scaling target is unproven. These are remaining engineering targets,
not reasons to change observable results. The complete source/binary/pin,
per-repetition, CPU and RSS qualification is
`conformance/reports/perf/2026-10-28-core-m1-default-x86.json`.

Startup/fold observations at the same binary retain every sample: 30 fresh
processes per engine/profile, core medians 3.144 ms default / 3.194 ms TLC,
all 120 process times below 3.972 ms. This addresses the trivial-model 10-ms
startup target (R8) on this host, not cold-cache or arbitrary-spec startup.
CoffeeCan's one-state stopped prefixes report compile 1.132 / 1.066 ms and
fold 1.069 / 1.014 ms, with whole-command 5.805 / 5.578 ms. Those prefixes
establish no verdict. The bounded measurements address the specific R9 probe;
R10 memory tradeoffs above remain. Record:
`conformance/reports/perf/2026-10-28-core-startup-fold-x86.json`.

### Historical single matched compat-guard diagnostic

The unchanged prototype #598 and its exact parent (`1374df4e` / `e2183389`)
complete the original TLC-profile Sailfish model in 300.021 / 836.998 seconds,
with identical success, 109,604 distinct / 314,144 generated / depth 16. This
is a 2.790× improvement in **one matched pair**, with peak RSS
77,602,816 / 78,966,784 bytes. It confirms direction; it is not five-repeat
acceptance and is still much slower than the earlier approximately 52-second
TLC observation. The prototype stays draft, no further optimization was folded
into this tranche, and no new all-model compat speedup is claimed. Record:
`conformance/reports/perf/2026-10-28-core-tlc-guard-quiet-pair.json`.

## 4. Decisions taken in M1, and why

- **Memo on by default (#575).** I first held this PR. The M3 showed fewer
  instructions but equal cycles, and M0's four-model ablation had found no
  gain. The quiet x86 A/B then measured 1.16× at 1 worker and 1.23× at 8
  over 9 models, so it went in. Perf PRs are now judged by cycles or x86
  wall time, not by instruction counts alone.
- **Liveness pulled from M3 into M1, producer only (`11-liveness.md`).** Two
  bench models needed it. The core builds the graph, and the existing
  analysis and validator check it, so their results are proven ones. Phase
  2 now has native persistent rows/CSR and graph-neutral reports. Full byte
  accounting and TLC-relative capacity qualification remain open (§6).
- **`Print` and effects.** An effect bit keeps printing and register
  expressions out of folding, speculation and memoization. A spec that
  prints runs on one worker, so output comes in TLC's order.
- **`RandomElement`.** The draws come from a stream that the API supplies to
  the core, which needs no new crate dependency.
- **`ENABLED` inside an INSTANCE** supports constant/operator parameters
  and proved identity mappings of root variables. Hidden/nonidentity variable
  substitutions, initial queries, state-dependent configured operators and
  query effects/recursive/nested calls remain refused. Open question 8 (the B3 cutoff) keeps its default: TLC's cutoff in both profiles.
- **Persistent function trees deferred (#532).** The measurement was
  negative.
- **Where TLC is wrong (stale lazy reuse, B2 and B4),** the approved contract
  requires sound evaluation by default and TLC reuse under `--compat=tlc`
  (`02-semantic-contract.md` §3a). Hidden-query cache support remains pending;
  D-027 is registered when that code lands.

## 5. Historical acceptance inventory (superseded)

This inventory records the earlier checkpoint. Its counts and implementation
statements remain historical; they do not describe the current remaining work.
Use the [current checkpoint](#current-checkpoint-implemented-versus-unqualified)
and [active execution plan](10-plan.md#next-checkpoint-trusted-core-and-temporal-capacity-2026-10-29)
for landed scopes, remaining refusals and measured next priorities.

The census identifies distinct obligations; one fix cannot waive another family:

| Obligation | Evidence and next action |
|---|---|
| Sampled level context | Fixed in #601; the three original findings per profile now have source-level and independently expected-successor controls. The final actual-main census has zero successor findings. |
| Invariant evaluation-error words | Fixed in #604 for 37 default / 38 TLC cases, with exact named cause, prefix and failure-order validation. Other #490 families remain. |
| Other terminal error words | 50 findings per profile still compare a 2103 wrapper with an underlying ending cause (#606); classify each evaluation site before changing representation. |
| Constraint/action error words | 13 default / 16 TLC refuted words remain in the #499 families. Preserve actual terminal evaluation ordering; do not exempt the last state broadly. |
| Errors while expanding the final state | 17 default / 23 TLC words are independently unconfirmed; #434 needs an independent successor-generator strategy. |
| Missing words | Ten native documents per profile are absent for reported partial-state words (#607). Matching reports supply no substitute; independent generator/partial-state semantics need their own strategy. |
| Resource stops | Six default / four TLC runs stop with native exit 5. They remain incomplete. `ErDeepRecursion` separately returns a classified resource-limit refusal in both profiles. |
| M1 performance decision | Source `75f0ac4f` has 300 correct PRE-C2 observations, default 10/10 and TLC 8/10 faster at both worker counts. Original quiet=false and separate host controls remain; M1/M2 acceptance is open. |
| Compat performance | #597 remains: source `75f0ac4f` five-repeat Sailfish medians are 61.9935 s core versus 51.3167 s TLC at one worker; GameOfLife also remains slower. Keep PRE-C2 timing and later integration correctness evidence separate. |
| Exact comparison and normalization | #619 is fixed conservatively by #621. Raw order, duplicates, demand/history and alias lifetime precede the exact checked port; `Kind::Set`/`is_norm` and persistence are not certificates. #620 remains unverified/refused diagnostic parity. |
| Correctness coverage | Both small-tier auto controls pass with explicit engine accounting. The ten-switch matrix retains original failures, declared materialization fallbacks and separate corrected-source hashcons retests; see the recorded matrix qualification. |
| Startup and memory | Trivial startup and Coffee compile/fold measured above. Retain explicit eight-worker RSS and large-model scaling regressions. |

Known implementation gaps remain: `ENABLED` with hidden/nonidentity INSTANCE
variables or in an instantiated initial predicate,
SYMMETRY with liveness (`P4SymLive`), L5 graph capacity, `kSubset`, mixed,
compound, lazy and effectful sequence replace forms and overlapping All occurrences, `MapThenFoldSet`/`Folds`, TLC-profile `AllSubSeqs` membership and
the two printing refusals. L5 requires state storage as well as packed topology
(`11-liveness.md`); `kSubset` needs a symbolic value and startup/validator support
(`03-value-model.md`). Neither is a one-function completion.
Carry-forward defect #504 (failed ASSUME name) remains separate. The #515
reference instantiated-Naturals error is corrected by a bounded state-dependency
guard on opportunistic constant caching; explicit startup precomputation stays
unchanged. Its original pinned reproduction agrees on reference/core/auto in
both profiles at 1/4/8 workers. Eighteen additional consumed controls preserve
scope, demand, output and error order, but nested-scope core/auto disagreement
#634 remains a phase finding: its one-state violation word is independently
invalid with the repaired reference. This does not retire general INSTANCE or
whole-population obligations. Recheck #527 at `91742b3c` confirms the core already
handles empty-function length: MCFindHighest agrees with its existing pinned
golden on the core in both profiles at 1/4 workers (742 distinct, 1,523 generated,
depth 5). PR #533 had canonicalized empty function keys; no new runtime fix is
needed. Seven small oracle-backed length controls and unclassified non-sequence
rejection controls retain this behavior. This targeted result, recorded in
`conformance/reports/core/2026-10-29-empty-function-length.json`, does not replace
the historical full-population engine accounting above. The earlier checkpoint's approximately
11-billion-instruction MCLamportMutex increase was not causally attributed;
the new wall-time results do not retroactively explain that historical cost.

M2 still requires full-tier core coverage and at most 5% fallback. Default
throughput targets remain a 2× TLC geometric mean at both worker counts and
no slower benchmark; compatibility instead has ADR 0119's per-model runtime
ceiling of 2× TLC, investigated above 1.5×. M3 retains the
remaining execution modes, checkpoints and storage work. M4 default replacement
requires its own decision. Historical 182/217 full-tier agreements predate the
core reboot and do not establish current core breadth.

## 6. Historical execution order (superseded)

This is the earlier execution plan, retained as history. It includes work now
landed in bounded scopes, including first-Init receiver replay and construction
history. Follow the [current checkpoint](#current-checkpoint-implemented-versus-unqualified)
and [active execution plan](10-plan.md#next-checkpoint-trusted-core-and-temporal-capacity-2026-10-29)
instead of treating the instructions below as outstanding implementation.

1. Qualify the merged typed terminal replay over the complete selection, keeping
   uncovered sites explicit. Ordered Next/diagnostic and partial evidence are
   implemented through #659; valid evidence alone does not establish an unknown
   deciding class. Partial Init remains a separate error witness, not a formal
   behavior word. Exact fairness-primary #502 and #620 diagnostic parity remain
   open; do not exempt a final state or infer proof from matching metadata.
2. Qualify the implemented native L5 retention/source/CSR/report path: finish
   whole-run allocation accounting and bound remaining work while qualifying
   the implemented budgeted route. Primed SelectSeq is implemented; recheck
   original EWD998Chan temporal completion rather than treating operator
   admission or reduced-instance success as capacity. Preserve VIEW/symmetry/effects refusals and separate error-demand
   obligations. Use original temporal configurations for TLC capacity.
3. Retain explicit fallback and ablation accounting; broaden to the full tier
   on the core for M2 instead of promoting the historical reference result.
   Preserve the reference default while supported words remain unproven.
4. Deliver #623's consumed closed scalar/tuple literal lane after preserving
   covered construction order, duplicates, demand and alias history. Widen
   map/union/import/state producers and E7 provenance separately. Resume bounded
   stdlib and INSTANCE work from the measured refusal inventory; kSubset still
   needs symbolic/startup/validator support. #621 already prevents the known
   C2 false success, so the complete comparison port is not a prerequisite for
   every sound default optimization. General S5 indexing remains unbuilt.
5. Remeasure the final integrated source under ADR 0119. Retain source-75's
   PRE-C2 campaign and original quiet=false unchanged; acceptable compatibility
   deficits do not outrank correctness and capacity solely to exceed 1× TLC.

The corrective discipline is concrete: no invented agreement credit, no weakened
validator, no stale build identity, no source sharing between heavy jobs, no
performance inference from instruction counts or mismatched campaigns, and no
large graph rewrite before the existing state-storage boundary is explicit.

## 7. Working notes for the next machine

- **Workflow.** Work on a branch and PR in a separate worktree, with its own
  `CARGO_TARGET_DIR` per worktree.
  - Wrap heavy runs in `scripts/gate-lock.sh`. It is first-in first-out,
    with a separate lane for `gate --fast`.
  - Use focused checks with feedback in seconds to a few minutes, then required
    CI for incremental PRs. Keep a draft while required checks/review are pending.
  - Run the full gate on the integrated milestone candidate and before release
    qualification, recording its exact source. Do not repeat it for every edit
    or split implementation into extra unconsumed prerequisites (AGENTS.md).
- **AWS.** Nodes run on credits, are tagged `project=dive`, and are
  terminated after use, at most two at a time.
  - `scripts/aws/` needs an SSO `AWS_PROFILE` for the dive account.
  - Only benchmarks and gates run there, never agents.
- **macOS.** On the old Mac, macOS scanned every newly linked test binary,
  which turned a 20-minute fast gate into hours. Add the terminal under
  System Settings → Privacy & Security → Developer Tools.
- **Security.** An earlier bench bootstrap printed the GitHub token stored in
  the SSM parameter `/dive/bench/gh-token` into node logs and a local
  transcript. Rotate that token if it has not been rotated already. The
  scripts in `scripts/aws/` never print it.
