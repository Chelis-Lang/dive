# Scoreboard

L5's native capture/analysis/report path is implemented: persistent original
rows, checked CSR, controlled eager Graph admission, on-demand values with zero
retained Value cache, and sealed complete/prefix facts. Only selected-word values
survive into fresh independent typed and serialized validation. Existing fixed
capture limits remain; Store/native byte accounting is partial, and original
TLC-relative capacity/performance qualification remains open. No corpus
population, parity-surface, throughput or default-engine credit follows from
this storage integration (ADRs 0117 and 0122).

#619 soundness boundary: compound set normalization uses the existing shape
proof in both core profiles. Unproved nested comparisons return unsupported;
homogeneous, record-domain and lazy controls remain supported. Twelve pinned
consumed cases are in `crates/dive-api/tests/fixtures/core-normalization/`.
This does not add a completed parity surface; the separate reference initial
fingerprint classification finding remains open as #620.

Parity means observable results (ADR 0086). For every corpus model, dive must
match the pinned TLC on:

- the verdict;
- distinct and generated state counts (at 1 worker);
- the violated property name;
- the error class and exit code;
- the depth;
- a counterexample that the independent trace validator accepts.

dive does not emulate arbitrary or nondeterministic TLC internals, such as
timing, thread or queue order, tableau numbering or SCC and lasso choice.

## Corpus scoreboard

The #515 reference cache correction is checked by the explicit
`reference-instance-naturals` corpus and 18 consumed scope/startup controls.
The original reproduction remains strict for reference/core/auto; nested
instances retain separate core/auto disagreement #634, and assumption-name
metadata remains #504's obligation. These selected controls add no agreement
credit to the historical populations below.

