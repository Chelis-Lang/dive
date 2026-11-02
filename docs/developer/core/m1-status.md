# M1 acceptance status (2026-11-02)

M1 has broad implementation across the main workstreams, but **acceptance remains open**.
The default engine remains `reference`. Use `10-plan.md` for the milestone
contract and this page for current evidence and the next bounded steps.

The current checkpoint delivers usable native finite modes and bounded safety
storage building blocks (`10-plan.md` §1). Compiled simulation/DFID (#730),
shared finite budgets (#725), exact safety/predecessor storage (#724), ordered
live-Store frontier spill (#729) and independent PlusCal translation/model
comparison (#728) are merged.
Final runtime main is `5688504ee3405f78b7951d6cab2b4b841ee549c8`, tree
`a8ab9e3bc576bc564cfd3fd1adb24db3e4d3621e`.
The one integrated full gate passes at this actual runtime. The fixed census,
both Auto baselines and both independent PlusCal populations are closed with
findings retained. Both actual-Core full-tier diagnostics are also closed and
recorded separately below.
**The bounded Phase 5 delivery, evidence archival, cleanup and independent audit
are complete, with findings retained.**
The [Phase 5 checkpoint](../../../conformance/reports/core/2026-11-02-phase5-checkpoint.json)
binds closed results to source `5688504e`; this bounded approval does not close
M1/M2, clean parity, global performance or RC acceptance.

ADR 0119 preserves default ≥2× TLC aggregate throughput and no slower panel
model at W1/W8; compatibility requires runtime/TLC ≤2 per model, investigated
above 1.5. Semantic, independent-word and resource obligations are unchanged.
The separate matched old-bridge #651 contract is now accepted at `ccff7729`;
the one indexed-guard S6 prototype is stopped unadopted. Neither result refreshes
later-source qualification or accepts the global performance panel. Historical
qualifications below remain attached to their actual sources.

### Phase 5: landed capability and closed execution stages

Native finite modes use Plan/Machine in both profiles with one actual worker.
Explicit Core never falls back; finite Auto uses isolated static preparation to
select a whole backend before execution. Reached native refusals never restart
effects. Shared simulation preflight refuses configured VIEW before assumptions,
output or execution. One budget owner covers assumptions, Init, steps and fresh
independent replay on both backends. Preflight dispositions retain priority;
actual budget exhaustion cannot certify a model verdict. Initial loading and
individual value encoding/decoding remain uninterruptible, and memory is sampled.
DFID preserves eager candidate/check/drain order and keeps only its current
batch's generator transients live until actual deferred demand, then reclaims
that batch after retaining required DFS rows. Simulation does not prove safety;
DFID depth exhaustion is not completion. Broader #111/#118/#128/#239 and #350's
unmeasured ≤1% instruction-overhead requirement remain open.

Exact native safety membership compares complete canonical identities while
keeping concrete representation and receiver history for execution/replay.
Ordered frontier spill stores paired canonical/original opaque rows under the
same live Store, validates retained records under control and decodes chunks of
at most 1,024 pairs. Its files are process-local, not checkpoints. Component
frontier/predecessor budgets exclude retained Store/history, seen membership,
candidates, arenas, decoded chunks and temporal graphs; predecessor close still
uses its existing record scan. Native checkpoint/recovery, disk seen, temporal
spill and complete accounting remain open. See the [storage guide](../../user/storage-recovery.md).

The PlusCal integration compares pinned `pcal.trans` → TLC with native
translation → dive in both profiles and label settings. It uses the ordinary
Reference checker backend, canonical report admission and independent native
words. Keep all 149 selected inputs and 59 configurations: missing configurations,
unsupported translations, startup/preflight matches, actual exploration, stops
and differences remain distinct. Startup agreement is not exploration; this
runner alone does not complete translation, native-core or broad API parity.

The [Phase 5 performance record](../../../conformance/reports/perf/2026-11-02-phase5-bridge-and-s6.json)
records two separate outcomes:

| Bounded assessment | Actual source and result | Claim boundary |
|---|---|---|
| #651 original CoffeeCan1000 old-bridge comparison | `ccff7729` versus bridge `b5f6af4c`, 24 children / four profile-worker cells / three repetitions; every pair passes runtime ≤1.10 and RSS ≤0.65. Median runtime ratios: default W1/W8 0.999226/1.006190, TLC W1/W8 0.994352/1.006049; median RSS ratios 0.589692/0.594620/0.598825/0.616372. #651 accepted and manually closed. | Matched VM old-bridge comparison, not stock-TLC timing, a quiet five-repeat panel or later-source execution. #650's already accepted original capacity remains a separate Phase 4 result. |
| #726 indexed-guard S6 prototype | `f60450d3`, one two-model default/W1 screen against a baseline with the disabled plan-dump diagnostic: median candidate/base 0.963964 SimpleRegular and 1.053560 Bakery. Stopped unadopted; no shipping code or rescue variant. | Does not reject broader S6 expensive-body/read strategies or qualify current performance. Existing history/demand safeguards remain mandatory. |

### Phase 5 source-bound qualification at `5688504e`

One `cargo xtask gate --full` invocation physically exits **0**, and its
postflight source check exits **0**. The report records **63m30s**; the outer
controller takes **3,812.810098 seconds**, including its own overhead. All
15 applicable steps pass, with one conditional unchanged-input cargo-deny skip
among 16 recorded steps. All **3,118 nextest tests** in **382 binaries** pass;
six tests are source-declared skipped and seven are reported slow. The nextest runner takes **1,358.946 seconds**; the gate's
nextest step takes **23m40s**, including its separate overhead. Those clocks are
not interchangeable. All **189 doctests** pass. Dedicated standalone trace
compilation takes **8.3 seconds**; its rlib SHA-256 is
`ab848970760ceef6177cf9994321fd18dc48390d811f2d2ed405eb07e177a34b`
(9,572,354 bytes). Its eight internal crates have empty feature lists, without
evaluator enumeration or forbidden backend edges. Reference CM1 (21 models) and
validation (16 models) pass at W1/W4/W8. Core/Reference shadows retain 57
agreements / six refusals in 63 CM1 rows and 250 / 62 in 312 small-tier rows.
Fresh full-gate content audit passes. These are actual final-runtime results,
not a rerun of an earlier qualification or final phase-preservation approval.

The fixed census retains 1,520 selected rows per profile, 99 parser exclusions
and 18 ambiguous skips. The census retains physical exit **1**. Independent
complete ID/status joins find **no gain or loss** against Phase 4:

| Profile | Shadow agree | Refused | Timeout | Unconfirmed words |
|---|---:|---:|---:|---:|
| default | 1,218 | 290 | 8 | 4 |
| TLC | 1,191 | 318 | 7 | 4 |

All 5,457 raw invocations have physical exit receipts; 17 incomplete child runs
earn no success credit. The four unconfirmed words in each profile remain
`core-liveness/GenError/{General,Nested,Record,Tuple}`. Unchanged fixture-shadow
agreements are neither new pinned-TLC classifications nor repaired findings.

Both Auto baselines physically exit **0**, without baseline regressions. TLC
keeps all 156 models: 150 agreements / six unsupported, plus **458/458 SANY**
agreements. Default retains 156: 144 agreements / seven registered expected
divergences / five unsupported, and intentionally omits SANY.

| Profile | Canonical actual Core | Static fallback | Dynamic fallback | Canonical route absent |
|---|---:|---:|---:|---:|
| TLC | 124 strict | 21 | 3 | 8 |
| default | 120 strict + 4 expected divergences | 22 | 3 | 7 |

Raw JSON headers instead report **124 Core / 27 static / 3 dynamic / 2 absent**
in each profile because the Unsupported mapper drops routing metadata in the
canonical harness view. Both views remain recorded. Missing metadata does not
establish that no search ran or no workers were used. Fallback/unsupported results gain no actual-Core success
credit. The canonical and raw routes are not pooled or silently reconciled.

The independent PlusCal runs are closed for **both label variants**, each
retaining **149 inputs, 59 same-name configuration slots and 90 inputs without
an original configuration**. Both physical population exits are **1**, and
`overall_accepted` remains **false**. Native translations run through the
ordinary Reference checking backend; these are independent pinned pcal/TLC
comparisons, not actual-Core population credit. Each profile in each label
variant has:

| Outcome | default profile | TLC profile |
|---|---:|---:|
| Completed success agreement | 20 | 20 |
| Matching independently validated violation word | 8 | 9 |
| Startup/configuration match, without exploration | 3 | 3 |
| Checker refusal | 10 | 10 |

Each variant also retains six **whole-pair 30-second helper timeouts**,
39 translation-unavailable inputs and 62 translated inputs without an original
configuration. Missing configurations, startup matches, unavailable translations,
refusals and timeouts earn no explored-model agreement. The unavailable set
contains 30 native-unsupported inputs in either variant, plus five both-rejected
inputs in default / four in label and four error mismatches in default / five in
label.
Those mismatches arise at the documented strict enclosing/output module-analysis
boundary. They do not demonstrate an emitter regression, earn no parity credit
and remain findings rather than automatic exemptions.

Default additionally produces a genuinely validated SBB `Immutability` word,
but its distinct/generated counts are **1,581/3,042** versus pinned TLC and
compatibility's **1,617/3,126**, at depth 15. Equal translated blocks and unchanged
configuration do not make that an agreement. The unregistered difference is
[#731](https://github.com/Chelis-Lang/dive/issues/731); it earns neither strict
agreement nor expected-divergence credit.

Approved **v4 postrun accounting** admits legitimate clean-check plus separate
trace-replay primary receipts without rerunning any model population. The v3
accounting rejection remains preserved, as do all **3,141 original raw/control
files** and **25 old accounting snapshots**. Corrected accounting is not a new
source execution or repaired population result.

Both fixed actual-Core full-tier diagnostics are terminal, accounting complete,
with physical exit **1**. Each retains **265 IDs: 217 in scope / 48 excluded**,
including nine missing references. Both use W1, jobs 2, native **60 seconds /
16 GiB** per check. Controller durations are **857.042642s TLC** and
**856.9595716s default**:

| Diagnostic outcome | TLC | default |
|---|---:|---:|
| Agreement | 143 | 139 |
| Registered expected divergence | 0 | 4 |
| Unsupported | 44 | 44 |
| Timeout | 21 | 21 |
| Oracle error / missing reference | 9 | 9 |
| Out of scope | 48 | 48 |
| Disagreement | 0 | 0 |

Actual-Core credit is **141 strict TLC** and **137 strict + four registered
expected default**. Two additional overall agreement rows per profile earn no
actual-Core credit; the TLC pair are frontend/engine-absent rows. Canonical
routing among 208 run rows records 141 Core / 67 without engine metadata in
each profile. All **205 actual raw children per profile** have physical exit
receipts; three manifest-level simulation refusals account for the other
canonical rows without launching children. Raw reports contain 203 Core headers
and two frontend failures per profile, with no Reference fallback; headers on
refusals and stops earn no success credit. Both profiles record
21 incomplete children, with no success credit. No unregistered full-tier
mismatch is observed; PlusCal's separate #731 finding remains unchanged.

These diagnostics are not the 900-second deep qualification, historical
Reference 182-model acceptance or M2 full-tier acceptance. Missing, refused,
stopped and unconfirmed rows earn no successful native-check credit; absent
route metadata does not establish that no work ran. All runtime campaigns are
closed. Independent review reconciles all 410 children, the exact original
selection and all 208 available pinned goldens at the executed source.

The runtime evidence archive is uploaded, downloaded and independently verified
across all **41,411 members** (41,410 payload files plus its manifest), SHA-256
`e6d7ce958775293268500e03e6d333832ba7dd87792cf64cc0ba34d67c52ec19`.
The separate verified support archive retains finished author/review records,
rejected attempts, source provenance and cleanup receipts; it excludes later
documentation updates. Both phase-owned instances are terminated and their root
volumes deleted. Qualification-node cleanup is verified at **01:56:32 UTC**,
with no active Phase 5 instance remaining and no fixed-deadline extension.
Conservative phase compute is about **$3.95**, excluding storage and transfer,
below the $40 ceiling and not an invoice. The compact checkpoint records the
archive hashes and final independent preservation decision. This closes the
bounded delivery and evidence work while retaining the parity, performance and
release obligations above.

### Historical Phase 4 qualification

Phase 4's qualified runtime main is `949e549cc1fece68cfa1bcfe59ea4b1c83bc368f`,
tree `37a77be0a70fe86dcaf4fce4617fa82690fe356b`. Its bounded trusted-report and
original-capacity deliverables are complete: standalone trace gating (#716,
closing #714), fresh invariant/VIEW certification (#717, closing #713), and
bounded native WF pair reuse (#719, closing #718), each independently reviewed
and passing all three required CI checks. Execution belongs to source
`95e73d2f05ba4af94ac6a2bca7f46fbc06d63a52`; the runtime squash has the exact
qualified tree and was not itself executed. The [phase 4 checkpoint](../../../conformance/reports/core/2026-11-01-phase4-checkpoint.json)
records accepted #650 capacity, retained census findings, preservation and
actual cleanup. That phase did not perform the later #651 bridge comparison;
M1/M2, global performance and RC/release acceptance remain open.

The historical frozen phase 3 runtime is
`fb48dbbf42bd224f278b138e74c1c8e29e2af106`, tree
`b2d90500b04efdd00f1c3bcac43dde43e4a0376a`. It includes native atom input-proof
reuse (#709, ADR 0134) and bounded ordered error-prefix replay (#712, ADR 0135),
each with fresh independent review and all three required CI checks passing.
The small-payload experiment #710 is closed unmerged after a valid negative
result. The [phase 3 checkpoint](../../../conformance/reports/core/2026-11-01-phase3-checkpoint.json)
records terminal qualification outcomes and their unmet acceptance criteria.
Evidence stays bound to its actual source and separate builds; documentation changes receive no new
execution credit. M1, M2 and release qualification remain open.

Focused candidate-error replay additionally covers typed assertions, division
and TLC arithmetic overflow only after actual controlled fresh word/diagnostic
execution. A different or unproved diagnostic lifetime now refuses without a
formal word or TLC class. Independent review rejected the initial 92-agreement
claim: two pinned minimal controls have opposite primary/diagnostic module
errors sharing physical 255. The corrected scope has 92 valid focused words,
64 strict classified-oracle comparisons and 28 explicit ambiguous non-agreements.
Only the owned2132/14 assertion classifier route is admitted; shared 255 streams
remain Unclassified (#697). The later bounded VIEW, constraint and invariant
proofs below do not reclassify those streams. Unproved receiver lifetimes,
temporal candidate scopes and initial-generation errors remain outside the
proved scope.

### Current checkpoint: implementation and remaining acceptance

| Area | Current state |
|---|---|
| Native modes and budgets | Compiled safety simulation/DFID and explicit CLI/API engine selection are implemented in both profiles; finite Auto is static only. Shared budgets cover production and independent replay. VIEW simulation refuses in every engine. Full mode matrices, generation/effects/workers and #350 instruction-overhead acceptance remain open. |
| Native safety storage | Exact membership, ordered live-Store frontier and predecessor spill are implemented. Files are process-local; component budgets are not total-memory limits. Native recovery, disk seen, temporal spill and complete accounting remain open. |
| Native temporal capture reports | Persistent original rows/CSR, controlled eager admission, bounded closed-function reuse, checked growth and actual budgets. Periodic stops report their real capture stage and completion facts (#675). |
| Independent PlusCal model comparison | #728 merged: pinned pcal/TLC against native translation/dive through Reference, fixed 149 inputs / 59 configurations, both profiles/label settings, independent native words. Missing configuration, preflight matches and actual exploration receive separate accounting. |
| Construction history | Owned literal/MAP/union construction order, raw demand and sparse state handoff are implemented with independent replay controls (#677, ADR 0131). Broader #623 remains open. |
| Default compound membership | The bounded worker-local index starts on the second eligible lookup in normalized sets of at least 16 members (#698). Original extensional comparison, ownership/phase admission, collision checks and miss/error behavior remain authoritative. It shares the memo budget; scalar queries, finite-fuel analysis, TLC comparisons and unproved values retain the original search. `DIVE_CORE_OPT=-member-index` restores that search. |
| Native temporal analysis | Exact reachable-product lookup is private; deterministic Cartesian ordering is restored before search (#694). Selected original-target descriptors remove repeated successor scans without changing original/stutter selection, conversion demand or error order (#701). These changes alone establish neither capacity nor throughput acceptance. |
| Native atom input proofs | Successful closed-input checks can be reused for exact retained rows within one analysis (#709, ADR 0134). Current Store ownership, poison, descriptor identity, conversion demand, fallback and run controls remain authoritative. Optional proof storage is capped at one MiB; declined storage retains the original checks. Counters establish actual reuse, not time saved. |
| Native WF result reuse | One privately authorized first frozen behavior WF action/ENABLED pair may retain completed fully native columns within one analysis (#719, ADR 0138). Optional storage is capped at `min(16 MiB, requested memory / 256)`; metadata, required reads/conversion, current controls, exact descriptor/Store/poison and row proofs remain authoritative. Single-property prefix analyses allocate no columns. Original twelve-cell capacity is accepted as #650; no whole-process memory or throughput acceptance follows. |
| Error reporting and trust | Fresh candidate-word and diagnostic proof precedes verified candidate reports (#699). Phase 4 adds a shared fresh source/diagnostic certificate for successor-invariant and v6 VIEW reports (#717, ADR 0136), closing coordinated primary-2111 class/header/physical-exit forgery #713 while retaining genuine 2111/2179/2103 and the caller's exit convention in both profiles. Initial context-only failures stay unverified (#702); proved first-Init receiver diagnostics retain `diagnostic_witness.trace` and a null formal counterexample (#683, ADR 0133). |
| Ordered error prefixes | Fresh bounded replay covers six source-final VIEW, two top-level non-Boolean constraint and two named invariant diagnostic families (#712, ADR 0135). The later #717 certificate extends the supported VIEW diagnostic lifetime and keeps bounded first-actual-source, record, multi-Init and effect restrictions. Native versions 1–6, report schema 3 and canonical classifications are unchanged; the ten ambiguous streams remain Unclassified. Broader receiver/source scopes still refuse. |
| Next receiver diagnostics | The bounded first-source route reconstructs one exhausted Init emission, no prior complete/partial candidate, and the actual ordinary/diagnostic function receiver before admitting 2147/75 (#703). Later sources, multiple emissions, effects and broader receiver lifetimes remain unproved; #680 stays open. |
| Temporal source errors | Version 5 independently replays the bounded one-state source, actual Next order, temporal row and diagnostic lifecycle (#679, ADR 0132). Later rows, multiple initial emissions and unproved scopes still refuse; broader #502 remains open. |
| Actual-word liveness names | Complete false-property list in configured order, independently proved on the actual word (#658, ADR 0126). Other observables remain strict. |
| Expected divergences | Multi-worker registry matching requires the exact complete observed generated-count pair when comparison omits that field (#695; closes #684). One-worker differences, exact exits and independent trace proof remain mandatory. Expected divergences are separate from strict agreements. |
| Unadopted optimizations | Indexed-guard S6 #726 is stopped unadopted after a Bakery regression, without rejecting broader S6. Projection reuse #705 remains closed unmerged after its SimpleRegular regression; #704 stays open. Small-payload scratch allocation #710 also remains closed unmerged after failing its predeclared paired adoption criteria; #708 stays open. None of these stopped experiments is part of the runtime. |

### Phase 4: bounded deliverables and original capacity accepted

The optional native WF pair candidate at `8e7dc78c`, tree `405e5d5f`, has
independent code approval and passes its original ABA TLC/W1+W8 adoption bar:

| Source / actual workers | Outcome | Controller seconds | Physical exit |
|---|---|---:|---:|
| baseline `b1a5972d` / W1 | complete success, above 810s | 888.542907478 | 0 |
| baseline `b1a5972d` / W8 | native time stop in atoms | 900.060743129 | 5 |
| candidate `8e7dc78c` / W1 | complete success, passes 810s | 435.507529242 | 0 |
| candidate `8e7dc78c` / W8 | complete success, passes 810s | 543.845765324 | 0 |

These are the original N4/T1/F1 configuration, TypeOK and all three properties,
seed/fingerprint zero, actual W1/W8, native 900-second/16-GiB limits and
930-second external backstops. Completed candidate rows retain 5,843,977 distinct /
85,612,896 generated / depth 45 / queue 0, capture and temporal exhaustion, and
zero native atom fallback. Two completed columns have accepted storage of
12,162,744 bytes below the applicable 16-MiB cap, excluding allocator overhead;
consumed reuse counters establish actual reuse, not
saved time. Fresh adoption review approves these measured `8e7dc78c` cells only.
All 38 baseline and 48 candidate archive members are verified. Baseline runner
exit 1, W8's censored stop and `quiet_window_certified: false` remain explicit; no W8 speedup ratio,
quiet-host throughput, memory saving or full twelve-cell acceptance follows.
The capacity record separately pins the historical TLC golden policy: one
worker, seed/fingerprint zero, final liveness checking, pinned JRE/TLC,
ParallelGC, Java 3-GiB heap / 8-MiB stack and a 600-second harness timeout.
Those oracle observables are not a fresh matched-host TLC timing or throughput
comparison; the actual native policy remains 900 seconds / 16 GiB at W1/W8.

The reviewed rebase `95e73d2f05ba4af94ac6a2bca7f46fbc06d63a52`, tree
`37a77be0a70fe86dcaf4fce4617fa82690fe356b`, preserves all thirteen reviewed
non-changelog files and includes landed #716/#717. PR #719 merged at 20:47:24 UTC
as runtime `949e549c`, with the exact qualified tree. One integrated
full-gate/census/Auto qualification launched at
18:56:34 UTC; one unchanged original twelve-cell capacity run launched at
18:59:40 UTC using its separate release ELF
`db17c8d3b5ecbc1f232df40cb12aec22d7b2cfcc1ee73c9995d7f350fd98c397`.
The actual `95e73d2f` matrix is terminal at 20:32:58 UTC: all twelve original
cells record complete temporal success and physical exit 0; its runner exits 0.
These controller seconds belong to that source and its actual capacity ELF:

| Original model | TLC/W1 | default/W1 | TLC/W8 | default/W8 |
|---|---:|---:|---:|---:|
| CoffeeCan3000Beans | 314.298 | 301.682 | 397.527 | 383.644 |
| EWD998Chan | 446.620 | 675.757 | 405.019 | 641.950 |
| aba_asyn_byz | 436.716 | 465.479 | 543.983 | 584.735 |

Original ABA TLC/W1 and TLC/W8 again meet 810 seconds, with JSON clocks
436.667s and 543.911s as well as controller clocks 436.715784933s and
543.982503086s. The renewed confirmation is separate from measured `8e7dc78c`.
No existing successful cell is lost. Root preservation verifies all 148 final
archive members (147 payloads plus manifest), SHA-256
`8d4cc56e14d0b342b715964eaf3e264ffe343b506cb94c6937a55a71fd257282`,
25,973,501 bytes. Fresh independent capacity review passes 694 checks with zero
failures and accepts the original twelve-cell #650 contract and bounded fairness
adoption. #718 and #650 were manually closed at 20:47:33 and 20:47:35 UTC after
verified merge and cleanup. Measured `8e7dc78c` results are never relabeled as
`95e73d2f` execution, and later runtime/documentation commits receive no new
execution credit. The [capacity record](../../../conformance/reports/perf/2026-11-01-phase4-original-capacity.json)
keeps every actual row, original input policy, physical outcome and source separate.

The fixture census is terminal at this exact `95e73d2f` source. It retains the
old 1,520 selected rows per profile, 99 parser exclusions and 18 ambiguous skips,
with no additional fixtures or inputs. Every old agreement remains agreement:

| Profile | Shadow agree | Refused | Timeout | Unconfirmed |
|---|---:|---:|---:|---:|
| default | 1,218 | 290 | 8 | 4 |
| TLC | 1,191 | 318 | 7 | 4 |

`ErViewAssert`, `ErViewDivZero` and `ErViewEnabled` gain shadow agreement in
both profiles; TLC also gains `ErViewOverflow`. These seven gains are separate
from the ten phase 3 gains per profile and do not reclassify canonical streams.
The remaining unconfirmed words in each profile are exactly
`core-liveness/GenError/{General,Nested,Record,Tuple}`. The census physically
exits 1; seventeen incomplete child runs remain fourteen core time stops, one
core memory stop and two core resource-limit refusals. Its ledger records all
5,457 invocation exits. No incomplete observation is a successful native check.
The prior TLC/W1 `LvPeriodic` difference is not repaired: phase 3's core
liveness violation versus reference graph-capture resource refusal now becomes
a core temporal-analysis time stop at 20,035ms, physical exit 5. This row earns
no agreement or fix credit. Existing [#664](https://github.com/Chelis-Lang/dive/issues/664)
tracks this unresolved asymmetry.
Zero loss of old agreements does not mean every old finding has been
semantically resolved.

TLC Auto physically exits 0 in 413.577s. It retains all 156 models: 150
agreements, six unsupported and no model differences; all 458 SANY comparisons
agree. Engine accounting remains 124 actual-core runs, three dynamic reference
fallbacks, 21 static reference fallbacks and eight without an engine. Default
Auto exits 0 in 418.440s, with 144 agreements, five unsupported, seven expected
divergences and no differences among its 156 models; default intentionally
omits SANY. Its 124 actual-core runs are 120 strict agreements and four expected
divergences, separately from three dynamic / 22 static fallbacks and seven
without an engine. Fallback earns no native-success credit. All 312 Auto
model-process receipts are present.

The census/Auto role is terminal at 19:21:20 UTC with physical/parallel exit 1
for census findings. Before/after source `95e73d2f`, tree `37a77be0`, remains
clean. The single full gate also terminates with physical exit 0. Its internal
report duration is 79m37s; the outer command controller records
4,780.746311389841 seconds including startup. Fifteen of sixteen recorded
stages pass, with cargo-deny skipped because its inputs are unchanged. All
3,066 nextest tests pass, six source-declared tests are skipped and seven are
reported slow. The nextest runner reports 2,498.862 seconds; its gate stage
also includes compilation and overhead. All 187 doctests pass.
Formatting, Clippy, rustdoc, layering and the dedicated standalone trace build
pass. Standalone compilation takes 11.5s, with eight allowed internal crate
fingerprints, no forbidden backends and evaluator enumeration disabled.

Reference CM1 and validation smoke pass at W1/W4/W8. Default core/reference
shadows retain 57 agreements / six refusals among 63 CM1 rows and 250 / 62 among
312 small-tier rows; refusals earn no native-success credit. Both small-corpus
baselines have zero regressions: TLC has 150 model agreements / six unsupported
and 458 SANY agreements; default has 144 agreements / five unsupported / seven
declared divergences. These are the full gate's own panels, separate from Auto.
The joined chain closes at 20:16:16 UTC with physical exit 1 for census findings,
distinct from full-gate exit 0 and both Auto exits 0. Fresh independent review
approves the closed local content while retaining those findings. Its
106,177,007-byte local-qualification/support archive is uploaded, downloaded and
root-verified across all 34,513 members at 20:24:08 UTC, SHA-256
`03c4721281a4aebf2a45bdfa097651ecf071a1ac0123fc6a61612d7d7dff71fd`.
The compact record pins the manifest and all four gate/census release ELFs.
The separate independent preservation supplement approves all 34,513 members:
34,512 payloads total 929,211,388 bytes, including the complete 32,300-file /
693,905,747-byte qualification tree. Archived old
reviews and support files keep their actual snapshot state; later documentation updates
receive no new execution credit and are not relabeled as part of that archive.

All five required archives were verified before termination was requested at
20:40:53 UTC. The owned capacity node is terminated and its root volume deleted,
confirmed at 20:41:27 UTC; no active owned phase instance remained in that
observation. The original 22:56:59 UTC deadline was not extended. Conservative
compute through observed termination is $2.60711180246 at the recorded
$0.714/hour rate, an upper-bound 3.65141709 hours below the six-hour limit.
This is not an invoice and excludes storage/transfer. Current cleanup remains
distinct from all historical phase receipts.

The value-proof lane is **STOP_BEFORE_IMPLEMENTATION**. Eight bounded local
cells yield 59 valid stack observations and do not establish material certifiable
repeated guard work for both Sailfish/default and Bakery/default. This is no
global absence or stopped-prefix correctness finding. The separate WF
diagnostic supports trying one bounded candidate: conservative direct-interpreter
observations are 347/2,972 TLC and 508/2,971 default after excluding Shape,
unknown-source and control call-site attribution. Most exported chains lack full
runtime ancestry. The initial stderr-lock instrumentation failed and receives no
credit; successful perf samples ended with model SIGINT, not semantic verdicts.
There is no precise removable share, ENABLED CPU share or whole-run savings
claim. No value-proof implementation, formal 300-row campaign or #705/#710
rescue follows, and M1/M2/RC acceptance remains open.

### Phase 3 local qualification at `fb48dbbf`

The single integrated full gate physically exits **0**. Its report records
**78m07s**; the outer command receipt records 4,690.533 seconds. All 14
applicable stages pass, with **3,054 nextest passes**, six source-declared
ignored tests and **185 passing doctests**. Formatting, Clippy, rustdoc and
layering pass; conditional cargo-deny is not applicable because its inputs are
unchanged. Reference CM1 and validation smoke agree at 1/4/8 workers. Default
core/reference shadows retain 57 agreements/six refusals among 63 CM1 rows and
250/62 among 312 small-tier rows. Explicit refusals earn no successful native
checking credit. The existing syntax/sema `acceptance` example output filename
collision warning remains recorded; it did not fail this gate.

The fresh independent census audit confirms the same **1,520 selected rows per
profile**, all 99 parser exclusions and 18 ambiguous-module skips as
`16095d43`. Every old selected configuration/module byte is retained. No old
agreement is lost against that source or the preserved `83356b01`, `9fc88333`,
`c21e5097` and `68dd8266` populations. Exactly ten formerly refuted words become
shadow agreements in each profile:

| Profile | Shadow agree | Refused | Native stop | Trace refuted | Unconfirmed | Differ |
|---|---:|---:|---:|---:|---:|---:|
| default | 1,215 | 290 | 8 | 0 | 7 | 0 |
| TLC | 1,187 | 318 | 6 | 0 | 8 | 1 |

The twenty repaired fixture/profile rows record independent **core-word**
validation. The forty actual engine/profile observations match the compared
observables and canonical counts/depth. Ordinary shadow comparison does not
separately replay reference words. All ten pinned oracle streams remain
Unclassified, so these are native proof and strict engine-shadow gains, not ten
new pinned-TLC agreements.

TLC `LvPeriodic` changes from a core time stop to the retained known difference:
a core liveness violation versus reference `resource-limit: behavior graph
capture states`. It earns no agreement. All **5,458 physical invocations** have
readable exit receipts, with no missing receipt, signal, malformed JSON or
physical/report exit contradiction. The 17 incomplete observations comprise
13 core time stops, one core memory stop, two core recursion resource refusals
and one reference capture resource refusal. Census exit 1 remains a finding.

Both small-tier Auto commands physically exit 0 and retain their 156-model
baselines. TLC has 150 agreements/six unsupported and retains all 458 passing
SANY comparisons. Default has 144 agreements/seven expected divergences/five
unsupported. Actual-core strict agreements remain 124 TLC / 120 default, with
four further default core expected divergences and three dynamic reference
fallbacks per profile. Static fallbacks and frontend results stay separate.
The census/Auto controller and joined chain retain exit **1** for census
findings, distinct from full-gate exit 0 and both Auto exits 0. Terminal
independent review approves the bounded local accounting and source binding,
while rejecting clean-census, milestone and release claims.

A separate fresh-target `cargo build -p dive-trace --locked` physically exits
0 in 11.64 seconds. Its reviewed normal dependency tree excludes `dive-check`,
`dive-liveness`, `dive-sim` and `dive-store`, and its feature/fingerprint records
exclude `dive-eval` enumeration. This requested compilation supplement is not
a stage of the 78-minute full gate. #714 tracks the difference between the
owner-provided explicit standalone-build requirement and frozen repository
AGENTS/xtask, which require dependency independence without listing that command.

The local raw archive is uploaded, downloaded and verified across all **21,924
members**, with separate gate/census ELFs, gate baseline reports, standalone
receipts and immutable reviews. Its 94,959,086-byte archive has SHA-256
`0ca65949ce48d52bfbb4ca5b7ce693a63757e039281238ad28782f79d5d18550`.
Preservation adds no outcome or acceptance credit. Historical passes below keep
their original source and scope.

The small-payload A/B is a separately reviewed **valid negative**. Its exact
48 native rows form 24 matched pairs across the unchanged four-model panel,
W1/W8 and three repetitions. Median candidate/baseline wall ratios at W1 are
1.003321809 for Sailfish and 0.996771165 for SimpleRegular, both missing the
required ≤0.95. CoffeeCan safety/W8 is 1.039337829, exceeding the ≤1.02 ceiling.
#710 is closed unmerged; no confirmation or compatibility-eight cohort was run.
The downloaded archive has all 60 members verified. Observed-window approval
retains the raw quiet flags as false and grants no five-repeat, TLC-throughput,
capacity or milestone credit. No rescue variant follows this rejected experiment.

One fresh same-host original ABA default/W8 pair compares `b2274f3d` with the
#709 candidate `a1e731d3`, using the unchanged N4/T1/F1 temporal configuration,
seed/fingerprint 0, actual eight workers and 900-second/16-GiB native budgets.
The baseline completes capture then stops during product analysis at 900.097s
with physical exit 5; the candidate succeeds at 836.981s with physical exit 0.
The candidate consumes 396,895,660 proof hits, 11,615,818 certifications/scans
and 730,498 peak retained proof bytes. These counters prove consumption, not
time saved. GNU-time peak RSS is 12,227,108,864 bytes for the candidate and
9,546,985,472 for the stopped baseline, both below 16 GiB. Their different
completion states establish no memory savings or exclusive allocation cause.

The focused controller itself exits 1: after both physical cells finish, its
archival step raises `PermissionError` reading root-owned
`ready-validation.json`. The historical nested tar is partial and is not the
final receipt. Later root preservation recovers the completed cell files,
actual ELFs and drained-service facts in the final 132-member archive, with
every member downloaded and verified. Fresh independent review passes 88
checks and approves that recovered bounded evidence, without approving the
failed controller as successful. This operations failure belongs to the focused
controller; the final twelve-cell launcher has its own control-tree ownership
preparation. The single completion observation has a censored baseline, so it
establishes no completion speed factor, quiet-host result or qualification of
the final source's twelve-cell capacity matrix.

### Phase 3 performance at `fb48dbbf`: valid diagnostics, targets unmet

Fresh independent audits approve the complete observed windows of two separate
cohorts: a three-pair, two-model focus with 36 correct rows/24 actual-core runs,
and a one-repeat, ten-model breadth diagnostic with 60 correct rows/40
actual-core runs. All native rows use the requested workers without fallback.
Their repetitions are never pooled. The focus records these median native/TLC
runtime ratios:

| Model | Default W1 | Default W8 | TLC W1 | TLC W8 |
|---|---:|---:|---:|---:|
| TLCSailfish1 | 1.109948 | 1.297839 | 1.509538 | 1.577358 |
| SimpleRegular | 1.366113 | 1.242777 | 1.430661 | 1.188877 |

All four default cells miss the speed floor. Sailfish's compatibility ratios
exceed 1.5 in every focus repetition at both worker counts, retaining ADR 0119's
investigation obligation; its medians remain below 2. The formal 300-row,
five-repeat campaign was **not started**.

In breadth, default is faster than paired TLC on six of ten models at each
worker count. Its descriptive equal-model throughput geometric means are
1.481826 at W1 and 1.483856 at W8, below 2. GameOfLife, Sailfish, Bakery and
SimpleRegular miss the individual floor at both widths. Compatibility
GameOfLife/W1 records **2.001332924**, above the ≤2 ceiling; this adverse single
observation is retained without rounding down or a repeatability claim.
Compatibility Sailfish/W8 records 1.551085975 and also requires investigation.
One pair does not establish the five-pair median condition.

All forty breadth native peak-RSS observations are lower than paired current
TLC; the largest native/TLC ratio is 0.2957185053. These are GNU-time process
RSS receipts, with TLC's 16g heap and native's 16-GiB whole-process budget
retaining different scopes. They are not allocation-time guarantees or original
temporal-capacity evidence. CoffeeCan uses the declared safety variant and
supplies no original CoffeeCan3000Beans temporal-property credit.

Both physical-window audits retain raw quiet flags as false and disclose
sampling limits, sticky preexisting power-log bits and normal kernel/MSR work.
The actual remote JRE/jar/cache receipt was captured after focus and before
breadth, at 14:09:23 UTC. Its matching pinned hashes are boundary evidence;
remote cache files are absent from the focus archive, so no continuous or
earlier cache attestation is invented. Focus's 58-member and breadth's
60-member archives are downloaded and verified independently.

The performance instance `i-0016fedfda67e43bc` is terminated and root volume
`vol-03fb9dabdd3513451` deleted, verified at 14:42:08 UTC after all three
performance cohorts were preserved. The separate capacity cleanup is recorded
below.
These results reject performance acceptance and grant no M1/M2 or release
qualification.

### Phase 3 original temporal capacity at `fb48dbbf`: eleven successes, one stop

All twelve unchanged original cells are terminal on the final source's separate
capacity ELF, under the original 900-second/16-GiB native budgets and
930-second external backstops. All four CoffeeCan3000Beans and all four
EWD998Chan cells succeed; the final EWD TLC/W1 cell completes at 461.819s.
ABA has three successes and one native time stop:

| ABA profile/workers | Outcome | Seconds |
|---|---|---:|
| default/W1 | success | 716.140 |
| default/W8 | success | 833.645 |
| TLC/W1 | success | 895.382 |
| TLC/W8 | time stop in atoms | 900.076 |

The stopped ABA TLC/W8 arm has physical exit **5**, completed capture,
5,843,977 distinct / 85,612,896 generated / depth 45 / queue 0,
`exhaustive: false` and no TLC success class. Its temporal execution stage is
**atoms**, distinct from the earlier focused baseline's product-stage stop.
Matching counts and completed capture do not establish temporal success. The
matrix runner physically exits **1**; **that source did not accept #650 capacity**.

The earlier `16095d43` matrix had nine successes. This final-source run recovers
ABA default/W8 and TLC/W1 completion, yielding eleven of twelve; TLC/W1 has
only about **4.618s** of observed wall-time margin to 900 seconds. Completion
with useful headroom and the remaining TLC/W8 cell stay acceptance obligations.
The historical `a1e731d3` single-pair observation is separate and supplies no
execution credit to this final matrix. None of these outcomes establishes a
quiet-host speed factor, complete memory accounting or memory savings.

The final 25,958,943-byte archive is downloaded and verified across all **147
members** (146 manifested files plus the manifest), SHA-256
`f653ac17247aeee60c97eb457087a995468479186d1f720ce6ea9146bbf5a700`.
It preserves the actual final-source ELF, controller, inputs and all terminal
cell receipts. Fresh independent review passes **188 checks with zero failures**
and approves the bounded final-source evidence while rejecting twelve-of-twelve
capacity and milestone/release acceptance.

After both focused and final capacity archives were verified, capacity instance
`i-0cac45996af611dc6` was terminated and root volume `vol-030fac13de8e74193`
deleted, verified at **15:55:47 UTC**. Both phase instances and their root
volumes are gone, with no active owned phase instance remaining. The original
fixed deadlines were not extended. Conservative compute through observed
termination confirmation is about **$10.235**, below the $40 ceiling; this is
not an invoice and excludes small storage and transfer costs. The exact
per-node estimate and cleanup receipts are in the checkpoint record.

### Historical local qualification at `16095d43`

The independently audited [integrated local record](../../../conformance/reports/core/2026-11-01-integrated-qualification.json)
records a full-gate pass, physical exit 0, in **53m22s**. All 14 applicable
phases pass: 3,036 nextest tests pass with six source-declared skips, 184
doctests pass, and formatting, Clippy, documentation and layering pass.
Conditional cargo-deny is not applicable because its inputs did not change.
Reference CM1 and validation smoke agree at 1/4/8 workers. Reference small-tier
baselines retain 150 TLC agreements/six unsupported and 144 default
agreements/seven expected divergences/five unsupported; all 458 TLC-profile
SANY comparisons agree. Default core/reference shadows retain 57 agreements/
six refusals among 63 CM1 rows and 250/62 among 312 small-tier rows. Refusals
are separate from successful native checking.

The complete fixture census selects **1,520 of 1,637 configurations**, with
the original 99 parser exclusions and 18 ambiguous selections. Old selection
rows and selected fixture/module bytes are unchanged; eight new configuration
IDs are counted separately. Relative to `83356b01`, no old agreement is lost;
10 default and 14 TLC old IDs gain shadow agreement. These are core/reference
shadow results, not pinned-TLC parity totals.

| Profile | Shadow agree | Refused | Native stop | Trace refuted | Unconfirmed |
|---|---:|---:|---:|---:|---:|
| default | 1,205 | 290 | 8 | 10 | 7 |
| TLC | 1,177 | 318 | 7 | 10 | 8 |

All **5,457 fixture-process invocations** have physical exit receipts.
Seventeen native observations are incomplete: 14 time stops, one memory stop
and two resource refusals, with the latter already included in refused totals.
The census correctly exits 1 for its remaining findings.

Auto retains all original 156 model IDs and compared observables in both
profiles. Actual core strict agreements are 124 TLC and 120 default; four
further default core rows are expected divergences. Each profile retains three
dynamic reference fallbacks; static fallback and frontend-only results also
remain separate from actual-core credit. The default Auto command exits 0.
The TLC Auto command exits **1** because `--no-sany` omitted all 458 baseline
module entries, despite zero model regressions. The full gate's SANY pass is
separate evidence; the failed Auto comparison is not an overall baseline pass.
The joined local chain exits 1, distinct from the full gate's exit 0.

The raw archive was uploaded, downloaded and verified across all 21,919 file
members. The record binds the independent audit, exact source and separate
gate/census ELFs. It grants no clean-census, M1/M2 completion or release credit.

### Historical performance diagnostics at `16095d43`: acceptance open

The [final performance record](../../../conformance/reports/perf/2026-11-01-integrated-performance.json)
retains separately audited focus and breadth diagnostics with downloaded,
member-verified archives. The two-model focus completes all 36 rows correctly,
including 24 actual-core runs with zero fallbacks or worker mismatches. Its
three paired repetitions have the following descriptive median native/TLC
runtime factors:

| Model | Default W1 | Default W8 | TLC W1 | TLC W8 |
|---|---:|---:|---:|---:|
| TLCSailfish1 | 1.10 | 1.24 | 1.51 | 1.61 |
| SimpleRegular | 1.36 | 1.12 | 1.41 | 1.14 |

All four default cells miss the per-model speed floor. The formal 300-row,
five-repeat campaign was **not started**. Compatibility factors remain below
2 in this focus only; Sailfish's factors above 1.5 retain their investigation
obligation. These three-pair observations supply no five-repeat acceptance.

A separate ten-model breadth diagnostic completes 60 correct rows, with 40
actual-core runs and 20 TLC runs, using one repetition. Default is faster than
paired TLC on six of ten W1 models and seven of ten W8 models; its descriptive
geometric-mean throughput factor is about 1.48 at each worker count. All 40
native rows use less peak RSS than paired TLC. These one-repeat observations
fall below the 2x throughput target and do not meet the per-model floor.
The largest compatibility single-pair factor is GameOfLife/W1 at 1.9716;
all observed compatibility factors are below 2, without establishing formal
five-pair acceptance. Factors above 1.5 retain their investigation obligation.
`CoffeeCan1000BeansSafety` is an explicit benchmark safety
variant and supplies no capacity credit for original temporal
`CoffeeCan3000Beans`.

Independent reviews approve bounded assessments of both complete observed
windows; the raw `quiet_window_certified` flags remain false and unchanged.
Neither cohort is pooled into a five-repeat or whole-phase claim. The
one-repeat breadth cohort does not qualify the ten-model panel. The earlier
permission-failed launcher ran zero models and retains no timing credit.
The performance instance is terminated and its root volume deletion verified.

### Historical original temporal capacity at `16095d43`: nine successes, three stops

All twelve original cells have terminal results under unchanged configurations,
properties, 900-second/16-GiB native budgets and 930-second external backstops.
All four CoffeeCan3000Beans and all four EWD998Chan cells succeed. ABA completes
only in default/W1:

| ABA profile/workers | Outcome | Seconds |
|---|---|---:|
| default/W1 | success | 846.981 |
| default/W8 | time stop in atoms | 900.106 |
| TLC/W1 | time stop in atoms | 900.070 |
| TLC/W8 | time stop in atoms | 900.064 |

Each stopped arm has physical exit 5, completed graph capture, 5,843,977
distinct / 85,612,896 generated / depth 45 / queue 0, `exhaustive: false` and
no TLC success class. Matching counts and policy checks do not prove temporal
success. The capacity service physically exits **1**, separate from the full
gate's exit 0. **That historical matrix did not accept #650.**

Original ABA default/W8 completed at `83356b01` in 889.180s and now stops at
900.106s: a required completion was lost. The old result had only 10.82s
(1.20%) nominal margin, which does not erase the new failure. Both campaigns
use c7i.4xlarge VMs with eight physical/16 logical CPUs, SMT on, matching inputs
and budgets. They ran on distinct VMs without certified equivalent runtime
conditions; source and host causes remain unproved. A stopped arm's lower RSS
cannot establish a completion-memory advantage over the older completed arm.

The raw archive is downloaded and verified across all 146 file members. Both
phase instances are terminated and their root volumes deleted, with no active
owned phase instance remaining. The independently audited [capacity record](../../../conformance/reports/perf/2026-11-01-original-capacity.json)
binds all twelve raw outcomes, the prior completion comparison and verified
cleanup. The audit confirms these bounded results and rejects capacity
acceptance. Complete store/native byte accounting
and the unproved semantic scopes above remain open. Historical measurements
below keep their original source and limitations.

### Next bounded actions

1. Build on the completed, independently approved Phase 5 checkpoint at
   source `5688504e`, with its findings retained. Preserve the fixed census,
   Auto/SANY selections and PlusCal inputs/configurations/labels/profiles,
   all physical outcomes, independent words and verified preservation/cleanup
   without projecting historical successes forward. Do not repeat the passing
   full gate or performance campaign for documentation updates. Development
   continues with focused checks.
2. Resolve #731's unregistered default SBB count difference under the existing
   parity/divergence policy. Independently prove
   `core-liveness/GenError/{General,Nested,Record,Tuple}`
   source/cause words. Keep #664's `LvPeriodic` asymmetry and #697's ambiguous
   shared-255 streams open: a stop or valid diagnostic is not an agreement or
   canonical deciding-class proof. Preserve #717's repaired forgery boundary.
   Extend #680/#502 receiver/source scopes and #623 constructor/history producers
   only through their recorded plans and demanded-lifetime proof.
3. Finish seeded generation, supported captured effects and complete
   simulation/DFID matrices, then justified worker/scheduler breadth. The native
   one-worker slice is useful implementation, not closure of #111/#118/#128/#239.
   Measure #350's ≤1% instruction-overhead criterion separately from functional
   cooperative budgets.
4. Build native checkpoint/recovery that preserves owned history/representation,
   followed by disk seen, remaining candidate/trace/temporal graph/SCC spill and
   complete memory accounting. Native retry-safe distribution depends on the
   identity-checked recovery contract. Java worker/checkpoint interoperability
   remains excluded; same-process frontier files do not satisfy recovery.
5. Complete remaining operators/configuration and actual-core full-tier breadth,
   explicitly granted environment/runtime services, coverage/debugger, public
   APIs/cargo-test integration and PlusCal obligations. These remain mandatory
   parity scope. LSP/native compilation and Verus export remain after v1 under
   ADR 0095 and the roadmap; do not expand that deferral to other product work.
6. Address the measured ADR 0119 default floors and compatibility outliers.
   GameOfLife/W1's historical above-2 observation and Sailfish's above-1.5 cases
   remain investigation inputs, not later-source measurements. Pursue material
   expensive-body/read reuse under S6 only with eligibility and demand proofs.
   Keep #704/#708 open; do not resurrect rejected #705/#710 or the single stopped
   indexed-guard prototype. #651 is closed, with no broad-panel credit.
7. At the release boundary, satisfy full-tier actual-core/fallback acceptance,
   unchanged deep frontend/corpus populations, supported package/platform/user
   flows, and the RC performance/qualification gates. Preserve the 8 implemented /
   27 partial / 3 missing family inventory until full-family evidence changes it.

Phase 4's bounded #650/#718 contracts and Phase 5's measured #651 comparison
are accepted on their own recorded sources. Phase 5's bounded delivery,
preservation, cleanup and independent audit are complete with findings retained.
Clean census, M1/M2, global performance, full parity and RC/release qualification
remain open. Historical failures, unclassified streams and source/ELF distinctions
remain evidence. The [roadmap](../../ROADMAP.md#through-full-tlc-parity) retains
the whole mandatory scope; this checkpoint is one substantial step within it.

### Historical local qualification at `83356b01`

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
Default records 1,191 agreements, 285 refusals, 21 refuted words, eight native
stops (seven time, one memory) and seven unconfirmed words. TLC records 1,161
agreements, 311 refusals, 24 refuted words, seven time stops and nine unconfirmed
words. The sole status
change is known TLC `LvPeriodic`: its earlier difference now reaches the
20-second temporal-analysis budget. That stop gains no parity credit.

All **5,437** physical invocations have exit receipts. Seventeen physical runs
are incomplete: 14 core time stops, one core memory stop and two core resource
limits; no reference
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

The [bounded historical profile at `cdaf7e13`](../../../conformance/reports/perf/2026-10-31-remaining-history-cost.json)
ranks the then-planned S5 normalized
membership index before S6 finer keys. Of 28 valid stack observations, seven
are membership/filter work and seven are construction projection/normalization;
other costs include hashing, scratch, interning and evaluation. This does not
assign exact time shares or justify removing history. S6's lost eligibility is
intentional until history-sensitive bodies are proved safe. That profile required
the proposed index to preserve admission and fallback, query demand, collision
checks and error behavior; it granted no authority to restore memo sites.

### Historical original temporal capacity at `83356b01`

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

### Historical integrated fixture census at `9fc88333`

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
Use the [current checkpoint](#current-checkpoint-implementation-and-remaining-acceptance)
and [active execution plan](10-plan.md#current-checkpoint-usable-native-modes-and-bounded-safety-storage-2026-11-02)
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
history. Follow the [current checkpoint](#current-checkpoint-implementation-and-remaining-acceptance)
and [active execution plan](10-plan.md#current-checkpoint-usable-native-modes-and-bounded-safety-storage-2026-11-02)
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
