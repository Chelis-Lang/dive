# Scoreboard

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

The latest full gate at runtime source `2c966c74` on 2026-10-25 passed all 13
phases and **2,555 Rust tests with none skipped**. Small-tier results remain
**150/156 TLC** and **144/156 default**, with seven registered expected
divergences; all 458 SANY comparisons agree. A separate check of the original
`SpanTreeTest5Nodes` agrees in both profiles: 410,112 distinct states, 3,150,464
generated states and depth 5, within explicit 900-second/8-GiB budgets. This
closes its nested-powerset Init gap without adding credit to the historical
full-tier population below. See the
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

The fresh 2026-10-24 measurement of frozen source `68c09a1b` records **181 agree
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

The fresh run retains the same two-job, one-worker, 16 GiB and TLC-derived
900-second maximum model policy. All ten harness timeouts are native `stopped`
results with exit 5: nine time stops and one memory stop. The
[current compact record](survey/data/full-tier-measurement-2026-10-24.json)
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
