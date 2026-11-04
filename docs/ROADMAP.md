# Roadmap

Milestones match the GitHub milestones on `Chelis-Lang/dive`. Each milestone ends with:
- a review pass (a red-team agent or a human);
- a conformance report delta;
- ADRs for the decisions it made.

The charter's soundness contract applies throughout.

The [scoreboard](scoreboard.md) records the latest corpus results and the
roadmap feature checklist. Parity means observable results (ADR 0086).

## Phase 7: native workflows and bounded candidate transport

PlusCal publication (#747), native Graphs/UndirectedGraphs (#748/#759), owned
candidate payload spill (#750), the independent Rust consumer (#752), finite
random-source replay (#753) and native source-work observation (#755) are merged.
The explicit startup environment/first-TXT and offline-replay increment (#762)
is also merged. Integrated source `20ee7f75`, tree `b6625ae5`, passes its separately
dated November 4 replacement qualification: the full gate, retained census and
both fixed Auto panels. The archive was downloaded and verified before worker
termination. Reference remains default.

The first attempt remains rejected for preservation
([#763](https://github.com/Chelis-Lang/dive/issues/763)): wrapper exit 0 survives,
but its complete gate report, receipt/postflight proof and raw node archive do
not. A minimal runner repair passed 23 local fault/deadline controls and
independent review; the dated replacement also passed 14 affected window controls.
The [Phase 7 checkpoint](../conformance/reports/core/2026-11-03-phase7-checkpoint.json)
keeps both attempts and the locally preserved survivor archives distinct.

| Area | Delivered boundary |
|---|---|
| Core breadth | Six finite graph overrides run natively. Original YoYoAllGraphs completes in both profiles at `49188584`, matching the pinned 26,731 distinct / 48,535 generated / depth 39 success. Explicit-set demand and error precedence are repaired. The inherited lazy empty-product mismatch remains #760. |
| Candidate retention | Ordered Core safety queues can spill owned payloads and occurrences before speculative Store promotion. Both profiles, stores and widths 1/4/8 retain independent concrete words. Record/scratch/origin-index limits fail explicitly. Active Arena roots, accepted Store and temporal structures remain outside this component budget. |
| Native workflows | Filesystem PlusCal publication and actual cargo-test consumers work. Simulation carries owned RNG state from binding through repeated finite choices; independent replay rebuilds source alternatives. Native source counters use exact static origins and exclude private reconstruction/replay. These are not full modes, translation, TLC logical-cost or debugger parity. |
| External inputs | The #762 increment requires explicit category grants and records bounded raw startup assumptions. Offline CLI/API replay consumes fresh source requests without ambient I/O. Runtime reads, writes, exec, Core/temporal/recovery and unproved native name/history cases remain refused. |
| Performance | Candidate `88a5845a` is stopped unadopted. Thirty correct Sailfish rows miss the required 10% TLC/W1 gain (observed 1.82%) and fail the frozen default/W8 guard. The remaining 186 planned rows, including ablations and other controls, are unrun. No code, switch, default change or rescue ships. See [ADR 0147](decisions/0147-demanded-comparison-summaries.md). |

The accepted replacement's full gate records 3,246 test passes, six skips and
201 doctest passes. All 3,040 retained census IDs preserve their statuses and
required result fields; refusals and budget stops remain explicit. Both Auto
panels preserve semantic outcomes. YoYoAllGraphs alone moves from fallback
agreement to Core agreement: TLC actual Core strict credit is 125; default is
121 strict plus four expected differences. The 31 current additional fixture
configurations stay outside the retained census. The standard gate preserves
progress/summaries; the separate census and Auto stages retain raw streams and
876 required words, without claiming a fresh post-run replay.

The replacement archive contains 41,533 verified members and the worker/root
disk are confirmed gone. All milestone nodes are absent at the independent
observation. Recorded compute intervals total about $10.08; the conservative
compute bound through observed absence plus a $4 storage/transfer reserve is
about $24.15 against the unchanged $40 cap. These are estimates, not an invoice.
The rejected performance candidate and original feature controls keep their own
source identities. No full-tier, deep, formal performance, default-engine cutover
or release acceptance follows. Bulky evidence is archived in S3; local worktrees
keep active work and regenerable build products.

Continue the mandatory scope in [full TLC parity](#through-full-tlc-parity):
accepted Store/Arena and temporal paging/recovery, native distributed safety,
broader language/modes/effects, real coverage/Core source maps and full DAP,
remaining product/API flows, and Linux qualification before the macOS final mile.
Distributed pure safety can already build on ADR 0141's completed-level recovery;
it does not depend on completing temporal paging.

## Historical Phase 6: restartable Core, generation and evaluation reuse

All five runtime PRs (#738–#742) are merged at `ee79de82`, tree `14f02996`.
**The one full gate passed (3,164 nextest tests and 195 doctests). The fixed census and
both Auto panels are closed with no new regressions; refusals, timeouts and registered
differences remain separate.** The
[Phase 6 record](../conformance/reports/core/2026-11-02-phase6-checkpoint.json)
binds integrated results to that source; the separate
[performance record](../conformance/reports/perf/2026-11-02-phase6-scalar-specialization.json)
retains the unadopted candidate. Reference remains default and the 38-family
inventory remains **8 implemented / 27 partial / 3 missing**. This bounded phase
closes neither M1/M2 nor global performance or RC acceptance.

| Deliverable | Delivered scope and remaining acceptance |
|---|---|
| Restartable owned Core and disk | Fallible Core disk membership (#733/#738) runs with one actual worker. Completed-level Core checkpoint/recovery (#734/#741, merged `cd5b5e62`) preserves the owned Store DAG, registered construction authority, original/canonical discoveries, keys, predecessors and ordered frontier. Fresh-process recovery checks source/configuration/executable/startup identity. Both profiles, fp/exact, VIEW/safety symmetry, changed widths and existing spill/disk combinations are supported. Auto pause stays Reference; Auto recovery selects the encoded backend before work. Archive/component bounds are not total memory limits. |
| Constructive generation and sample proof | #735/#742, merged `ee79de82`, adds bounded successor traversal through `simulate --generate`, finite TLC `-generate num=N` transport and explicit sample validation. Domain evaluation and error order remain authoritative. Retained samples independently check Init/Next, constraints, invariants and implied initial/action properties. This proves the sample, not safety/liveness or TLC RNG identity. General RandomElement relations and broader effects remain open. |
| GenError and SBB | #737/#740, merged `e647c4ee`, independently certifies the bounded primitive Next-error word before a second temporal row completes. Broader #680/#502 lifetimes and Unclassified streams remain open. #731/#739 narrowly registers the source-proved D-010 SBB canonical-order count pair; strict TLC counts and independent words remain required. Historical Phase 5 census/PlusCal outcomes are unchanged. |
| Stopped scalar-fold experiment | `c4003fc6` remains unadopted/unmerged. Initial GameOfLife N=4 had three matched pairs/six correct rows and median candidate/base runtime 0.5002306 versus `b809a288`. The original-model panel completed 138 of 216 rows, then exited 10 at SimpleRegular TLC actual/requested W8: median 1.1794395975 exceeded the 1.02 control bar in two of three pairs. The remaining 78 rows were correctly unrun. Independent review verifies the receipts and archive; unresolved reserved-CPU observer activity prevents whole-panel quiet certification and leaves causality unproved. No cache, flag, default change or rescue ships. Broader S5/S6 remains open. See [ADR 0140](decisions/0140-certified-scalar-fold-reuse.md). |

Qualification consists of one full gate, the unchanged retained census and both
Auto baselines, joined by actual IDs/statuses, independently validated words and
physical receipts. New-feature checks remain separately identified. Retained
shadow agreement is not new pinned-TLC parity; Reference fallback earns no Core
execution credit. Phase 5 PlusCal/full-tier panels and original capacity were not
repeated. **The node and authored-source archives are uploaded and verified member by
member. Both allocated nodes and their root disks are confirmed gone; the independent
qualification data audit approves the bounded evidence.**

Continue through the substantial mandatory scope in
[current core actions](developer/core/m1-status.md#next-bounded-actions) and
[full TLC parity](#through-full-tlc-parity). Use focused tests and required CI for
development, with broad qualification at integrated milestones and releases.
See [storage/recovery](user/storage-recovery.md),
[simulation/generation](user/simulation.md#constructive-successors) and
[trace reports](user/reports-and-errors.md) for supported workflows.

## Historical Phase 5 checkpoint: native modes, bounded safety storage and PlusCal

Native simulation/DFID (#730) and shared finite-mode budgets (#725) are merged,
as are exact safety membership/predecessor spill (#724) and ordered live-Store
frontier spill (#729). Both profiles support the native finite slice with one
actual worker. Explicit Core never falls back; finite Auto selects a whole
backend before execution and never retries a reached refusal. Simulation VIEW
refuses in shared preflight. Reports still require independently validated words;
sampling and depth-bound DFID cannot certify safety.

Frontier files retain canonical/original rows under the issuing live Store.
They preserve concrete history but are process-local, not recoverable snapshots.
Component queue/predecessor budgets do not bound the retained Store, histories,
seen set, candidates or temporal graph. Core checkpoint/recovery, disk seen,
temporal spill and complete accounting remain separate implementation work.
The [storage guide](user/storage-recovery.md) states the current limits.

Independent PlusCal comparison (#728) is merged. It compares
pinned translation/TLC against native translation/dive through the Reference
checker in both profiles and label settings. All 149 inputs / 59 configurations
remain selected; missing configurations and startup/preflight matches earn no
explored-model credit. Full translation/API/corpus parity remains open.

Final runtime main is `5688504e`, tree `a8ab9e3b`, after all five implementation
PRs passed their required CI checks and merged. That actual source passes its
one full gate (3,118 nextest tests, six declared skips and 189 doctests; physical
and postflight 0). The census retains exactly 1,218 default / 1,191 TLC shadow
agreements, with unchanged refusals/stops/four unconfirmed words per profile and
physical exit 1. Both Auto baselines pass. The independent PlusCal populations
remain partially qualified: both 149-input / 59-configuration label variants
retain physical exit 1, valid words, missing configurations, refusals, timeouts
and the unregistered SBB count difference #731. A corrected postrun accounting
accepts legitimate clean/trace receipts without rerunning the populations.
Both actual-Core full-tier diagnostics are terminal, physical exit 1, under a
separate 60-second policy. Core credit is 141 strict TLC and 137 strict + four
registered expected default; refusals/stops/missing references remain. These
are not deep/M2/RC acceptance. **The bounded Phase 5 delivery, evidence archival,
cleanup and independent audit are complete**, with findings retained in the
[checkpoint record](../conformance/reports/core/2026-11-02-phase5-checkpoint.json).
Reference stays default; M1/M2 and RC/release acceptance remain open. See the
[current implementation and next actions](developer/core/m1-status.md#next-bounded-actions).

The separate [bridge/S6 record](../conformance/reports/perf/2026-11-02-phase5-bridge-and-s6.json)
accepts #651 only for actual `ccff7729` versus old bridge `b5f6af4c`: 24 children
cover four profile/worker cells with three repetitions and pass runtime ≤1.10 /
RSS ≤0.65. The one `f60450d3` indexed-guard S6 prototype is stopped unadopted
(SimpleRegular median 0.963964, Bakery 1.053560 candidate/base runtime), with no
rescue variant or rejection of broader S6. Neither assessment is execution of
the final Phase 5 source or broad TLC performance acceptance.

## M0: Foundation

Delivers:
- the workspace with one stub crate per charter component;
- pinned toolchain, workspace lints, `cargo deny`, and crate layering enforcement;
- lean CI (`ci.yml`): AI authorship guard plus workspace gate;
- the commit-msg hook, `AGENTS.md`, and the charter with its corrections appendix;
- GitHub tracking.

**Exit:** CI green on `main`, and an AI-trailer commit rejected by both the hook and CI.

## M1: Oracles and conformance harness

Delivers:
- pinned oracles: a snapshot of TLC 1.8.0 re-hosted in this repo's releases, plus v1.7.4; both use a Temurin JDK fetched by SHA-256;
- pinned corpora: tlaplus/Examples, CommunityModules, ValidationTestSuite, and the tlaplus repo's own tests;
- the `dive-conformance` harness:
  - oracle and corpus fetch;
  - TLC `-tool` output parsing;
  - golden generation with provenance;
  - per-model reports in JSON and `docs/conformance.md`;
  - worker-count invariance runs;
  - a cross-check against the Examples manifests;
- the survey of oracles and adjacent work in `docs/survey/`.

**Exit:** the first oracle-only report (TLC against TLC goldens and the Examples manifests) is committed, and every model's outcome is accounted for.

## M2: Competitive analysis

Measures tla2, tla-rs, tla-mc and tlago through harness adapters, and reads tlaplusplus's source (it is GPL, so reference only). Each is assessed on:
- conformance, including whether it honestly refuses what it doesn't support;
- performance: startup, states per second, memory, and scaling from 1 to 8 workers;
- Rust ergonomics;
- architecture.

Outputs:
- `docs/survey/competitive.md`, covering where `dive` can beat each tool;
- an ADR on positioning and which designs to borrow;
- baseline numbers in `dive-bench`.

This is not a build-or-buy gate: `dive` is built regardless.

## M3: Syntax and values

- **`dive-syntax`:**
  - a lossless lexer and CST with spans;
  - error recovery and stable error codes;
  - round-trip, property and fuzz tests;
  - parse acceptance matching SANY on every corpus module.
- **`dive parse`** with `--output json`.
- **`dive-value`**, in parallel:
  - canonical ordering, hashing and FP64-compatible fingerprints;
  - property tests.

## M4: Semantics and configuration

- **`dive-sema`:** SANY-equivalent acceptance and rejection, covering the module system, levels and the proof-language syntax, measured against the Examples and SANY test corpora.
- **`dive-config`:** the TLC `.cfg` format.

## M5: First verdicts

- **IR and evaluation:** `dive-ir`, `dive-eval` and the `dive-stdlib` core.
- **Safety checking:** `dive-check` breadth-first safety checking, in both exact and fingerprint modes. It is designed from the start for *deterministic* parallel search (ADR 0010): the first release may run one worker, but its architecture must already be level-synchronous with deterministic ordering, so that `--workers N` gives results identical to TLC's single-worker run.
- **Trace validation:** the `dive-trace` validator replays every counterexample.
- **Conformance gate:** the standard conformance tier becomes a PR gate on a small subset.

The design is in ADRs 0013–0016. The safety checker and parallel engine are implemented; storage, errors, performance and corpus follow-ups remain open.

## M6: Liveness, fairness and symmetry

- **Liveness:** explicit final/short checking (`--lncheck final --lasso short`) checks the supported pure temporal language, including weak and strong fairness, over the deterministic parallel safety graph. It selects deterministic lassos and validates them independently with `dive-trace`, including fairness and configured property attribution. Eligible ordinary TLC-profile liveness requests use this bounded final/short route; eligible ordinary default-profile requests check at deterministic completed-level growth boundaries (D-014). Unsupported language and request modes still refuse explicitly; full temporal parity remains open. TLC's internal tableau numbering, SCC selection and wall-clock scheduling are not parity obligations (ADR 0086).
- **Symmetry:** pure safety reduction now uses the bounded group and canonical keys in both stores, retaining concrete traces. `--compat=tlc` uses TLC's group, warning rules and representative order. Deferred/lazy permutations, unverified startup contracts and randomness remain partial. The intended default for symmetry with liveness is unreduced checking (D-005); TLC's behavior stays under `--symmetry-liveness=tlc`. Liveness symmetry modes and the symmetry audit remain planned and refuse explicitly.

The separate [symmetry safety study](../conformance/reports/studies/symmetry-safety.md)
records all eleven intended manifest targets plus the extra Voting selection
agreeing at four workers, with the original one-worker timeout preserved.
It does not increase the historical standard 83/71 report totals or certify
complete symmetry parity. The required CM1 gate now exercises symmetry-bearing
Paxos Voting at 1/4/8 workers without removing an existing case.

The design is in ADRs 0017–0019 (proposed).

## Through full TLC parity

The approved rescue roadmap in issue #241 retains the following mandatory
scope. Bounded native modes/storage and independent PlusCal comparison advance
these contracts without closing their full families. Granted startup inputs move
I/O to partial: the current inventory is 8 implemented / 28 partial / 2 missing
families; `implemented` is a supported
corpus claim, not full parity.

| Area | Work still required |
|---|---|
| Correctness and breadth | Complete broader constructor/receiver/source lifetimes, standard/Community operators, startup/configuration, symmetry/temporal language and actual-Core corpus coverage. Keep the narrow registered SBB difference distinct from strict agreements; preserve refusals and Unclassified streams until proved. |
| Modes | Extend constructive generation/sample proof to general RandomElement relations, captured effects, full simulation/DFID matrices and justified worker/scheduler options. Broad #111/#118/#128/#239 remain open; landed budgets do not measure #350's ≤1% instruction-overhead criterion. |
| Recoverable bounded search | Extend owned candidate/frontier/disk and Core safety recovery to accepted Store/Arena, trace and temporal graph/SCC paging, temporal recovery and complete accounting. Retry-safe native distribution depends on the identity-checked recovery contract. No total-memory bound follows from archive or component limits. |
| Product and integration | Explicit host category grants, runtime/introspection/output, trace exploration, coverage/debugger, complete PlusCal and checking/simulation/translation/checkpoint/distributed Rust APIs with cargo-test integration. |
| Performance and release | ADR 0119 default floors, compatibility outliers, full-tier actual-core/fallback acceptance, unchanged deep populations, supported platform/package/user flows and RC qualification. |

Build these increments on the existing compiled kernel and independent validator.
Use focused development checks, then one integrated milestone qualification;
large corpus/worker and formal performance campaigns belong at milestone/RC
boundaries. For performance, retain the sound history/demand safeguards, S5
membership work and S6 expensive-body/read strategy. The stopped indexed-guard,
projection-reuse and small-payload variants are not prerequisites to revive.

Native protocols/checkpoints satisfy the approved scope; interoperability with
Java workers/checkpoints and arbitrary JVM plugin loading are excluded. Host
access still requires explicit category grants even under `--compat=tlc`.
See [the scoreboard](scoreboard.md) and
[current core plan](developer/core/10-plan.md#1-milestones) for implementation and
acceptance boundaries. These requirements are separate from promoting the core
to the default.

## After parity

The empty `dive-lsp` and `dive-compile` placeholder crates are retired
([ADR 0095](decisions/0095-retire-tooling-placeholders.md)). Language-server and
native-compilation work remain deferred until after v1, when they return as
fresh implementations.

- **Integration:** LSP on the certified front end/checker.
- **Compilation:** native specification compilation and the Verus export path.

Public checking, simulation, translation, checkpoint and distributed library
APIs with `cargo test` integration are part of parity, not postponed here.

## Ordinary liveness integration

Final/short checking and independent validation already support finite
constant-domain quantifiers inside state/action predicates, scoped temporal
operators and pure WF/SF actions. One shared planner retains TLC's observable
conversion errors; the retained formula classifier covers TLC errors 2213 and
2214 for later integration. See [the temporal guide](user/temporal.md) for the
current language and explicit refusal boundary.

Eligible ordinary TLC requests use FinalShort under the request guards
described in the temporal guide. Eligible ordinary default-profile requests check
at deterministic completed-level boundaries with a geometric growth threshold
(D-014, issue #258). A prefix check may prove a violation, but only complete
exploration can prove success. Independent validation, named-property attribution, temporal
language coverage and unreduced symmetry checking remain acceptance requirements.

Ordinary core-engine temporal checking now retains persistent original rows and
checked CSR, admits original values in producer order, and reads evaluator
values on demand with bounded closed-function read reuse. Every original still
receives eager controlled conversion. Shared analysis yields owned
selected words and sealed complete/prefix facts; native capture owners are
released before fresh independent typed and serialized validation. Existing
reference APIs and explicit Rust limits remain; ordinary core-file count
ceilings now use checked layouts and actual run budgets. Whole-run byte accounting
and measured TLC-relative capacity/performance are still acceptance work; the default
engine remains `reference`. See [the core liveness contract](developer/core/11-liveness.md).

The removed Phase-L receipt, producer-clock, request-owned, hook and terminal
service diagnostics are historical. Ordinary liveness does not require matching
TLC's wall-clock thresholds, queue suspension order, tableau numbering or lasso
choice (ADR 0082, ADR 0086 and charter appendix 12). Checkpoint/recovery and other
modes retain their observable contracts, including equivalent resumed and
uninterrupted outcomes. Any new result-changing default still needs an approved
and documented divergence under ADR 0010.

### Historical Phase 4 qualification

Phase 4 completes its bounded trusted-report and original-capacity scope at
qualified runtime main `949e549c`, tree `37a77be0`. It includes standalone trace
compilation (#716, closing #714), fresh invariant/VIEW diagnostic certification
(#717, closing #713), and bounded native WF pair reuse (#719, closing #718),
each with independent review and the three required CI checks. Report schema 3, native
versions 1–6 and the canonical oracle classifications retain their meanings.
Ambiguous streams stay Unclassified. The [phase 4 checkpoint](../conformance/reports/core/2026-11-01-phase4-checkpoint.json)
binds actual execution to source `95e73d2f`; runtime squash `949e549c` has its
exact tree and receives no new execution credit.

The default value-proof candidate stopped before implementation after its
single bounded diagnostic. The separately reviewed native WF action/ENABLED
pair at measured `8e7dc78c` completes original ABA TLC/W1 and TLC/W8 in
435.508s and 543.846s under unchanged 900-second/16-GiB native limits. Both
pass the declared 810-second two-cell adoption bar. That permits integrated
qualification; it establishes no quiet-host throughput, broad performance or
full-matrix acceptance by itself. The separately executed `95e73d2f` matrix
supplies final capacity evidence and landed through PR #719 as runtime `949e549c`.

The terminal `95e73d2f` census retains its old 1,520-row selection per profile
and loses no old agreement: default now has 1,218 shadow agreements and TLC
1,191, with three/four VIEW-family gains. Four `GenError` words per profile
remain unconfirmed, refusals and stops remain explicit, and census exit 1 is
retained. All 5,457 invocations have physical receipts; 17 incomplete child
runs earn no success credit. TLC Auto exits 0 with 150 model agreements / six
unsupported and 458/458 SANY agreements. These completed stages do not establish
clean-census or milestone acceptance or erase fallbacks. TLC `LvPeriodic` changes
from the old core-violation/reference-resource difference to a core time stop;
it earns no agreement or semantic-fix credit, and
[#664](https://github.com/Chelis-Lang/dive/issues/664) remains open.
Default Auto also exits 0, with 144 agreements, five unsupported and seven
expected divergences; default's SANY omission is intentional. All 312 Auto
model-process receipts are present. The census/Auto role physically exits 1
for preserved census findings.

The one integrated full gate at `95e73d2f` physically exits 0, with report
duration 79m37s and outer controller duration 4,780.746 seconds. All 3,066
nextest tests and 187 doctests pass; six tests are declared skipped. Dedicated
standalone trace compilation, CM1/validation W1/W4/W8, both small-corpus
baselines and default core shadows pass. The joined chain retains census
finding exit 1; this does not become clean-census or milestone acceptance.
Independent review approves the closed local content and preservation of all
34,513 local/support archive members. That snapshot excludes later documentation
updates and supplies no execution credit to them.

All twelve unchanged original capacity cells at `95e73d2f` now record qualified
temporal success, physical exit 0 and no earlier success loss; both ABA TLC
rows meet 810s again. The final archive's 148 members are root-verified.
Fresh capacity review independently verifies all twelve cells, the archive,
unchanged policies and cleanup, passing 694 checks with zero failures. The
[capacity record](../conformance/reports/perf/2026-11-01-phase4-original-capacity.json)
preserves the stopped baseline and all actual final rows. All five required
archives were verified before actual node/root-volume cleanup, confirmed at
20:41:27 UTC with no active owned phase instance remaining. The fixed deadline
was not extended; conservative compute is $2.607, excluding storage/transfer,
not an invoice. Reference remains the
default; #650's original-capacity contract is accepted and #650/#718 are manually
closed after verified runtime merge. M1/M2 and RC/release acceptance remain open. No formal 300-row
campaign, value-proof implementation or rescue of #705/#710 is introduced.
Issue #650 has its own original twelve-cell capacity contract. Issue #651's
matched old-bridge CoffeeCan runtime/RSS criterion was not measured by Phase 4;
it is now separately accepted on actual `ccff7729` in the Phase 5 bridge record.
The wider milestone requirements remain open. No broader TLC replacement or performance
acceptance follows from this bounded phase.

### Historical trusted-core checkpoints

The historical trusted-core checkpoint includes bounded construction-history,
diagnostic receiver and temporal-source-row replay, native closed-action execution and
accurate periodic capture reports (#674–#677, #679 and #683). It also includes
bounded worker-local default compound-membership indexing (#698), private exact
temporal-product lookup with deterministic final ordering (#694), and selected
original-target descriptors (#701). The frozen phase 3 runtime `fb48dbbf`, tree
`b2d90500`, adds exact-row native atom input-proof reuse within each analysis
(#709, ADR 0134) and bounded ordered error-prefix replay (#712, ADR 0135).
Original comparison/history authority, fallback, demand, error order and
resource checks remain guarded. The [implementation and acceptance status](developer/core/m1-status.md)
records the current boundaries and preserves earlier source-bound results.

Candidate errors require fresh independent word/diagnostic proof; the focused
matrix has 92 valid words, 64 strict classified-oracle comparisons and 28
ambiguous non-agreements (#699). Worker-aware exact divergence facts close
#684 (#695). Context-only initial errors now refuse deciding-class proof and
omit their invalid anonymous word, closing #685 (#702). First-source Next
function receivers have a bounded actual-lifetime proof (#703); broader #680
remains open. The later ordered-prefix proof covers six source-final VIEW,
two top-level non-Boolean constraint and two named invariant diagnostic
families. Their canonical streams remain Unclassified, with no new
pinned-TLC classification credit. Version-6 advance
[consumer notice](https://github.com/Chelis-Lang/oyster/issues/61#issuecomment-6558501056)
was posted before landing. The inherited coordinated primary-2111 class/exit
acceptance remained outside that proof as #713. Phase 4's later #717 repairs
that defect; it does not reclassify the historical ambiguous streams.
Broader candidate classification (#697), history (#623), temporal source scopes
(#502), receiver lifetimes and initial-generation errors remain unfinished.

The [phase 3 checkpoint](../conformance/reports/core/2026-11-01-phase3-checkpoint.json)
records the audited unchanged 1,520-row census selection per profile: 1,215
default / 1,187 TLC shadow agreements, ten former refuted words repaired per
profile, zero refuted words and no old agreement lost. Refusals, stops,
unconfirmed words and TLC `LvPeriodic`'s known core-violation/reference-resource
difference remain findings. The twenty core words independently validate;
ordinary shadow comparison does not separately replay the reference words.
Both 156-model Auto baselines exit 0; TLC retains all 458 passing SANY
comparisons. The single integrated full gate physically exits 0 in 78m07s,
with 3,054 nextest passes, six source-declared ignored tests and 185 doctests.
The census/joined chain retains exit 1. A separately reviewed standalone trace
build exits 0 in 11.64s without forbidden dependencies or enumeration; #714
tracked the frozen gate's omission of that explicit command, since fixed in
the current gates by #716. This old supplement is separate from
the full gate. Local preservation is uploaded, downloaded and verified across
all 21,924 archive members.

The small-payload scratch experiment #710 is a reviewed valid negative and is
closed unmerged; #708 remains open. Its 48 native rows miss both required W1
improvement floors and exceed the CoffeeCan safety/W8 regression ceiling.
No confirmation, compatibility-eight cohort, five-repeat or capacity credit
follows. One same-host original ABA default/W8 candidate completes while its
baseline stops at 900 seconds; that censored observation establishes no speed
factor, memory savings or final twelve-cell capacity qualification. Its recovered
cell evidence is approved, while its failed archival controller remains failed.

Final-source performance has separate independently audited focus36 and
breadth60 diagnostics, all 96 rows correct/64 actual core. Default misses all
four focus floors; breadth is faster on six of ten models at each width with
descriptive throughput geometric means 1.481826/1.483856, below 2. The exact
single breadth compatibility GameOfLife/W1 ratio **2.001332924** exceeds 2;
Sailfish observations above 1.5 retain investigation. No formal 300-row campaign
or pooled repetition credit follows. All forty breadth native peak-RSS
observations are lower than paired TLC, with their different budget scopes and
single-observation limits retained. Archives and performance-node/root-volume
cleanup are verified. Final original capacity is terminal at eleven successes
and one ABA TLC/W8 atom-analysis time stop at 900.076s, physical exit 5; the
matrix runner exits 1 and capacity is not accepted. ABA default/W8 succeeds
at 833.645s and TLC/W1 at 895.382s, with only about 4.618s of observed margin
for the latter. All 147 final archive members are verified. Both phase nodes
and root volumes are gone, with no active owned phase instance and no deadline
extension. Conservative compute is about $10.235, below the $40 phase ceiling;
it is not an invoice and excludes small storage/transfer costs. #707/#711 are manually closed
after verification; #708/#713/#714 and broader capacity/parity work were open at
that checkpoint. Phase 4 later closes #713/#714 without changing those old runs.
That historical source did not accept #650; M1/M2 and RC/release qualification remain open.

The historical [integrated local qualification](../conformance/reports/core/2026-11-01-integrated-qualification.json)
passes its full gate at runtime base `16095d43`. The fixture census loses no
old agreement and gains 10 default/14 TLC old-ID shadow agreements, while
retaining refusals, stops and refuted/unconfirmed words. TLC Auto keeps all
model observables but physically exits 1 for omitted baseline SANY entries;
the default Auto command exits 0. The twelve original capacity cells finish
with nine successes and three ABA atom-analysis time stops. ABA default/W8
loses its older completion; completed capture earns no temporal success.
That historical matrix did not accept #650. The separately audited [performance diagnostics](../conformance/reports/perf/2026-11-01-integrated-performance.json)
retain a three-pair focus and a one-repeat ten-model breadth cohort. The focus
misses the default speed floor, so the formal five-repeat campaign was not
started. Breadth observations add no five-repeat or original temporal capacity
credit. Performance and capacity raw evidence is archived, and both phase
nodes/root volumes have verified cleanup; those receipts apply only to that
earlier campaign. The independently audited
[capacity record](../conformance/reports/perf/2026-11-01-original-capacity.json)
retains the failed acceptance and prior comparison. Historical
`83356b01`/`9fc88333` results keep their original source and limitations. The
projection-reuse experiment #705 is closed unmerged after its SimpleRegular
regression; #704 remains open. Reference remains the default. The Phase 5
section above records the later
bounded native simulation/DFID and safety storage gains; recovery, disk storage
and full-tier breadth remain open. The [next bounded actions](developer/core/m1-status.md#next-bounded-actions)
preserve the remaining word/classification/receiver boundaries and unmet
performance requirements. Phase 4 later accepted original capacity on its own
source; proof-hit counters alone never establish full-matrix acceptance.
