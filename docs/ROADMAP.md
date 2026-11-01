# Roadmap

Milestones match the GitHub milestones on `Chelis-Lang/dive`. Each milestone ends with:
- a review pass (a red-team agent or a human);
- a conformance report delta;
- ADRs for the decisions it made.

The charter's soundness contract applies throughout.

The [scoreboard](scoreboard.md) records the latest corpus results and the
roadmap feature checklist. Parity means observable results (ADR 0086).

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

The approved rescue roadmap in issue #241 retains complete symmetry, temporal
language, ordinary integrated liveness, simulation/generation and DFID, remaining
operators/configuration/output/debugger services, bounded-memory spill, native
checkpoint/recovery, native distributed checking, PlusCal and release
certification. Consolidating the search kernel and simplifying independent trace
replay support the remaining periodic liveness and mode integration. Corpus coverage, PlusCal, CLI/API
and performance work can proceed independently; modes and storage then build on
the consolidated kernel. Current capabilities and remaining work are recorded in
[the scoreboard](scoreboard.md).

Native protocols/checkpoints satisfy the approved scope; interoperability with
Java workers/checkpoints and arbitrary JVM plugin loading are excluded. Host
access still requires explicit category grants even under `--compat=tlc`.

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

The trusted-core checkpoint includes bounded construction-history, diagnostic
receiver and temporal-source-row replay, native closed-action execution and
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
acceptance remains open as #713, outside the new diagnostic-proof scope.
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
tracks the frozen gate's omission of that explicit command. It is separate from
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
after verification; #708/#713/#714 and broader capacity/parity work remain open.
#650, M1/M2 and RC/release qualification remain open.

The historical [integrated local qualification](../conformance/reports/core/2026-11-01-integrated-qualification.json)
passes its full gate at runtime base `16095d43`. The fixture census loses no
old agreement and gains 10 default/14 TLC old-ID shadow agreements, while
retaining refusals, stops and refuted/unconfirmed words. TLC Auto keeps all
model observables but physically exits 1 for omitted baseline SANY entries;
the default Auto command exits 0. The twelve original capacity cells finish
with nine successes and three ABA atom-analysis time stops. ABA default/W8
loses its older completion; completed capture earns no temporal success.
#650 acceptance remains open. The separately audited [performance diagnostics](../conformance/reports/perf/2026-11-01-integrated-performance.json)
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
regression; #704 remains open. Reference remains the default; simulation, DFID,
checkpoints, disk storage and full-tier breadth retain their existing later
scope. The [next bounded actions](developer/core/m1-status.md#next-bounded-actions)
preserve the remaining word/classification/receiver boundaries and finish the
unmet original-capacity/performance requirements. ABA TLC/W8 completion and
useful TLC/W1 headroom remain
an obligation; proof-hit counters do not establish its full-matrix acceptance.