The core's bounded instance-query extension (#609) is checked separately by
`core_enabled_instance`: 210 supported source/profile/worker/cache cells,
124 explicit refusal cells and 28 frontend level-rejection cells. It admits
constant/operator parameters with proved identity mappings of root variables;
hidden/nonidentity query variables and instantiated initial queries remain
refused. This adds no credit to the corpus populations below. See the
[compiler boundary](developer/core/04-plan-compiler.md#55-enabled-built).

A fresh `--engine=auto` control at source `0f0c6ad1` on 2026-10-28 preserves
both one-worker 156-model baselines and all 458 SANY comparisons. Actual strict
agreements on the core are 121 default and 125 TLC; the remaining strict
agreements use fallback or end before a search. Four further default core rows
are registered expected divergences. See the
[engine accounting](../conformance/reports/core/2026-10-28-small-auto-source0f.json).
No core-only full-tier or engine-default credit follows.

The separate `assumption-reporting` corpus adds five pinned model facts and
five accepted SANY modules for failed-ASSUME property names (#504). API controls
compare actual source images, class/status, early termination and null error
properties across both profiles, Reference/Core/Auto and workers 1/4. These
are targeted controls, not an update to the historical populations below.

The initial core-reboot acceptance census on 2026-10-28 is a separate shadow population,
not an update to the TLC agreement rows below. At backend `2d55317a` with the
strengthened harness `16336b64`, 1,361 fixture configurations per profile yield
983 default / 959 TLC-profile shadow agreements, with explicit refusals, budget
stops and successor/trace findings. It is not clean M1 acceptance. The
[qualification](../conformance/reports/core/2026-10-28-fixture-census-v1.json)
preserves every non-agreement ID, source/binary identity and accounting; the
[current core status](developer/core/m1-status.md) separates the remaining
families. No full-tier, core-only or default-engine promotion credit follows.
The final actual-main run at `f1acc846` selects 1,363 fixtures per profile
(1,476 configurations minus 99 parser exclusions and 14 ambiguous skips):
1,025 default / 1,002 TLC-profile shadow agreements, zero sampled-successor
findings, but still explicit trace/refusal/stop findings and job exit 1.
The [final qualification](../conformance/reports/core/2026-10-28-fixture-census-final.json)
separates 40 / 41 improved common IDs from two newly added fixtures in each
profile. Every other original fixture status is unchanged. This adds no credit
to the TLC parity table or historical full-tier measurement.

The 13-phase full gate at runtime source `2c966c74` on 2026-10-25 passed,
with conditional cargo-deny not applicable because dependency inputs did not
change, and **2,555 Rust tests with none skipped**. Small-tier results remain
**150/156 TLC** and **144/156 default**, with seven registered expected
divergences; all 458 SANY comparisons agree. A separate check of the original
`SpanTreeTest5Nodes` agrees in both profiles: 410,112 distinct states, 3,150,464
generated states and depth 5, within explicit 900-second/8-GiB budgets. This
closed its nested-powerset Init gap. That isolated check added no population
credit; the fresh full-tier measurement below now records the agreement. See the
[qualification](survey/data/nested-powerset-mask-qualification-2026-10-25.json).

The earlier full gate at source `5217163f` on 2026-10-25 passed all 13 phases
and **2,487 Rust tests with none skipped**. Small-tier results remain
**150/156 TLC** and **144/156 default**, with seven registered expected
divergences; all 458 SANY comparisons agree. A separate 16-model startup
primary selection records **11 agreements and five refusals in each profile**,
including seven startup2253 agreements. These separate probes add no credit to
the established populations or historical full-tier result. See the
[startup qualification](survey/data/startup-zero-solution-qualification-2026-10-25.json).


The earlier one-worker recheck at frozen source `92a33e3b` on 2026-10-25 passed
all 13 full-gate phases and 2,450 Rust tests with none skipped. The rows below
remain **150/156 TLC** and **144/156 default**, with seven registered expected
divergences in the default profile; all 458 SANY comparisons agree. No baseline
or established population changed. A separate 13-model validation selection
records nine agreements and four refusals in each profile, including five new
finite temporal-error agreements. These microcorpus gains add no credit to this
table or the historical **181/217** full-tier result below. See the
[qualification](survey/data/temporal-row-errors-qualification-2026-10-25.json).

The one-worker baseline gains were measured on 2026-10-23 against pinned TLC
`tlc-2026.10.06.014338` goldens, at exact source `db8d4ad5` on `1892ddc7`:
all 13 full-gate phases passed, with 2,295 tests passed and none skipped.
`CoffeeCan1000Beans` moves from a product-vertex refusal to agreement in both
profiles: 501,500 distinct / 2,000,002 generated / depth 1. All 156 model IDs
per profile and 458 SANY comparisons remain selected; every other typed baseline
observable is unchanged. Separate ordinary checks retain all four properties
and TypeInvariant, and all eight normal instruction pairs remain within the
1% overhead ceiling. The
[compact capacity qualification](survey/data/product-vertices-qualification-2026-10-23.json)
records actual source, reports, binaries, generated baselines and limitations.
No new four-worker, full-tier, throughput or peak-memory result follows.

The earlier 2026-10-22 observation used exact source `a9c7376f` on `5913ffd6`,
with its full gate passing and all 2,274 tests run. The complete 156-model population
is unchanged. Supported default-profile symmetry liveness now checks the whole
unreduced graph. `P4SymLive` moves from refusal to the exact registered D-005
expected divergence: 2 distinct / 3 generated / depth 2, versus TLC's reduced
1 / 2 / 1. This closes one default refusal without increasing Agree. TLC quotient
and VIEW liveness remain refused. All other baseline observables are unchanged;
all 458 SANY module comparisons agree. The
[compact qualification](survey/data/unreduced-symmetry-liveness-qualification-2026-10-22.json)
records source and tool pins, actual reports, generated baselines and limitations.

The earlier `866c00ba` results on `4e219ea8`, with an explicit file graph-label
policy over packed atom truth, remain unchanged. `FiniteMonotonic/MCCRDT`
moved from unsupported to
agreement under TLC semantics (25,000 distinct / 1,350,001 generated / depth 16).
Its default-profile result (25,000 / 900,001 / 16) remains the exact registered
D-022 expected divergence, never agreement. `allocator/AllocatorImplementation`
moves from unsupported to agree in both profiles (17,701 / 64,414 / 16).
All other retained one-worker observable fields match their baselines.

The earlier `11b4eefb` improvements for ElevatorLivenessMedium and
MCLiveWriteThroughCache remain unchanged. The four-worker row is a separate
historical observation at clean `main` source `52485957` on 2026-10-21; it was
not repeated by the newer one-worker qualification.
The selection is the small tier of `examples`, `probes`, `validation-smoke` and
`configured-temporal-operators`: 156 models, plus 458 SANY module comparisons in
the TLC profile.

| Profile | Workers | Models | Agree | Unsupported | Expected divergence | Disagree / timeout / pending / oracle error |
|---------|--------:|-------:|------:|------------:|--------------------:|-------------------------------------------:|
| TLC | 1 | 156 | 150 | 6 | 0 | 0 |
| default | 1 | 156 | 144 | 5 | 7 | 0 |
| default | 4 | 156 | 144 | 9 | 3 | 0 |

The four-worker observation replaces the historical `1f5ae5fa` row
(93 agree, 60 unsupported, three expected divergences). It used two concurrent
model jobs, TLC-derived native time budgets capped at 900 seconds, an 8 GiB
whole-process ceiling per run, and independent process/campaign backstops.
All 156 raw native reports were retained; no run stopped, timed out or remained
pending. SANY was not repeated for this model-only observation.

Of the 144 agreements, 128 actually used four workers, 14 serialized to one,
and two were expected parser rejections before any search. Across all outcomes,
134 runs used four workers, 14 used one and eight ran no search. All 49 completed
temporal runs (47 successes and two independently validated liveness violations)
used four workers. These observations earn no throughput or scaling credit.

The historical 144 agreements at four workers versus its contemporaneous 142
at one worker (both source `52485957`) reflect the maintained multi-worker
comparison policy: `FiniteMonotonic/MCReplicatedLog`
and `MissionariesAndCannibals` move from expected divergence to agreement under
that policy. They earn no additional one-worker parity credit. The three
remaining expected divergences and nine named refusals belong to that historical
source; do not compare its aggregate directly with the newer one-worker row. The [compact observation](survey/data/small-default-w4-2026-10-21.json)
records every selected model, actual worker use, pins, budgets and raw-report
hashes. This row is scoped to source `52485957`.

The one-worker rows are committed as baselines in `conformance/baselines/`.
`cargo xtask gate --full` and the nightly `baseline` job fail on any model or
module that regresses from them (see `docs/developer/ci.md`). A PR that
improves a row refreshes its baseline and this table from its own runs.
Expected divergence never counts as agreement. The per-model outcomes are in
[docs/conformance.md](conformance.md); earlier measurements are in the history
of this file.

## Full-tier measurement

The latest 2026-10-26 measurement of source `672fc1b1` records **182 agreements
out of 217 in-scope models**, up from 181. The sole new agreement is
`SpanningTree/SpanTreeTest5Nodes.cfg`; no historical agreement was lost. All 265
historical IDs, 48 exclusions, nine missing references and four selected corpora
remain unchanged. The 35 remaining gaps are 16 named refusals, ten resource
stops and nine missing references, with zero disagreements. Report:
`20261026T023947024-dive-all`. The later `b91b73a6` main has identical runtime
and Cargo inputs; its new primary corpus is outside this selection.

The same TLC-profile W1/jobs2 policy uses a 16 GiB native ceiling and
`min(900 s, max(60 s, 10 × golden TLC elapsed))`, with 30 seconds of cleanup
grace. All ten stops returned native exit 5: eight time stops and two memory
stops. Their prefix statistics earn no completion or agreement credit. The
normal harness completed with exit 1 and nine missing references; this is not
a full-gate pass or clean release acceptance.

The controller exhausted its original S3 preservation window after the models
and normal `report baseline` utility had completed. The original failure stays
intact. After whole-node quiescence, preservation-only recovery and a RAM
audit verified all 1,288 original files (3,009,809 bytes), without rerunning
models, builds or the utility. The generated digest is copied exactly from that
utility's output. The [current compact record](survey/data/full-tier-measurement-2026-10-26.json)
retains the 35 gaps, typed stops, source/input pins, S3 receipts and cleanup.
No new small-tier, four-worker, SANY or performance result follows.

The earlier 2026-10-24 measurement of frozen source `68c09a1b` records **181 agree
of 217 in-scope models**, in the TLC profile at one worker. All 265 historical
model IDs remain selected, with the same 48 exclusions and nine missing
references. There are ten new agreements and no lost agreements relative to the
controlled 2026-10-21 baseline. The remaining 36 in-scope gaps are 17 named
refusals, ten resource stops and nine missing references; there are no
disagreements. Report: `20261024T165628670-dive-all`.

The earlier controlled 2026-10-21 campaign measured frozen native source `39f4e483`
with harness `0ee2a2dd`, TLC profile, one worker: **171 agree of 217 in-scope
models**. This is a historical source measurement, separate from the newer
small-tier main results above. All 265 historical model IDs remain selected,
including 48 exclusions and nine missing references; no population was filtered
or resized. Report: `20261021T141221299-dive-all`.

| Campaign | Agree | Unsupported | Budget stop / timeout | Missing reference | Disagree |
|----------|------:|------------:|----------------------:|------------------:|---------:|
| Historical `652a5334`, 16 jobs, 3,600 s/model | 174 | 28 | 5 | 9 | 1 |
| Controlled `39f4e483`, 2 jobs, explicit budgets | 171 | 27 | 10 | 9 | 0 |
| Fresh `68c09a1b`, 2 jobs, same bounded policy | 181 | 17 | 10 | 9 | 0 |
| Fresh `672fc1b1`, 2 jobs, same bounded policy | 182 | 16 | 10 | 9 | 0 |

The 2026-10-21 controlled run used a 64 GiB c7i.8xlarge, a 16 GiB whole-process ceiling per
model and TLC-derived time budgets capped at 900 seconds, with 30 seconds of
cleanup grace. Gates and instruction profiling shared the host, so there is no
quiet-host throughput claim. All ten incomplete runs returned native `stopped`
(exit 5); the harness records these resource findings as `timeout`, never as
unsupported or agreement. Nine stopped on time and one on memory.

Three historical agreements became budget stops in that controlled run: ElevatorSafetyMedium
([#359](https://github.com/Chelis-Lang/dive/issues/359)), bosco
([#366](https://github.com/Chelis-Lang/dive/issues/366)), and DijkstraMutex's
Safety-4-processors configuration
([#368](https://github.com/Chelis-Lang/dive/issues/368)). The count falls from
174 to 171; those findings are explicit and unresolved, not erased by a new
digest. The historical `MCNanoLarge` OOM remains a disagreement in its original
report ([#351](https://github.com/Chelis-Lang/dive/issues/351)); its controlled
2026-10-21 time stop earned no agreement or termination-fix credit. EWD998Chan stopped before reaching its
old graph-capture count refusal, which earns no capacity credit
([#369](https://github.com/Chelis-Lang/dive/issues/369)).

The 2026-10-24 run retained the same two-job, one-worker, 16 GiB and TLC-derived
900-second maximum model policy. All ten harness timeouts are native `stopped`
results with exit 5: nine time stops and one memory stop. The
[earlier compact record](survey/data/full-tier-measurement-2026-10-24.json)
lists the ten gains and all 36 gaps, including actual stopped phases and prefix
statistics. No native signal or partial output stream was substituted for a
completed result.

The original harness completed normally with exit 1. A later packaging
`TypeError` occurred after the report and raw audits; its failure is preserved.
After whole-campaign quiescence, recovery ran only the previously unexecuted
`report baseline` utility and preserved originals in S3. No model or build was
rerun. `conformance/baselines/full-tlc-w1.json` is an exact copy of that generated
measurement; no gate consumes this full-tier observation digest. This is a
complete measurement of **incomplete full-tier support, not clean release
acceptance**. The historical
[compact summary](survey/data/full-tier-budgeted-2026-10-21.json) retains exact
pins, hashes, selection counts and every model-specific resource finding.
This full-tier measurement does not rerun SANY or refresh the independent
small-tier or four-worker measurements above. It supplies no full-gate,
throughput, scaling or peak-memory result. The backlog remains
[#338](https://github.com/Chelis-Lang/dive/issues/338).

## Roadmap feature checklist

Status is one word: `implemented`, `partial` or `missing`. `implemented` means
the feature works for the supported corpus. It does not certify full parity.
`conformance/parity-surfaces.json` is the reference inventory of TLC and SANY
surfaces.

Core sequence replacement now supports pure materialized sequences with one
integer, boolean or string element class across arguments (#610). Compound,
mixed, lazy and effectful arguments, plus overlapping All occurrences after
the early equality CASE arms, remain refused; CommunityModules and
fallback definitions stay partial. Focused fixture results are not a new corpus
census or milestone acceptance (`docs/developer/core/03-value-model.md` §12).

| Family | Status |
|--------|--------|
| TLA+ grammar and lossless parsing | implemented |
| Semantic resolution, levels and module/proof syntax | implemented |
| Configuration parsing | implemented |
| Model/configuration binding and startup | partial |
| Values, equality, order, laziness and representation | partial |
| Evaluation scopes, recursion and higher-order operators | partial |
| Finite domains, successor enumeration and independent ENABLED | partial |
| Native override discovery and installation | partial |
| Standard-module operators | partial |
| CommunityModules operators and TLA+ fallback definitions | partial |
| Error classes and precedence | partial |
| Sequential BFS safety, invariants and deadlock | implemented |
| Deterministic parallel checking | implemented |
| Fingerprint/exact stores and collision accounting | implemented |
| State/action constraints and action-property split | implemented |
| VIEW identity and temporal lifting | partial |
| Symmetry declaration binding, warnings and closure | partial |
| Symmetry reduction, concrete traces and audit | partial |
| Temporal language, simplification and tableaux | partial |
| Temporal graph construction | partial |
| WF/SF fairness | partial |
| Liveness acceptance, counterexamples and attribution | partial |
| Periodic/final/off liveness schedules | missing |
| Simulation, generation and random replay | partial |
| DFID checking | partial |
| Granted environment, file and process effects | missing |
| Runtime introspection and configuration services | partial |
| CLI/output conventions and message controls | partial |
| Trace/state dumps, ALIAS and trace exploration | partial |
| Coverage and debugger services | missing |
| Bounded-memory stores, frontiers, graphs and spill | partial |
| Checkpoint and recovery | partial |
| Distributed checking and recovery | missing |
| PlusCal translation and translation validation | partial |
| Versioned JSON reports and trace compatibility policy | implemented |
| Public Rust APIs and cargo-test integration | partial |
| Complete corpus, harness and release certification | partial |
| Global TLC profile and approved better defaults | partial |
