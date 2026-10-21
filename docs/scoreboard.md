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

Measured 2026-10-21 (pinned TLC `tlc-2026.10.06.014338` goldens, unchanged
comparison rules). The one-worker rows are from `11b4eefb`, separating pure
Boolean recurrence and persistence conditions from shared temporal memory.
`examples` models `specifications/MultiCarElevator/ElevatorLivenessMedium.cfg`
and `specifications/SpecifyingSystems/Liveness/MCLiveWriteThroughCache.cfg`
each change from unsupported to agree in both profiles. They match 4,122 / 14,296
and 5,196 / 28,170 distinct/generated states, with depths 36 and 18 respectively.
The four-worker row is from `main` at `1f5ae5fa`.
The selection is the small tier of `examples`, `probes`, `validation-smoke` and
`configured-temporal-operators`: 156 models, plus 458 SANY module comparisons in
the TLC profile.

| Profile | Workers | Models | Agree | Unsupported | Expected divergence | Disagree / timeout / pending / oracle error |
|---------|--------:|-------:|------:|------------:|--------------------:|-------------------------------------------:|
| TLC | 1 | 156 | 147 | 9 | 0 | 0 |
| default | 1 | 156 | 142 | 9 | 5 | 0 |
| default | 4 | 156 | 93 | 60 | 3 | 0 |

The one-worker rows are committed as baselines in `conformance/baselines/`.
`cargo xtask gate --full` and the nightly `baseline` job fail on any model or
module that regresses from them (see `docs/developer/ci.md`). A PR that
improves a row refreshes its baseline and this table from its own runs.
Expected divergence never counts as agreement. The per-model outcomes are in
[docs/conformance.md](conformance.md); earlier measurements are in the history
of this file.

## Full-tier measurement

The controlled 2026-10-21 campaign measured frozen native source `39f4e483`
with harness `0ee2a2dd`, TLC profile, one worker: **171 agree of 217 in-scope
models**. This is a historical source measurement, separate from the newer
small-tier main results above. All 265 historical model IDs remain selected,
including 48 exclusions and nine missing references; no population was filtered
or resized. Report: `20261021T141221299-dive-all`.

| Campaign | Agree | Unsupported | Budget stop / timeout | Missing reference | Disagree |
|----------|------:|------------:|----------------------:|------------------:|---------:|
| Historical `652a5334`, 16 jobs, 3,600 s/model | 174 | 28 | 5 | 9 | 1 |
| Controlled `39f4e483`, 2 jobs, explicit budgets | 171 | 27 | 10 | 9 | 0 |

The controlled run used a 64 GiB c7i.8xlarge, a 16 GiB whole-process ceiling per
model and TLC-derived time budgets capped at 900 seconds, with 30 seconds of
cleanup grace. Gates and instruction profiling shared the host, so there is no
quiet-host throughput claim. All ten incomplete runs returned native `stopped`
(exit 5); the harness records these resource findings as `timeout`, never as
unsupported or agreement. Nine stopped on time and one on memory.

Three historical agreements are now budget stops: ElevatorSafetyMedium
([#359](https://github.com/Chelis-Lang/dive/issues/359)), bosco
([#366](https://github.com/Chelis-Lang/dive/issues/366)), and DijkstraMutex's
Safety-4-processors configuration
([#368](https://github.com/Chelis-Lang/dive/issues/368)). The count falls from
174 to 171; those findings are explicit and unresolved, not erased by a new
digest. The historical `MCNanoLarge` OOM remains a disagreement in its original
report ([#351](https://github.com/Chelis-Lang/dive/issues/351)); the new timed
stop earns no agreement or fix credit. EWD998Chan now stops before reaching its
old graph-capture count refusal, which earns no capacity credit
([#369](https://github.com/Chelis-Lang/dive/issues/369)).

`conformance/baselines/full-tlc-w1.json` is generated only through `report
baseline`; no gate consumes this full-tier observation digest. This remains
**an incomplete full-tier result, not clean release acceptance**. The
[compact summary](survey/data/full-tier-budgeted-2026-10-21.json) retains exact
pins, hashes, selection counts and every model-specific resource finding.
The accepted small-tier baselines above are unchanged; the backlog remains
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
| Public Rust APIs and cargo-test integration | partial |
| Complete corpus, harness and release certification | partial |
| Global TLC profile and approved better defaults | partial |
