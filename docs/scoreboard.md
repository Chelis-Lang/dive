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
comparison rules). The one-worker rows are from `e87693d6`, adding pure
state-level recursive temporal atoms: `examples` model
`specifications/glowingRaccoon/product.cfg` changes from unsupported to agree
in both profiles. The four-worker row is from `main` at `1f5ae5fa`.
The selection is the small tier of `examples`, `probes`, `validation-smoke` and
`configured-temporal-operators`: 156 models, plus 458 SANY module comparisons in
the TLC profile.

| Profile | Workers | Models | Agree | Unsupported | Expected divergence | Disagree / timeout / pending / oracle error |
|---------|--------:|-------:|------:|------------:|--------------------:|-------------------------------------------:|
| TLC | 1 | 156 | 144 | 12 | 0 | 0 |
| default | 1 | 156 | 139 | 12 | 5 | 0 |
| default | 4 | 156 | 93 | 60 | 3 | 0 |

The one-worker rows are committed as baselines in `conformance/baselines/`.
`cargo xtask gate --full` and the nightly `baseline` job fail on any model or
module that regresses from them (see `docs/developer/ci.md`). A PR that
improves a row refreshes its baseline and this table from its own runs.
Expected divergence never counts as agreement. The per-model outcomes are in
[docs/conformance.md](conformance.md); earlier measurements are in the history
of this file.

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
